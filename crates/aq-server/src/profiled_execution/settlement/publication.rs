//! D1 delivery owner for D3's default completion policy (`delivery =
//! required`). After D3 has accepted a result and holds only on missing
//! delivery evidence, this owner publishes the report's server-confirmed
//! retained source through the project's D1b publication policy, records the
//! server-owned delivery evidence D3 reads, and advances the project's source
//! head. Every aq-git call goes through D1a's durable intents and receipts; a
//! dispatched publication is never sent again.
use super::{
    automatic::{AutoDisposition, AutoSettlement, settle_after_record},
    owner::*,
};
use crate::{
    AppState,
    delivery::{
        CandidateRequest, CandidateState, ChecksState, CommitMetadata, DeliverRequest,
        DeliveryError, DeliveryOutcome, DeliveryService, GitHubAdapter, GitHubError,
        OperationContext, PublicationMode, PublicationPolicy, PullRequest, PullRequestSpec,
        RefusalReason,
    },
    profiled_execution::results,
};
use aq_worker_protocol::{ObjectAlgorithm, ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Mutex};
use uuid::Uuid;

const OWNER: &str = "aq.d1.default-policy-publication@1";
const HELD: &str = "delivery_evidence_missing";
const COMMIT_NAME: &str = "AQ";
const COMMIT_EMAIL: &str = "aq@aq.invalid";

/// The project's integration-branch head as last published by AQ. Recorded in
/// the `published` delivery evidence that established it; the newest
/// `sequence` per project, repository, generation and target is current.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceHead {
    pub schema_version: u32,
    pub project_id: Uuid,
    pub repository_id: Uuid,
    pub binding_generation: u64,
    pub target: String,
    pub sequence: u64,
    /// The published commit (40 lowercase hex digits).
    pub oid: String,
    /// The aq-git candidate retention handle that retains `oid` centrally.
    pub retention_handle: Uuid,
    pub candidate_operation_id: Uuid,
    pub publication_operation_id: Uuid,
    /// The `expected_old` this publication fast-forwarded.
    pub previous: String,
}
impl SourceHead {
    pub fn validate(&self) -> bool {
        let hex = |s: &str| {
            s.len() == 40
                && s.bytes().any(|b| b != b'0')
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        self.schema_version == 1
            && ![
                self.project_id,
                self.repository_id,
                self.retention_handle,
                self.candidate_operation_id,
                self.publication_operation_id,
            ]
            .iter()
            .any(Uuid::is_nil)
            && (1..=i64::MAX as u64).contains(&self.binding_generation)
            && (1..=i64::MAX as u64).contains(&self.sequence)
            && self.target.starts_with("refs/heads/")
            && self.target.len() <= 1024
            && hex(&self.oid)
            && hex(&self.previous)
    }
}

/// The current recorded source head, if AQ has published to this target.
pub async fn current_head<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    project: Uuid,
    repository: Uuid,
    generation: u64,
    target: &str,
) -> Result<Option<SourceHead>> {
    let row: Option<Value> = sqlx::query_scalar(
        "SELECT evidence->'source_head' FROM profiled_delivery_evidence \
         WHERE state='published' AND evidence->'source_head'->>'project_id'=$1 \
         AND evidence->'source_head'->>'repository_id'=$2 \
         AND evidence->'source_head'->>'binding_generation'=$3 \
         AND evidence->'source_head'->>'target'=$4 \
         ORDER BY (evidence->'source_head'->>'sequence')::bigint DESC LIMIT 1",
    )
    .bind(project.to_string())
    .bind(repository.to_string())
    .bind(generation.to_string())
    .bind(target)
    .fetch_optional(executor)
    .await
    .map_err(db)?;
    let Some(value) = row else { return Ok(None) };
    let head: SourceHead = serde_json::from_value(value).map_err(|_| conflict())?;
    if !head.validate() {
        return Err(conflict());
    }
    Ok(Some(head))
}

/// aq-git 0.2.0 cannot create a pull-request head branch and no GitHub
/// credentials are configured (D0/D2): pull-request policies hold explicitly
/// before any call, so no GitHub client is bound.
struct NoGitHub;
#[async_trait::async_trait]
impl GitHubAdapter for NoGitHub {
    async fn open_pull_request(
        &self,
        _: &PullRequestSpec,
    ) -> std::result::Result<PullRequest, GitHubError> {
        Err(GitHubError("not configured".into()))
    }
    async fn checks(&self, _: &PullRequest) -> std::result::Result<ChecksState, GitHubError> {
        Err(GitHubError("not configured".into()))
    }
}

