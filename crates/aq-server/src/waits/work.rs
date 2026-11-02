//! Finite physical work, separate from durable leases. No external IO occurs
//! while SQL locks are held. A canceled async waiter cannot free a live callback.
use super::*;
use std::{
    collections::BTreeSet,
    sync::{Mutex, OnceLock},
    time::Duration,
};
struct Activity {
    wait: Uuid,
    active: Arc<Mutex<BTreeSet<Uuid>>>,
    _permit: tokio::sync::OwnedSemaphorePermit,
}
impl Drop for Activity {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            active.remove(&self.wait);
        }
    }
}
fn physical() -> Arc<tokio::sync::Semaphore> {
    static CELL: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(2)))
        .clone()
}
fn activities() -> Arc<Mutex<BTreeSet<Uuid>>> {
    static CELL: OnceLock<Arc<Mutex<BTreeSet<Uuid>>>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(Mutex::new(BTreeSet::new())))
        .clone()
}
pub(super) fn limits() -> (Arc<tokio::sync::Semaphore>, Arc<Mutex<BTreeSet<Uuid>>>) {
    (physical(), activities())
}
impl Owner {
    fn activity(&self, wait: Uuid) -> Result<Activity> {
        let permit = self
            .physical
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Capacity)?;
        let mut active = self.active.lock().map_err(|_| Error::Corrupt)?;
        if !active.insert(wait) {
            return Err(Error::Capacity);
        }
        drop(active);
        Ok(Activity {
            wait,
            active: self.active.clone(),
            _permit: permit,
        })
    }
    /// Only actual clock observations are enabled in the initial production
    /// profile. Graph/mail adapters require separately qualified source joins.
    pub async fn drive_clock(
        &self,
        node: &AuthenticatedNode,
        attempt: Uuid,
        wait: Uuid,
    ) -> Result<Observation> {
        self.run(
            node,
            attempt,
            wait,
            Box::new(Clock {
                clock: self.state.clock.clone(),
            }),
            false,
        )
        .await
    }
    #[cfg(feature = "test-support")]
    pub async fn drive_fixture(
        &self,
        node: &AuthenticatedNode,
        attempt: Uuid,
        wait: Uuid,
        port: Box<dyn engine::SourceOwnerPort + Send>,
    ) -> Result<Observation> {
        if !self.fixture_sources {
            return Err(Error::Unsupported);
        }
        self.run(node, attempt, wait, port, true).await
    }
    /// Event delivery is only a dirty hint. It supplies no verdict or watermark.
    pub async fn hint(
        &self,
        node: &AuthenticatedNode,
        attempt: Uuid,
        wait: Uuid,
        member: Uuid,
    ) -> Result<Observation> {
        let (owner, bound_attempt) = self.peek(wait).await?;
        if owner != node.id() || bound_attempt != attempt {
            return Err(Error::NotFound);
        }
        let before = self.preflight(node, attempt, true).await?;
        let mut tx = self.transaction().await?;
        self.gates(&mut tx, &before.anchor, false).await?;
        let mut current = self.current(&mut tx, node, attempt, true).await?;
        self.mailbox_head(&mut tx, &current.anchor).await?;
        let stored = self.load(&mut tx, wait, true).await?;
        current.refresh(&self.state)?;
        same_owner(&stored.original.anchor, &current.anchor)?;
        current.now = current
            .now
            .max(stored.observation.snapshot.logical_now_ms());
        let plan = stored.observation.snapshot.step(
            engine::Input::Hint { member_id: member },
            &current.fence,
            current.now,
        )?;
        self.save(&mut tx, &stored, &plan.next, &current).await?;
        tx.commit().await?;
        Ok(Observation {
            registration: stored.observation.registration,
            snapshot: plan.next,
            retired: stored.observation.retired || current.cancellation.is_some(),
        })
    }
    async fn run(
        &self,
        node: &AuthenticatedNode,
        attempt: Uuid,
        wait: Uuid,
        mut port: Box<dyn engine::SourceOwnerPort + Send>,
        fixture: bool,
    ) -> Result<Observation> {
        let activity = self.activity(wait)?;
        let (owner, bound_attempt) = self.peek(wait).await?;
        if owner != node.id() || bound_attempt != attempt {
            return Err(Error::NotFound);
        }
        let before = self.preflight(node, attempt, true).await?;
        let mut tx = self.transaction().await?;
        self.gates(&mut tx, &before.anchor, false).await?;
        let mut current = self.current(&mut tx, node, attempt, true).await?;
        self.mailbox_head(&mut tx, &current.anchor).await?;
        let mut stored = self.load(&mut tx, wait, true).await?;
        current.refresh(&self.state)?;
        same_owner(&stored.original.anchor, &current.anchor)?;
        current.now = current
            .now
            .max(stored.observation.snapshot.logical_now_ms());
        let snapshot = &stored.observation.snapshot;
        if snapshot.outcome().is_some()
            || current.cancellation.is_some()
            || current.now >= snapshot.deadline_ms()
            || stored.original.anchor.source_access != current.anchor.source_access
        {
            let plan = snapshot.step(engine::Input::Drive, &current.fence, current.now)?;
            self.save(&mut tx, &stored, &plan.next, &current).await?;
            tx.commit().await?;
            return Ok(Observation {
                registration: stored.observation.registration,
                snapshot: plan.next,
                retired: stored.observation.retired || current.cancellation.is_some(),
            });
        }
        if let Some(expiry) = stored.lease_expires {
            if current.now < expiry {
                return Err(Error::Stale);
            }
            let ticket = snapshot.pending_work().ok_or(Error::Corrupt)?.0;
            let plan = snapshot.step(
                engine::Input::Reclaim { ticket },
                &current.fence,
                current.now,
            )?;
            self.save(&mut tx, &stored, &plan.next, &current).await?;
            tx.commit().await?;
            return Ok(Observation {
                registration: stored.observation.registration,
                snapshot: plan.next,
                retired: false,
            });
        }
        if snapshot.pending_work().is_none() {
            let plan = snapshot.step(engine::Input::Drive, &current.fence, current.now)?;
            self.save(&mut tx, &stored, &plan.next, &current).await?;
            stored = self.load(&mut tx, wait, true).await?;
        }
        let Some((ticket, sources)) = stored.observation.snapshot.pending_work() else {
            tx.commit().await?;
            return Ok(stored.observation);
        };
        if !fixture
            && sources
                .iter()
                .any(|s| !matches!(s.member.condition, wire::Condition::Timer { .. }))
        {
            return Err(Error::Unsupported);
        }
        let expiry = current
            .now
            .checked_add(10_000)
            .ok_or(Error::Capacity)?
            .min(stored.observation.snapshot.deadline_ms());
        if expiry <= current.now {
            return Err(Error::Stale);
        }
        let lease = Uuid::new_v4();
        sqlx::query("UPDATE wait_owner_work SET state='leased',lease_id=$2,lease_expires_ms=$3,transition_at_ms=$4 WHERE wait_id=$1 AND lease_id IS NULL")
            .bind(wait).bind(lease).bind(expiry as i64).bind(current.now as i64).execute(&mut *tx).await?;
        tx.commit().await?;
        let kind = ticket.kind.clone();
        let job = tokio::task::spawn_blocking(move || {
            // Activity/permit is owned by this physical callback and its result,
            // never just the cancelable async wait. There is no admission queue.
            let replies = sources
                .iter()
                .map(|source| match kind {
                    engine::WorkKind::Install => port.install(source),
                    engine::WorkKind::Reobserve => port.reobserve(source),
                })
                .collect::<engine::Result<Vec<_>>>();
            (replies, activity)
        });
        let (replies, _activity) =
            tokio::time::timeout(Duration::from_millis(expiry - current.now), job)
                .await
                .map_err(|_| Error::Stale)?
                .map_err(|_| Error::Corrupt)?;
        let replies = replies?;
        let mut tx = self.transaction().await?;
        self.gates(&mut tx, &before.anchor, false).await?;
        let mut current = self.current(&mut tx, node, attempt, true).await?;
        self.mailbox_head(&mut tx, &current.anchor).await?;
        let stored = self.load(&mut tx, wait, true).await?;
        current.refresh(&self.state)?;
        same_owner(&stored.original.anchor, &current.anchor)?;
        current.now = current
            .now
            .max(stored.observation.snapshot.logical_now_ms());
        if stored.lease != Some(lease)
            || stored.lease_expires != Some(expiry)
            || current.now >= expiry
        {
            return Err(Error::Stale);
        }
        let input = engine::Input::SourceBatch {
            ticket: ticket.clone(),
            replies,
        };
        let mut plan = stored
            .observation
            .snapshot
            .step(input, &current.fence, current.now)?;
        if Self::bounded(&plan.next).is_err() {
            // Refuse the entire oversized observation; retain the prior bounded
            // projection and use bounded retry/RecoveryHeld, not terminal space.
            plan = stored.observation.snapshot.step(
                engine::Input::Reclaim { ticket },
                &current.fence,
                current.now,
            )?;
        }
        self.save(&mut tx, &stored, &plan.next, &current).await?;
        tx.commit().await?;
        Ok(Observation {
            registration: stored.observation.registration,
            snapshot: plan.next,
            retired: stored.observation.retired || current.cancellation.is_some(),
        })
    }
}
pub(super) fn same_owner(original: &Anchor, current: &Anchor) -> Result<()> {
    if original.node_id != current.node_id
        || original.attempt_id != current.attempt_id
        || original.scope != current.scope
        || original.binding != current.binding
        || original.principal != current.principal
    {
        return Err(Error::Authority);
    }
    Ok(())
}
struct Clock {
    clock: crate::Clock,
}
impl Clock {
    fn observe(&self, source: &engine::SourceRequest) -> engine::Result<engine::SourceReply> {
        let wire::Condition::Timer { due_at_ms } = source.member.condition else {
            return Err(engine::Error::Proof);
        };
        let now = (self.clock)().timestamp_millis().max(1) as u64;
        // A clock owner has no allocated SQL sequence. The actual sampled time
        // is its monotonic condition basis; the reducer separately fixes age.
        let at = source
            .previous
            .as_ref()
            .and_then(|b| b.committed_revision.as_str().parse::<u64>().ok())
            .unwrap_or(1)
            .max(now);
        let condition = wire::canonical_digest(&source.member.condition)?;
        let owner_bytes = wire::canonical_bytes(&(
            "aq.wait.clock-observation.1",
            &source.scope,
            &source.binding,
            source.member.member_id,
            due_at_ms,
            at,
        ))?;
        let reference = wire::ArtifactRef {
            id: wire::Text::new(format!(
                "aq-clock/{}/{}",
                source.subscription.wait.wait_id, source.member.member_id
            ))?,
            version: wire::Text::new(at.to_string())?,
            digest: wire::Digest::of(&owner_bytes),
        };
        let revision = wire::Text::new(at.to_string())?;
        let observation = wire::MemberOutcome {
            member_id: source.member.member_id,
            state: if at >= due_at_ms {
                wire::MemberState::Satisfied
            } else {
                wire::MemberState::Pending
            },
            evidence: Some(wire::SourceEvidence {
                owner: wire::SourceOwner::Clock,
                condition_digest: condition.clone(),
                revision: revision.clone(),
                result: reference.clone(),
            }),
        };
        let boundary = wire::SourceBoundary {
            member_id: source.member.member_id,
            owner: wire::SourceOwner::Clock,
            condition_digest: condition,
            committed_revision: revision.clone(),
            committed_replay_cursor: revision,
            subscription_receipt: wire::ArtifactRef {
                id: wire::Text::new(format!(
                    "aq-clock-subscription/{}/{}",
                    source.subscription.wait.wait_id, source.member.member_id
                ))?,
                version: wire::Text::new("1")?,
                digest: source.subscription.digest()?,
            },
            snapshot_digest: reference.digest.clone(),
        };
        Ok(engine::SourceReply::Committed(Box::new(
            engine::CommittedObservation {
                protocol: engine::SourceProtocol::CommittedReplay,
                subscription: source.subscription.clone(),
                scope: source.scope.clone(),
                binding: source.binding.clone(),
                principal: source.principal.clone(),
                source_access: source.source_access.clone(),
                incarnation: wire::Text::new("aq-clock-v1")?,
                boundary,
                owner_snapshot: reference,
                projection_digest: wire::canonical_digest(&observation)?,
                replay_from: source
                    .previous
                    .as_ref()
                    .map(|b| b.committed_replay_cursor.clone()),
                observation,
            },
        )))
    }
}
impl engine::SourceOwnerPort for Clock {
    fn install(&mut self, s: &engine::SourceRequest) -> engine::Result<engine::SourceReply> {
        self.observe(s)
    }
    fn reobserve(&mut self, s: &engine::SourceRequest) -> engine::Result<engine::SourceReply> {
        self.observe(s)
    }
}
