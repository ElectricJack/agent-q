//! Durable dispatch owner. Only `tick` may turn a committed attempt into HTTP;
//! recovery never returns that attempt to the send queue.
use super::*;
use aq_notifications::discord::{Discord, Exchange, ThrottleScope};
use aq_notifications::{DispatchAuthority, FailureReason, SuppressionReason, TransportOutcome};

const LEASE_MS: u64 = 20_000;
async fn dispatcher(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<()> {
    lock_budget(tx).await?;
    let current: Option<Uuid> =
        sqlx::query_scalar("SELECT dispatcher_id FROM notification_budget WHERE singleton")
            .fetch_one(&mut **tx)
            .await?;
    if current != Some(id) {
        return Err(Error::DispatcherFenced);
    }
    Ok(())
}
pub(super) async fn save(
    tx: &mut Transaction<'_, Postgres>,
    d: &Delivery,
    expected: i64,
) -> Result<()> {
    d.validate()?;
    let (status, due, attempt, lease) = match d.status {
        Status::Queued => ("queued", d.message.created_at_ms, None, None),
        Status::Dispatching {
            attempt_id,
            started_at_ms,
        } => (
            "dispatching",
            started_at_ms,
            Some(attempt_id),
            Some(started_at_ms.checked_add(LEASE_MS).ok_or(Error::Invalid)?),
        ),
        Status::RetryScheduled { next_attempt_ms } => {
            ("retry_scheduled", next_attempt_ms, None, None)
        }
        Status::AcceptedByTransport { .. } => ("accepted_by_transport", 0, None, None),
        Status::DeliveryUnknown { .. } => ("delivery_unknown", 0, None, None),
        Status::PermanentlyFailed { .. } => ("permanently_failed", 0, None, None),
        Status::Suppressed { .. } => ("suppressed", 0, None, None),
    };
    let changed=sqlx::query("UPDATE notification_deliveries SET state=$1,status=$2,due_at_ms=$3,attempt_id=$4,lease_until_ms=$5,record_revision=record_revision+1 WHERE delivery_id=$6 AND record_revision=$7")
        .bind(serde_json::to_value(d)?).bind(status).bind(due as i64).bind(attempt).bind(lease.map(|v|v as i64)).bind(d.delivery_id).bind(expected).execute(&mut **tx).await?.rows_affected();
    if changed != 1 {
        return Err(Error::Conflict);
    }
    Ok(())
}
struct Claimed {
    delivery: Delivery,
    revision: i64,
    attempt: Uuid,
    claimed: std::time::Instant,
}
impl Service {
    /// At most one send per tick. Callers use a separate bounded notification
    /// pool/task; SQL connections and locks are released before HTTP begins.
    pub async fn tick(&self, dispatcher_id: Uuid, client: &Discord) -> Result<bool> {
        let Some(claim) = self.claim(dispatcher_id).await? else {
            return Ok(false);
        };
        // The committed dispatch is already possibly effective. A pause or
        // changed configuration cannot safely turn it back into queued work.
        if claim.claimed.elapsed().as_millis() >= 1000 {
            return Ok(true);
        }
        let endpoint = self
            .configuration
            .as_ref()
            .and_then(|c| c.endpoints.get(&claim.delivery.route.endpoint_key))
            .filter(|e| e.revision == claim.delivery.route.endpoint_revision)
            .and_then(|e| e.webhook.as_ref())
            .ok_or(Error::Unavailable)?;
        let exchange = client.send(endpoint, &claim.delivery.payload).await?;
        self.finish(dispatcher_id, &claim, exchange).await?;
        Ok(true)
    }
    async fn claim(&self, dispatcher_id: Uuid) -> Result<Option<Claimed>> {
        let mut tx = self.pool.begin().await?;
        dispatcher(&mut tx, dispatcher_id).await?;
        let now = self.now()?;
        // A takeover waits out the prior finite attempt lease. It never
        // retransmits, including when the old process died before opening HTTP.
        let expired=sqlx::query("SELECT state,record_revision FROM notification_deliveries WHERE status='dispatching' AND lease_until_ms<=$1 ORDER BY sequence LIMIT 64 FOR UPDATE")
            .bind(now as i64).fetch_all(&mut *tx).await?;
        for row in expired {
            let d: Delivery = serde_json::from_value(row.get("state"))?;
            save(&mut tx, &d.recover(now)?, row.get("record_revision")).await?;
        }
        let provider: i64 = sqlx::query_scalar(
            "SELECT provider_not_before_ms FROM notification_budget WHERE singleton",
        )
        .fetch_one(&mut *tx)
        .await?;
        let inflight: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notification_deliveries WHERE status='dispatching'",
        )
        .fetch_one(&mut *tx)
        .await?;
        if now < provider as u64 || inflight >= 4 {
            tx.commit().await?;
            return Ok(None);
        }
        let after: i64 =
            sqlx::query_scalar("SELECT scan_after FROM notification_budget WHERE singleton")
                .fetch_one(&mut *tx)
                .await?;
        let rows=sqlx::query("SELECT sequence,state,record_revision FROM notification_deliveries WHERE sequence>$2 AND status IN ('queued','retry_scheduled') AND due_at_ms<=$1 ORDER BY sequence LIMIT 64 FOR UPDATE")
            .bind(now as i64).bind(after).fetch_all(&mut *tx).await?;
        if rows.is_empty() {
            sqlx::query("UPDATE notification_budget SET scan_after=0 WHERE singleton")
                .execute(&mut *tx)
                .await?;
        }
        for row in rows {
            sqlx::query("UPDATE notification_budget SET scan_after=$1 WHERE singleton")
                .bind(row.get::<i64, _>("sequence"))
                .execute(&mut *tx)
                .await?;
            let mut d: Delivery = serde_json::from_value(row.get("state"))?;
            d.validate()?;
            let rev: i64 = row.get("record_revision");
            let scope_ok = match scope(&mut tx, &d.message.scope).await {
                Ok(()) => true,
                Err(Error::NotFound) => false,
                Err(e) => return Err(e),
            };
            let recipient_ok = match recipient(&mut tx, d.message.recipient_user_id).await {
                Ok(()) => true,
                Err(Error::NotFound) => false,
                Err(e) => return Err(e),
            };
            let current: Option<Value> = sqlx::query_scalar(
                "SELECT definition FROM notification_routes WHERE route_id=$1 FOR SHARE",
            )
            .bind(d.route.route_id)
            .fetch_optional(&mut *tx)
            .await?;
            let current: Option<Route> = current.map(serde_json::from_value).transpose()?;
            let now = self.now()?;
            if now < d.last_transition_ms {
                return Err(Error::Unavailable);
            }
            let suppression = if !scope_ok || !recipient_ok {
                Some(SuppressionReason::ScopeRevoked)
            } else if current.as_ref() != Some(&d.route) || !d.route.enabled {
                Some(SuppressionReason::RouteChanged)
            } else if now >= d.message.expires_at_ms {
                Some(SuppressionReason::MessageExpired)
            } else {
                None
            };
            if let Some(reason) = suppression {
                if now < d.last_transition_ms {
                    return Err(Error::Unavailable);
                }
                d.status = Status::Suppressed { reason };
                d.last_transition_ms = now;
                save(&mut tx, &d, rev).await?;
                continue;
            }
            let endpoint=sqlx::query("SELECT endpoint_revision,available,disabled,not_before_ms FROM notification_destinations WHERE endpoint_key=$1 FOR SHARE").bind(&d.route.endpoint_key).fetch_optional(&mut *tx).await?;
            let usable = endpoint.as_ref().is_some_and(|e| {
                e.get::<i64, _>("endpoint_revision") as u64 == d.route.endpoint_revision
                    && e.get::<bool, _>("available")
                    && !e.get::<bool, _>("disabled")
            });
            if !usable {
                d.status = Status::PermanentlyFailed {
                    reason: FailureReason::DestinationUnavailable,
                };
                d.last_transition_ms = now;
                save(&mut tx, &d, rev).await?;
                continue;
            }
            if endpoint.unwrap().get::<i64, _>("not_before_ms") as u64 > now {
                continue;
            }
            let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notification_deliveries WHERE endpoint_key=$1 AND status='dispatching')").bind(&d.route.endpoint_key).fetch_one(&mut *tx).await?;
            if busy {
                continue;
            }
            let attempt = Uuid::new_v4();
            let next = match d.begin(
                DispatchAuthority {
                    route: current.as_ref().ok_or(Error::Unavailable)?,
                    recipient_can_view: true,
                },
                attempt,
                now,
            ) {
                Ok(next) => next,
                Err(aq_notifications::Error::Exhausted) => {
                    d.status = Status::PermanentlyFailed {
                        reason: FailureReason::RetryExhausted,
                    };
                    d.last_transition_ms = now;
                    save(&mut tx, &d, rev).await?;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };
            save(&mut tx, &next, rev).await?;
            // Start the local pause bound before commit, not after it.
            let claimed = std::time::Instant::now();
            tx.commit().await?;
            return Ok(Some(Claimed {
                delivery: next,
                revision: rev + 1,
                attempt,
                claimed,
            }));
        }
        tx.commit().await?;
        Ok(None)
    }
    async fn finish(&self, dispatcher_id: Uuid, claim: &Claimed, exchange: Exchange) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        dispatcher(&mut tx, dispatcher_id).await?;
        let row=sqlx::query("SELECT state,record_revision FROM notification_deliveries WHERE delivery_id=$1 FOR UPDATE").bind(claim.delivery.delivery_id).fetch_one(&mut *tx).await?;
        let d: Delivery = serde_json::from_value(row.get("state"))?;
        if row.get::<i64, _>("record_revision") != claim.revision || d != claim.delivery {
            return Err(Error::Conflict);
        }
        let now = self.now()?;
        if let Some(throttle) = exchange.throttle {
            let due = now
                .checked_add(throttle.after_ms)
                .filter(|v| *v <= i64::MAX as u64)
                .ok_or(Error::Unavailable)?;
            match throttle.scope {
                ThrottleScope::Provider => {
                    sqlx::query("UPDATE notification_budget SET provider_not_before_ms=GREATEST(provider_not_before_ms,$1) WHERE singleton").bind(due as i64).execute(&mut *tx).await?;
                }
                ThrottleScope::Destination => {
                    sqlx::query("UPDATE notification_destinations SET not_before_ms=GREATEST(not_before_ms,$1) WHERE endpoint_key=$2 AND endpoint_revision=$3").bind(due as i64).bind(&d.route.endpoint_key).bind(d.route.endpoint_revision as i64).execute(&mut *tx).await?;
                }
            }
        }
        if exchange.outcome == TransportOutcome::Permanent(FailureReason::DestinationUnavailable) {
            sqlx::query("UPDATE notification_destinations SET disabled=TRUE WHERE endpoint_key=$1 AND endpoint_revision=$2").bind(&d.route.endpoint_key).bind(d.route.endpoint_revision as i64).execute(&mut *tx).await?;
        }
        let jitter = Uuid::new_v4().as_u128() as u64;
        let next = d.observe(claim.attempt, exchange.outcome, now, jitter)?;
        save(&mut tx, &next, claim.revision).await?;
        tx.commit().await?;
        Ok(())
    }
}