/// One in-process owner per report: a duplicate result POST never races a
/// running publication into a second evidence write.
static IN_FLIGHT: Mutex<Option<HashSet<Uuid>>> = Mutex::new(None);
struct Flight(Uuid);
impl Flight {
    fn take(report: Uuid) -> Option<Self> {
        let mut set = IN_FLIGHT.lock().ok()?;
        set.get_or_insert_with(HashSet::new)
            .insert(report)
            .then_some(Self(report))
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        if let Ok(mut set) = IN_FLIGHT.lock()
            && let Some(set) = set.as_mut()
        {
            set.remove(&self.0);
        }
    }
}

/// True when D3 holds this settlement only on missing delivery evidence.
pub fn awaits_delivery(s: &AutoSettlement) -> bool {
    s.disposition == AutoDisposition::Held && s.reason.as_deref() == Some(HELD)
}

/// Publish a held result and let D3 re-evaluate it. Idempotent per report.
/// The caller runs this detached from any client request, so a dropped
/// request can never cancel a publication between dispatch and receipt.
pub async fn deliver_then_settle(
    state: &AppState,
    delivery: &DeliveryService,
    report_id: Uuid,
) -> Result<Option<AutoSettlement>> {
    let Some(_flight) = Flight::take(report_id) else {
        return Ok(None);
    };
    let Some(settled) = settle_after_record(state, report_id).await? else {
        return Ok(None);
    };
    if !awaits_delivery(&settled) {
        return Ok(Some(settled));
    }
    if publish(state, delivery, report_id).await? {
        return settle_after_record(state, report_id).await;
    }
    Ok(Some(settled))
}

fn sha1(hex: &str) -> ObjectId {
    ObjectId {
        algorithm: ObjectAlgorithm::Sha1,
        hex: hex.to_owned(),
    }
}

enum Recorded {
    Published {
        outcome: Value,
        head: Option<String>,
    },
    Unpublished(Value),
    Indeterminate(Value),
}

