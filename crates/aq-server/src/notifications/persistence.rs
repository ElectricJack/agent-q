//! PostgreSQL owner. Lock order: notification budget -> project -> sorted users
//! -> routes -> deliveries -> idempotency receipts. Never network under locks.
use super::{Error, Result, config::Loaded};
use aq_lifecycle_api::Scope;
use aq_notifications::{Delivery, Message, Payload, Route, Status};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub struct Service {
    pub pool: PgPool,
    pub configuration: Option<Arc<Loaded>>,
    pub public_origin: String,
    pub clock: crate::Clock,
}
pub use aq_notifications::{RetryRequest, RouteRequest, SendRequest};
#[derive(Serialize)]
pub struct Inbox {
    pub items: Vec<Value>,
    pub next_after: Option<i64>,
}
pub(crate) fn digest<T: Serialize>(v: &T) -> Result<String> {
    let mut value = serde_json::to_value(v)?;
    value.sort_all_objects();
    Ok(crate::tokens::verifier(&serde_json::to_string(&value)?))
}
fn revision(v: u64) -> Result<i64> {
    if v == 0 || v > i64::MAX as u64 {
        Err(Error::Invalid)
    } else {
        Ok(v as i64)
    }
}
async fn lock_budget(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    sqlx::query("SELECT singleton FROM notification_budget WHERE singleton FOR UPDATE")
        .fetch_one(&mut **tx)
        .await?;
    Ok(())
}
async fn owner(tx: &mut Transaction<'_, Postgres>, actor: Uuid) -> Result<()> {
    let role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id=$1 FOR SHARE")
        .bind(actor)
        .fetch_optional(&mut **tx)
        .await?;
    if role.as_deref() != Some("owner") {
        return Err(Error::Forbidden);
    }
    Ok(())
}
async fn scope(tx: &mut Transaction<'_, Postgres>, scope: &Scope) -> Result<()> {
    let binding: Option<Value> = sqlx::query_scalar(
        "SELECT binding FROM project_execution_bindings WHERE project_id=$1 FOR SHARE",
    )
    .bind(scope.project_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(binding) = binding else {
        return Err(Error::NotFound);
    };
    if binding["enabled"] != true || binding["graph_workspace_id"] != scope.workspace_id.to_string()
    {
        return Err(Error::NotFound);
    }
    Ok(())
}
async fn recipient(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<()> {
    let found: Option<Uuid> = sqlx::query_scalar("SELECT id FROM users WHERE id=$1 FOR SHARE")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
    if found.is_none() {
        return Err(Error::NotFound);
    }
    Ok(())
}
async fn replay(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
    hash: &str,
) -> Result<Option<Value>> {
    if id.is_nil() {
        return Err(Error::Invalid);
    }
    let row=sqlx::query("SELECT request_digest,response FROM notification_operations WHERE actor_id=$1 AND operation_id=$2").bind(actor).bind(id).fetch_optional(&mut **tx).await?;
    match row {
        None => Ok(None),
        Some(row) => {
            if row.get::<String, _>("request_digest") != hash {
                return Err(Error::Conflict);
            }
            Ok(Some(row.get("response")))
        }
    }
}
async fn reserve(
    tx: &mut Transaction<'_, Postgres>,
    messages: i64,
    deliveries: i64,
    routes: i64,
    operations: i64,
) -> Result<()> {
    let bytes = messages * 16384 + deliveries * 32768 + routes * 16384 + operations * 16384;
    let changed=sqlx::query("UPDATE notification_budget SET reserved_bytes=reserved_bytes+$1,message_count=message_count+$2,delivery_count=delivery_count+$3,route_count=route_count+$4,operation_count=operation_count+$5 WHERE singleton AND reserved_bytes+$1<=134217728 AND message_count+$2<=4096 AND delivery_count+$3<=4096 AND route_count+$4<=128 AND operation_count+$5<=8192")
        .bind(bytes).bind(messages).bind(deliveries).bind(routes).bind(operations).execute(&mut **tx).await?.rows_affected();
    if changed != 1 {
        return Err(Error::Overloaded);
    }
    Ok(())
}
async fn receipt(
    tx: &mut Transaction<'_, Postgres>,
    actor: Uuid,
    id: Uuid,
    hash: &str,
    response: &Value,
) -> Result<()> {
    if serde_json::to_vec(response)?.len() > 16384 {
        return Err(Error::Invalid);
    }
    sqlx::query("INSERT INTO notification_operations(actor_id,operation_id,request_digest,response) VALUES($1,$2,$3,$4)").bind(actor).bind(id).bind(hash).bind(response).execute(&mut **tx).await?;
    Ok(())
}
impl Service {
    pub(crate) fn now(&self) -> Result<u64> {
        revision(
            (self.clock)()
                .timestamp_millis()
                .try_into()
                .map_err(|_| Error::Unavailable)?,
        )
        .map(|v| v as u64)
    }
    /// Startup authority publication; a newly installed dispatcher fences the old
    /// process. Missing configuration disables all destinations without deleting
    /// retained message/receipt data. No network or file reads occur here.
    pub async fn install(&self, dispatcher_id: Uuid) -> Result<()> {
        if dispatcher_id.is_nil() {
            return Err(Error::Invalid);
        }
        let mut tx = self.pool.begin().await?;
        lock_budget(&mut tx).await?;
        let head=sqlx::query("SELECT configuration_revision,configuration_digest FROM notification_budget WHERE singleton").fetch_one(&mut *tx).await?;
        if let Some(config) = &self.configuration {
            let shape: Vec<_> = config
                .endpoints
                .iter()
                .map(|(key, v)| (key, v.revision))
                .collect();
            let hash = digest(&(config.revision, shape))?;
            let old = head.get::<i64, _>("configuration_revision") as u64;
            if config.revision < old
                || (config.revision == old
                    && head
                        .get::<Option<String>, _>("configuration_digest")
                        .as_deref()
                        != Some(hash.as_str()))
            {
                return Err(Error::Conflict);
            }
            let retained: Vec<String> =
                sqlx::query_scalar("SELECT endpoint_key FROM notification_destinations")
                    .fetch_all(&mut *tx)
                    .await?;
            if retained.len()
                + config
                    .endpoints
                    .keys()
                    .filter(|key| !retained.contains(key))
                    .count()
                > 128
            {
                return Err(Error::Overloaded);
            }
            for (key, endpoint) in &config.endpoints {
                let previous=sqlx::query("SELECT endpoint_revision,fingerprint FROM notification_destinations WHERE endpoint_key=$1").bind(key).fetch_optional(&mut *tx).await?;
                if let Some(previous) = previous {
                    let old_revision = previous.get::<i64, _>("endpoint_revision") as u64;
                    let old_fingerprint: Option<String> = previous.get("fingerprint");
                    if endpoint.revision < old_revision
                        || (endpoint.revision == old_revision
                            && old_fingerprint
                                .as_ref()
                                .zip(endpoint.fingerprint.as_ref())
                                .is_some_and(|(a, b)| a != b))
                    {
                        return Err(Error::Conflict);
                    }
                }
            }
            sqlx::query("UPDATE notification_destinations SET available=FALSE")
                .execute(&mut *tx)
                .await?;
            for (key, endpoint) in &config.endpoints {
                sqlx::query("INSERT INTO notification_destinations(endpoint_key,endpoint_revision,configuration_revision,available,fingerprint) VALUES($1,$2,$3,$4,$5) ON CONFLICT(endpoint_key) DO UPDATE SET endpoint_revision=EXCLUDED.endpoint_revision,configuration_revision=EXCLUDED.configuration_revision,available=EXCLUDED.available,fingerprint=CASE WHEN EXCLUDED.endpoint_revision>notification_destinations.endpoint_revision THEN EXCLUDED.fingerprint ELSE COALESCE(EXCLUDED.fingerprint,notification_destinations.fingerprint) END,disabled=CASE WHEN EXCLUDED.endpoint_revision>notification_destinations.endpoint_revision THEN FALSE ELSE notification_destinations.disabled END")
                    .bind(key).bind(revision(endpoint.revision)?).bind(revision(config.revision)?).bind(endpoint.webhook.is_some()).bind(&endpoint.fingerprint).execute(&mut *tx).await?;
            }
            sqlx::query("UPDATE notification_budget SET configuration_revision=$1,configuration_digest=$2,dispatcher_id=$3 WHERE singleton").bind(revision(config.revision)?).bind(hash).bind(dispatcher_id).execute(&mut *tx).await?;
        } else {
            sqlx::query("UPDATE notification_destinations SET available=FALSE")
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE notification_budget SET dispatcher_id=$1 WHERE singleton")
                .bind(dispatcher_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
    pub async fn set_route(&self, actor: Uuid, request: RouteRequest) -> Result<Value> {
        request.route.validate()?;
        revision(request.route.revision)?;
        if request.expected_revision.checked_add(1) != Some(request.route.revision) {
            return Err(Error::Invalid);
        }
        let hash = digest(&("route/1", &request))?;
        let mut tx = self.pool.begin().await?;
        lock_budget(&mut tx).await?;
        scope(&mut tx, &request.route.scope).await?;
        owner(&mut tx, actor).await?;
        recipient(&mut tx, request.route.recipient_user_id).await?;
        if let Some(response) = replay(&mut tx, actor, request.operation_id, &hash).await? {
            return Ok(response);
        }
        let old: Option<Value> = sqlx::query_scalar(
            "SELECT definition FROM notification_routes WHERE route_id=$1 FOR UPDATE",
        )
        .bind(request.route.route_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(old) = &old {
            let old: Route = serde_json::from_value(old.clone())?;
            if old.revision != request.expected_revision
                || old.scope != request.route.scope
                || old.recipient_user_id != request.route.recipient_user_id
            {
                return Err(Error::Conflict);
            }
        } else if request.expected_revision != 0 {
            return Err(Error::Conflict);
        }
        let value = serde_json::to_value(&request.route)?;
        if serde_json::to_vec(&value)?.len() > 16384 {
            return Err(Error::Invalid);
        }
        reserve(&mut tx, 0, 0, i64::from(old.is_none()), 1).await?;
        sqlx::query("INSERT INTO notification_routes(route_id,project_id,workspace_id,recipient_user_id,revision,definition,digest,endpoint_key,endpoint_revision,enabled) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT(route_id) DO UPDATE SET revision=EXCLUDED.revision,definition=EXCLUDED.definition,digest=EXCLUDED.digest,endpoint_key=EXCLUDED.endpoint_key,endpoint_revision=EXCLUDED.endpoint_revision,enabled=EXCLUDED.enabled")
            .bind(request.route.route_id).bind(request.route.scope.project_id).bind(request.route.scope.workspace_id).bind(request.route.recipient_user_id).bind(revision(request.route.revision)?).bind(value).bind(digest(&request.route)?).bind(&request.route.endpoint_key).bind(revision(request.route.endpoint_revision)?).bind(request.route.enabled).execute(&mut *tx).await?;
        let response = json!({"route_id":request.route.route_id,"revision":request.route.revision});
        receipt(&mut tx, actor, request.operation_id, &hash, &response).await?;
        tx.commit().await?;
        Ok(response)
    }
    pub async fn enqueue(&self, actor: Uuid, request: SendRequest) -> Result<Value> {
        if request.ttl_ms == 0 || request.ttl_ms > 604_800_000 {
            return Err(Error::Invalid);
        }
        let hash = digest(&("human-message/1", &request))?;
        let logical = digest(&(
            "human-message-identity/1",
            request.message_id,
            request.revision,
            &request.scope,
            request.recipient_user_id,
            &request.summary,
            request.ttl_ms,
        ))?;
        let mut tx = self.pool.begin().await?;
        lock_budget(&mut tx).await?;
        scope(&mut tx, &request.scope).await?;
        owner(&mut tx, actor).await?;
        recipient(&mut tx, request.recipient_user_id).await?;
        if let Some(response) = replay(&mut tx, actor, request.operation_id, &hash).await? {
            return Ok(response);
        }
        let existing =
            sqlx::query("SELECT logical_digest,revision,external_state FROM notification_messages WHERE message_id=$1")
                .bind(request.message_id)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(existing) = existing {
            if existing.get::<String, _>("logical_digest") != logical {
                return Err(Error::Conflict);
            }
            let ids:Vec<Uuid> = sqlx::query_scalar("SELECT delivery_id FROM notification_deliveries WHERE message_id=$1 ORDER BY sequence")
                .bind(request.message_id).fetch_all(&mut *tx).await?;
            let response = json!({"message_id":request.message_id,"revision":existing.get::<i64,_>("revision"),"external_state":existing.get::<String,_>("external_state"),"delivery_ids":ids});
            reserve(&mut tx, 0, 0, 0, 1).await?;
            receipt(&mut tx, actor, request.operation_id, &hash, &response).await?;
            tx.commit().await?;
            return Ok(response);
        }
        let now = self.now()?;
        let message = Message {
            message_id: request.message_id,
            revision: request.revision,
            scope: request.scope,
            recipient_user_id: request.recipient_user_id,
            summary: request.summary,
            created_at_ms: now,
            expires_at_ms: now.checked_add(request.ttl_ms).ok_or(Error::Invalid)?,
        };
        message.validate()?;
        let rows=sqlx::query("SELECT r.definition FROM notification_routes r JOIN notification_destinations d ON d.endpoint_key=r.endpoint_key AND d.endpoint_revision=r.endpoint_revision WHERE r.project_id=$1 AND r.workspace_id=$2 AND r.recipient_user_id=$3 AND r.enabled AND d.available AND NOT d.disabled ORDER BY r.route_id LIMIT 9")
            .bind(message.scope.project_id).bind(message.scope.workspace_id).bind(message.recipient_user_id).fetch_all(&mut *tx).await?;
        if rows.len() > 8 {
            return Err(Error::Overloaded);
        }
        let mut deliveries = Vec::new();
        for row in rows {
            let route: Route = serde_json::from_value(row.get("definition"))?;
            let payload = Payload::for_message(&message, &self.public_origin)?;
            let d = Delivery::queued(Uuid::new_v4(), message.clone(), route, payload)?;
            if serde_json::to_vec(&d)?.len() > 32768 {
                return Err(Error::Invalid);
            }
            deliveries.push(d);
        }
        reserve(&mut tx, 1, deliveries.len() as i64, 0, 1).await?;
        let external = if deliveries.is_empty() {
            "no_external_route"
        } else {
            "routed"
        };
        sqlx::query("INSERT INTO notification_messages(message_id,revision,project_id,workspace_id,recipient_user_id,definition,digest,external_state,created_at_ms,expires_at_ms,logical_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
            .bind(message.message_id).bind(revision(message.revision)?).bind(message.scope.project_id).bind(message.scope.workspace_id).bind(message.recipient_user_id).bind(serde_json::to_value(&message)?).bind(digest(&message)?).bind(external).bind(now as i64).bind(revision(message.expires_at_ms)?).bind(logical).execute(&mut *tx).await?;
        for d in &deliveries {
            sqlx::query("INSERT INTO notification_deliveries(delivery_id,message_id,route_id,route_revision,endpoint_key,endpoint_revision,state,status,due_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,'queued',$8)")
                .bind(d.delivery_id).bind(message.message_id).bind(d.route.route_id).bind(revision(d.route.revision)?).bind(&d.route.endpoint_key).bind(revision(d.route.endpoint_revision)?).bind(serde_json::to_value(d)?).bind(now as i64).execute(&mut *tx).await?;
        }
        let response = json!({"message_id":message.message_id,"revision":message.revision,"external_state":external,"delivery_ids":deliveries.iter().map(|d|d.delivery_id).collect::<Vec<_>>()});
        receipt(&mut tx, actor, request.operation_id, &hash, &response).await?;
        tx.commit().await?;
        Ok(response)
    }
}

impl Service {
    pub async fn inbox(
        &self,
        actor: Uuid,
        requested_recipient: Uuid,
        after: i64,
        limit: u32,
    ) -> Result<Inbox> {
        if after < 0 || !(1..=64).contains(&limit) {
            return Err(Error::Invalid);
        }
        let mut tx = self.pool.begin().await?;
        let role: Option<String> =
            sqlx::query_scalar("SELECT role FROM users WHERE id=$1 FOR SHARE")
                .bind(actor)
                .fetch_optional(&mut *tx)
                .await?;
        if role.is_none() || (role.as_deref() != Some("owner") && actor != requested_recipient) {
            return Err(Error::Forbidden);
        }
        let rows=sqlx::query("SELECT m.sequence,m.definition,m.external_state FROM notification_messages m JOIN project_execution_bindings p ON p.project_id=m.project_id WHERE m.recipient_user_id=$1 AND m.sequence>$2 AND p.binding->>'graph_workspace_id'=m.workspace_id::text AND p.binding->>'enabled'='true' ORDER BY m.sequence LIMIT $3")
            .bind(requested_recipient).bind(after).bind(i64::from(limit)+1).fetch_all(&mut *tx).await?;
        let more = rows.len() > limit as usize;
        let mut items = Vec::new();
        let mut last = None;
        for row in rows.into_iter().take(limit as usize) {
            let sequence: i64 = row.get("sequence");
            last = Some(sequence);
            items.push(json!({"sequence":sequence,"message":row.get::<Value,_>("definition"),"external_state":row.get::<String,_>("external_state")}));
        }
        tx.commit().await?;
        Ok(Inbox {
            items,
            next_after: if more { last } else { None },
        })
    }
    pub async fn inspect(&self, actor: Uuid, message_id: Uuid) -> Result<Value> {
        let mut tx = self.pool.begin().await?;
        let message: Option<Value> =
            sqlx::query_scalar("SELECT definition FROM notification_messages WHERE message_id=$1")
                .bind(message_id)
                .fetch_optional(&mut *tx)
                .await?;
        let message: Message = serde_json::from_value(message.ok_or(Error::NotFound)?)?;
        message.validate()?;
        scope(&mut tx, &message.scope).await?;
        let role: Option<String> =
            sqlx::query_scalar("SELECT role FROM users WHERE id=$1 FOR SHARE")
                .bind(actor)
                .fetch_optional(&mut *tx)
                .await?;
        if role.is_none()
            || (role.as_deref() != Some("owner") && actor != message.recipient_user_id)
        {
            return Err(Error::NotFound);
        }
        let rows=sqlx::query("SELECT state,record_revision FROM notification_deliveries WHERE message_id=$1 ORDER BY sequence LIMIT 9").bind(message_id).fetch_all(&mut *tx).await?;
        if rows.len() > 8 {
            return Err(Error::Unavailable);
        }
        let mut states = Vec::new();
        for row in rows {
            let d: Delivery = serde_json::from_value(row.get("state"))?;
            d.validate()?;
            if d.message != message {
                return Err(Error::Unavailable);
            }
            states.push(json!({"delivery_id":d.delivery_id,"record_revision":row.get::<i64,_>("record_revision"),"route_id":d.route.route_id,"route_revision":d.route.revision,"status":d.status,"attempts":d.attempts,"duplicate_risk_retries":d.duplicate_risk_retries,"human_acknowledgment":"unacknowledged","decision":"not_applicable"}));
        }
        tx.commit().await?;
        Ok(
            json!({"message":message,"external_state":if states.is_empty(){"no_external_route"}else{"routed"},"deliveries":states}),
        )
    }
}

#[path = "dispatch.rs"]
mod dispatch;

impl Service {
    /// An operator-created retry is a new durable intent, never a replay of the
    /// uncertain HTTP attempt. The frozen route must explicitly permit it too.
    pub async fn retry_unknown(&self, actor: Uuid, request: RetryRequest) -> Result<Value> {
        if !request.accept_duplicate_risk {
            return Err(Error::Invalid);
        }
        let expected = revision(request.expected_revision)?;
        let hash = digest(&("duplicate-risk-retry/1", &request))?;
        let mut tx = self.pool.begin().await?;
        lock_budget(&mut tx).await?;
        let row=sqlx::query("SELECT state,record_revision FROM notification_deliveries WHERE delivery_id=$1 FOR UPDATE").bind(request.delivery_id).fetch_optional(&mut *tx).await?.ok_or(Error::NotFound)?;
        let d: Delivery = serde_json::from_value(row.get("state"))?;
        d.validate()?;
        scope(&mut tx, &d.message.scope).await?;
        owner(&mut tx, actor).await?;
        recipient(&mut tx, d.message.recipient_user_id).await?;
        if let Some(response) = replay(&mut tx, actor, request.operation_id, &hash).await? {
            return Ok(response);
        }
        if row.get::<i64, _>("record_revision") != expected {
            return Err(Error::Conflict);
        }
        let current: Option<Value> = sqlx::query_scalar(
            "SELECT definition FROM notification_routes WHERE route_id=$1 FOR SHARE",
        )
        .bind(d.route.route_id)
        .fetch_optional(&mut *tx)
        .await?;
        if current != Some(serde_json::to_value(&d.route)?) || !d.route.enabled {
            return Err(Error::Conflict);
        }
        let next = d.retry_unknown(self.now()?)?;
        dispatch::save(&mut tx, &next, expected).await?;
        reserve(&mut tx, 0, 0, 0, 1).await?;
        let response = json!({"delivery_id":d.delivery_id,"record_revision":expected+1,"duplicate_risk_retries":next.duplicate_risk_retries,"status":next.status});
        receipt(&mut tx, actor, request.operation_id, &hash, &response).await?;
        tx.commit().await?;
        Ok(response)
    }
}