/// Returns true once delivery evidence for the report's source is recorded.
async fn publish(state: &AppState, delivery: &DeliveryService, report_id: Uuid) -> Result<bool> {
    let mut tx = state.store.pool.begin().await.map_err(db)?;
    let r = results::load_locked(state, &mut tx, report_id).await?;
    let mut source = None;
    for op in &r.report.source_receipt_ids {
        match crate::source_retention::load_confirmed_locked(&mut tx, *op, &r).await {
            Ok(confirmed) => {
                source = Some((*op, confirmed));
                break;
            }
            Err(crate::error::AppError::Conflict(_)) => {}
            Err(e) => return Err(e),
        }
    }
    tx.rollback().await.map_err(db)?;
    let Some((source_op, confirmed)) = source else {
        return Ok(false);
    };
    let admitted = &r.spec.source;
    let (project, repository, generation) = (
        admitted.project_id,
        admitted.repository_id,
        admitted.repository_binding_generation,
    );
    let binding = crate::project_bindings::current(state, project)
        .await?
        .binding;
    let Some(policy) = binding.publication.clone() else {
        tracing::info!(%report_id, %project, "delivery required but the project names no publication policy; held");
        return Ok(false);
    };
    if !binding.enabled
        || binding.repository_id != repository
        || binding.repository_binding_generation != generation
        || confirmed.manifest.source.repository_id != repository
        || confirmed.manifest.source.binding_generation != generation
    {
        tracing::warn!(%report_id, %project, "publication binding differs from the admitted source; held");
        return Ok(false);
    }
    if policy.mode == PublicationMode::PullRequest {
        tracing::warn!(%report_id, %project, "pull_request publication is unsupported with aq-git 0.2.0 (D0/D2); held, never degraded");
        return Ok(false);
    }
    let candidate_op = Uuid::new_v5(&report_id, b"aq.d1.default-policy.candidate");
    let publish_op = Uuid::new_v5(&report_id, b"aq.d1.default-policy.publish");
    let context = |operation_id| OperationContext {
        operation_id,
        repository_id: repository,
        binding_generation: generation,
    };
    // Candidate base and expected_old are one OID: the recorded source head of
    // the target, or before any publication the commit the attempt was
    // admitted on. A retry replays the base frozen in the first intent.
    let base = match delivery
        .frozen_inputs(candidate_op)
        .await
        .map_err(delivery_error)?
    {
        Some(inputs) => {
            serde_json::from_value::<ObjectId>(inputs["base"].clone()).map_err(|_| conflict())?
        }
        None => match current_head(
            &state.store.pool,
            project,
            repository,
            generation,
            &policy.target,
        )
        .await?
        {
            Some(head) => sha1(&head.oid),
            None => sha1(&admitted.resolved_object.hex),
        },
    };
    let created = delivery
        .create_candidate(CandidateRequest {
            context: context(candidate_op),
            base: base.clone(),
            source_receipts: vec![source_op],
            commit: CommitMetadata {
                name: COMMIT_NAME.into(),
                email: COMMIT_EMAIL.into(),
                message: format!(
                    "AQ: task {} result {}\n\nRetained checkpoint {} published by {}.",
                    r.spec.task.task_id, report_id, confirmed.manifest.checkpoint.hex, OWNER
                ),
            },
        })
        .await;
    let candidate = match created {
        Ok(receipt) => match receipt.outcome {
            Ok(c) if c.state == CandidateState::Ready => match c.candidate {
                Some(candidate) => candidate,
                None => return Err(conflict()),
            },
            Ok(c) => {
                let evidence = json!({"reason":"conflicted","conflict_step":c.conflict_step,"conflict_path_count":c.conflict_path_count});
                return record(
                    state,
                    &r,
                    source_op,
                    &policy,
                    &base,
                    candidate_op,
                    publish_op,
                    None,
                    Recorded::Unpublished(evidence),
                )
                .await;
            }
            Err(e) => {
                tracing::warn!(%report_id, code = %e.code, "candidate creation failed; held");
                return Ok(false);
            }
        },
        Err(e) => {
            tracing::warn!(%report_id, error = %e, "candidate creation unavailable; held");
            return Ok(false);
        }
    };
    let outcome = match delivery
        .deliver(
            &policy,
            DeliverRequest {
                context: context(publish_op),
                candidate_id: candidate_op,
                expected_old: base.clone(),
            },
            &NoGitHub,
        )
        .await
    {
        Ok(o) => Ok(o),
        // A reply that fails validation after dispatch proves nothing about
        // the remote: D1a keeps the destination held, and so is it recorded.
        Err(DeliveryError::Protocol) => Err(Recorded::Indeterminate(
            json!({"reason":"invalid_publish_reply","operation_id":publish_op,"next":"operation.reconcile under a new operation ID; never a re-push"}),
        )),
        Err(e) => {
            tracing::warn!(%report_id, error = %e, "publication unavailable; held");
            return Ok(false);
        }
    };
    let recorded = match outcome {
        Err(recorded) => recorded,
        Ok(outcome) => match outcome {
            DeliveryOutcome::Published {
                old,
                new,
                acknowledged_at,
                ..
            } => Recorded::Published {
                outcome: json!({"outcome":"applied","old":old,"new":new,"acknowledged_at":acknowledged_at}),
                head: Some(new.hex),
            },
            DeliveryOutcome::ObservedPresent {
                observed_target,
                observed_at,
                ..
            } => Recorded::Published {
                // Observed presence, never a claim that this operation caused it.
                head: (observed_target == candidate).then(|| candidate.hex.clone()),
                outcome: json!({"outcome":"already_present","observed_target":observed_target,"observed_at":observed_at,"caused_by_this_operation":false}),
            },
            DeliveryOutcome::Refused(refusal)
                if refusal.reason == RefusalReason::PullRequestUnsupported =>
            {
                return Ok(false);
            }
            DeliveryOutcome::Refused(refusal) => {
                Recorded::Unpublished(serde_json::to_value(refusal).map_err(|_| conflict())?)
            }
            DeliveryOutcome::Ambiguous { operation_id } => Recorded::Indeterminate(
                json!({"reason":"operation_indeterminate","operation_id":operation_id,"next":"operation.reconcile under a new operation ID; never a re-push"}),
            ),
            DeliveryOutcome::DestinationBlocked => {
                tracing::info!(%report_id, "publication destination is held by another operation; held");
                return Ok(false);
            }
            DeliveryOutcome::RecoveryBlocked(blocked) => {
                tracing::warn!(%report_id, message = %blocked.message, "aq-git recovery_blocked; held");
                return Ok(false);
            }
            DeliveryOutcome::Failed { error, .. } => {
                tracing::warn!(%report_id, code = %error.code, "publication failed before dispatch; held");
                return Ok(false);
            }
        },
    };
    record(
        state,
        &r,
        source_op,
        &policy,
        &base,
        candidate_op,
        publish_op,
        Some(&candidate),
        recorded,
    )
    .await
}

fn delivery_error(_: DeliveryError) -> crate::error::AppError {
    unavailable()
}

#[allow(clippy::too_many_arguments)]
async fn record(
    state: &AppState,
    r: &results::RecordedResult,
    source_op: Uuid,
    policy: &PublicationPolicy,
    base: &ObjectId,
    candidate_op: Uuid,
    publish_op: Uuid,
    candidate: Option<&ObjectId>,
    recorded: Recorded,
) -> Result<bool> {
    let admitted = &r.spec.source;
    let time = now(state)?;
    let mut tx = state.store.pool.begin().await.map_err(db)?;
    // Serializes head advancement per repository target.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,57))")
        .bind(format!(
            "aq-d1-source-head:{}:{}",
            admitted.repository_id, policy.target
        ))
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    let (state_name, outcome, head) = match recorded {
        Recorded::Published { outcome, head } => ("published", outcome, head),
        Recorded::Unpublished(v) => ("unpublished", v, None),
        Recorded::Indeterminate(v) => ("indeterminate", v, None),
    };
    // Compare-and-swap: advance only from the exact expected_old published
    // over; otherwise the evidence stands and the head is left unchanged.
    let source_head = match head {
        Some(oid) => {
            let current = current_head(
                &mut *tx,
                admitted.project_id,
                admitted.repository_id,
                admitted.repository_binding_generation,
                &policy.target,
            )
            .await?;
            if current.as_ref().is_none_or(|h| h.oid == base.hex) {
                let head = SourceHead {
                    schema_version: 1,
                    project_id: admitted.project_id,
                    repository_id: admitted.repository_id,
                    binding_generation: admitted.repository_binding_generation,
                    target: policy.target.clone(),
                    sequence: current.map_or(1, |h| h.sequence + 1),
                    oid,
                    retention_handle: candidate_op,
                    candidate_operation_id: candidate_op,
                    publication_operation_id: publish_op,
                    previous: base.hex.clone(),
                };
                if !head.validate() {
                    return Err(conflict());
                }
                Some(head)
            } else {
                tracing::warn!(report_id = %r.report.report_id, "source head moved since this publication's expected_old; head not advanced");
                None
            }
        }
        None => None,
    };
    let evidence = json!({
        "owner": OWNER,
        "report_id": r.report.report_id,
        "policy": policy,
        "candidate_operation_id": candidate_op,
        "candidate": candidate,
        "publication_operation_id": publish_op,
        "expected_old": base,
        "result": outcome,
        "source_head": source_head,
    });
    let inserted = sqlx::query("INSERT INTO profiled_delivery_evidence(attempt_id,source_operation_id,state,evidence,recorded_at_ms) VALUES($1,$2,$3,$4,$5) ON CONFLICT(attempt_id,source_operation_id) DO NOTHING")
        .bind(r.report.binding.attempt_id).bind(source_op).bind(state_name).bind(&evidence).bind(time)
        .execute(&mut *tx).await.map_err(db)?.rows_affected();
    tx.commit().await.map_err(db)?;
    let attempt = r.report.binding.attempt_id;
    if inserted == 1 {
        tracing::info!(report_id = %r.report.report_id, %attempt, state = state_name, "delivery evidence recorded");
    }
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM profiled_delivery_evidence WHERE attempt_id=$1 AND source_operation_id=$2)")
        .bind(attempt)
        .bind(source_op)
        .fetch_one(&state.store.pool)
        .await
        .map_err(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn head() -> SourceHead {
        SourceHead {
            schema_version: 1,
            project_id: Uuid::from_u128(1),
            repository_id: Uuid::from_u128(2),
            binding_generation: 1,
            target: "refs/heads/main".into(),
            sequence: 1,
            oid: "a".repeat(40),
            retention_handle: Uuid::from_u128(3),
            candidate_operation_id: Uuid::from_u128(3),
            publication_operation_id: Uuid::from_u128(4),
            previous: "b".repeat(40),
        }
    }
    #[test]
    fn source_head_shape_is_exact() {
        assert!(head().validate());
        let mut h = head();
        h.oid = "0".repeat(40);
        assert!(!h.validate());
        let mut h = head();
        h.target = "refs/tags/v1".into();
        assert!(!h.validate());
        let mut h = head();
        h.sequence = 0;
        assert!(!h.validate());
        let mut h = head();
        h.retention_handle = Uuid::nil();
        assert!(!h.validate());
        let mut v = serde_json::to_value(head()).unwrap();
        v["extra"] = json!(1);
        assert!(serde_json::from_value::<SourceHead>(v).is_err());
    }
}
