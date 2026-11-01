//! Bounded immutable input owner for a fresh phase execution confirmation.
use super::*;
use crate::{
    execution::{
        authority::{self, Policy, hot::HotState},
        profiled::Status,
    },
    profiled_execution::{
        accepted_preparation::{self, AcceptedHistory},
        artifacts, registration, source_exports,
    },
    worker_api::AuthenticatedNode,
};
use aq_admission::{
    Text, control::Versions, preparation_v2::PrepareRequest, transport::ArtifactReceipt,
};
use sqlx::Row;
use uuid::Uuid;
pub(super) struct Pinned {
    pub history: AcceptedHistory,
    pub input: observations::Input,
    pub policy: aq_profiles::SelectionPolicy,
    pub capabilities: Vec<aq_session_protocol::Capability>,
    pub hot: HotState,
    pub versions: Versions,
    pub registration_id: Uuid,
    pub qualification: ArtifactRef,
    pub issuance_ceiling: u64,
}
pub(super) async fn load_locked(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    auth: &AuthenticatedNode,
    q: &PrepareRequest,
) -> Result<Pinned, AppError> {
    use aq_session_protocol::Validate;
    q.original.binding.validate().map_err(|_| refused())?;
    if auth.id() != q.original.identity.node_id
        || !matches!(q.original.binding.session,aq_admission::SessionIdentity::Remote{node_id,boot_id,..} if node_id==auth.id()&&boot_id==q.original.identity.boot_id)
    {
        return Err(refused());
    }
    crate::worker_api::lock_current_node(tx, auth).await?;
    let h = accepted_preparation::load_accepted_history_locked(tx, &q.original.binding).await?;
    q.validate_for(&h.receipt, &h.request, &h.proposal)
        .map_err(|_| refused())?;
    accepted_preparation::require_live_accepted_locked(state, tx, auth, &h).await?;
    let runtime =
        registration::current_runtime_locked(state, tx, auth.id(), q.original.identity.boot_id)
            .await?;
    let qualification = policy::current(state, &h, &runtime)?;
    let current_registration =
        registration::admitted_runtime_locked(tx, auth.id(), q.original.identity.boot_id).await?;
    if current_registration.request.identity != q.original.identity {
        return Err(refused());
    }
    let registration =
        super::super::closure::history::for_attempt_locked(tx, &q.original.binding).await?;
    if registration.metadata.semantic_digest != current_registration.semantic_digest
        || registration.metadata.request.identity != q.original.identity
    {
        return Err(refused());
    }
    let view = authority::load_locked(state, tx, &q.original.binding.session, Policy::IdentityOnly)
        .await?;
    super::super::closure::ensure_launch_open_locked(tx, &q.original.binding).await?;
    if !view.holds_task
        || view.status != Status::Preparing
        || view.grant_id.is_some()
        || view.cancellation_id.is_some()
        || view.revoked
        || view.stopped
    {
        return Err(refused());
    }
    let row=sqlx::query("SELECT state,record_revision,authority_revision,grant_revision FROM profiled_authority_hot WHERE attempt_id=$1 FOR UPDATE").bind(q.original.binding.attempt_id).fetch_one(&mut **tx).await.map_err(db)?;
    let hot: HotState = serde_json::from_value(row.get("state")).map_err(|_| refused())?;
    hot.validate_against(&q.original.binding)
        .map_err(|_| refused())?;
    let versions = Versions {
        record_revision: row.get::<i64, _>("record_revision") as u64,
        authority_revision: row.get::<i64, _>("authority_revision") as u64,
        grant_revision: row.get::<i64, _>("grant_revision") as u64,
    };
    versions
        .validate_after(&h.receipt.versions)
        .map_err(|_| refused())?;
    if hot.grant.is_some() || hot.execution_deadline_ms.is_some() {
        return Err(refused());
    }
    let offer =
        super::super::native_offer::load_original_offer_locked(tx, q.original.binding.attempt_id)
            .await?;
    if !same(&offer, &h.proposal.original)? {
        return Err(refused());
    }
    // Protocol/host capabilities belong to the actual worker runtime. Installed
    // harness entries separately prove the selected harness/model configuration.
    let qualified = state
        .config
        .profiled_execution
        .as_ref()
        .and_then(|c| {
            c.qualified_runtimes
                .iter()
                .find(|r| r.runtime == runtime.runtime && r.qualification == runtime.qualification)
        })
        .ok_or_else(refused)?;
    if qualified.material_delivery.is_none()
        || offer.offered.required_features.iter().any(|f| {
            !qualified.features.contains(f) || !current_registration.request.features.contains(f)
        })
    {
        return Err(refused());
    }
    for caps in [
        &qualified.capabilities,
        &current_registration.request.capabilities,
    ] {
        aq_session_protocol::require_capabilities(&offer.offered.offer.required_capabilities, caps)
            .map_err(|_| refused())?;
        aq_session_protocol::require_capabilities(
            &offer.offered.offer.manifest.harness.required_capabilities,
            caps,
        )
        .map_err(|_| refused())?;
    }

    let row=sqlx::query("SELECT l.execution_spec,l.execution_spec_digest,l.graph_task_observation,l.decision_id,d.decision,s.selection,s.selection_digest FROM profiled_launches l JOIN intelligence_decisions d USING(decision_id) JOIN intelligence_attempt_selections s USING(attempt_id) WHERE l.attempt_id=$1 FOR SHARE OF l,d,s").bind(q.original.binding.attempt_id).fetch_one(&mut **tx).await.map_err(db)?;
    let spec: aq_admission::execution_spec::ProfiledExecutionSpec =
        serde_json::from_value(row.get("execution_spec")).map_err(|_| refused())?;
    if !same(&spec, &h.proposal.spec)?
        || spec.artifact().map_err(|_| refused())?.digest
            != q.original.binding.execution_spec_digest
        || row.get::<String, _>("execution_spec_digest")
            != q.original.binding.execution_spec_digest.as_str()
    {
        return Err(refused());
    }
    let stored: crate::profiled_execution::native_offer::observation::StoredOffer =
        serde_json::from_value(row.get("graph_task_observation")).map_err(|_| refused())?;
    if stored.read_provenance.context_response_digest != h.proposal.full_context_digest
        || !same(&stored.native_offer, &h.proposal.original)?
    {
        return Err(refused());
    }
    let decision: crate::intelligence::resolver::Decision =
        serde_json::from_value(row.get("decision")).map_err(|_| refused())?;
    let selection: aq_selection::ResolvedSelection =
        serde_json::from_value(row.get("selection")).map_err(|_| refused())?;
    if decision.decision_id != row.get::<Uuid, _>("decision_id")
        || decision
            .outcome
            .resolved
            .as_ref()
            .is_none_or(|s| !same(s, &selection).unwrap_or(false))
        || digest(&selection)? != spec.selection_digest
        || spec.selection_digest != h.proposal.choice.selection_digest
        || row.get::<String, _>("selection_digest") != spec.selection_digest.as_str()
        || selection.target
            != (aq_selection::ExecutionTarget::Worker {
                node_id: auth.id(),
                boot_id: q.original.identity.boot_id,
            })
        || selection.lineage.project_id != view.project_id
        || selection.lineage.task_id != view.task_id
        || selection.lineage.workspace_id != spec.task.workspace_id
    {
        return Err(refused());
    }
    let material = h.receipt.lease.materials[0].clone();
    if material.credential_profile != selection.choice.credential_profile
        || material.credential_revision != selection.credential_revision
        || material.provider.as_str() != selection.choice.provider
        || material.account_reference.as_str() != selection.choice.account_reference
        || material.endpoint_reference.as_str() != selection.choice.endpoint_reference
    {
        return Err(refused());
    }
    let source = source_exports::load_linked_locked(tx, q.original.binding.attempt_id).await?;
    if source.manifest().attempt != view.attempt
        || source.manifest().source != view.source.ok_or_else(refused)?
        || source.manifest().resolved_spec_digest != view.execution_spec_digest
        || source.token().request().project_revision != spec.project_revision
        || source.token().request().project_id != view.project_id
    {
        return Err(refused());
    }
    let mut expected = Vec::new();
    let mut context = None;
    let mut ceiling = h.receipt.lease.expires_at_ms.min(material.expires_at_ms);
    let ids:Vec<Uuid>=sqlx::query_scalar("SELECT artifact_id FROM profiled_launch_artifacts WHERE attempt_id=$1 ORDER BY artifact_id LIMIT 127").bind(q.original.binding.attempt_id).fetch_all(&mut **tx).await.map_err(db)?;
    if ids.len() != offer.offered.artifacts.len() || ids.len() > 126 {
        return Err(refused());
    }
    let time = now(state)?;
    for id in ids {
        let a = artifacts::load_locked(tx, id).await?;
        if a.project_id != view.project_id
            || !offer
                .offered
                .artifacts
                .iter()
                .any(|d| same(d, &a.descriptor).unwrap_or(false))
        {
            return Err(refused());
        }
        a.descriptor.verify(&a.bytes, time).map_err(|_| refused())?;
        ceiling = ceiling.min(a.descriptor.retain_until_ms);
        if a.descriptor.artifact == spec.task_context {
            if context.is_some() {
                return Err(refused());
            }
            context = Some(serde_json::from_slice(&a.bytes).map_err(|_| refused())?);
        }
        expected.push(ArtifactReceipt {
            artifact: a.descriptor.artifact.clone(),
            bytes: a.bytes.len() as u64,
            digest: a.descriptor.artifact.digest,
        });
    }
    for (name, hash, size) in [
        (
            "manifest",
            source.manifest().manifest_digest.as_str(),
            source.manifest().manifest_bytes,
        ),
        (
            "bundle",
            source
                .manifest()
                .bundle_digest
                .as_deref()
                .ok_or_else(refused)?,
            source.manifest().bundle_bytes,
        ),
    ] {
        let d: Digest = serde_json::from_value(serde_json::json!(hash)).map_err(|_| refused())?;
        expected.push(ArtifactReceipt {
            artifact: ArtifactRef {
                id: Text::new(format!(
                    "aq.source-{name}/{}",
                    source.manifest().transfer_id
                ))
                .map_err(|_| refused())?,
                version: Text::new("1").map_err(|_| refused())?,
                digest: d.clone(),
            },
            bytes: size,
            digest: d,
        });
    }
    if expected.len() != q.original.artifacts.len()
        || expected.iter().any(|e| {
            !q.original
                .artifacts
                .iter()
                .any(|a| same(e, a).unwrap_or(false))
        })
        || time >= ceiling
    {
        return Err(refused());
    }
    let (policy, _) =
        super::super::continuation_policy::load_locked(tx, &q.original.binding).await?;
    if policy.max_lineage_elapsed_ms != h.proposal.choice.maximum_lineage_elapsed_ms
        || offer.offered.offer.manifest.model.spending_budget.is_some()
        || policy.budget.max_cost_microunits.is_some()
        || policy.budget.require_complete_accounting
        || policy.budget.require_hard_cost_enforcement
        || decision
            .project
            .project
            .binding
            .ceilings
            .cost_limit_microunits
            .is_some()
        || decision
            .project
            .project
            .binding
            .ceilings
            .require_hard_cost_enforcement
        || decision.inputs.budget.elapsed_ms != Some(0)
    {
        return Err(refused());
    }
    let input = observations::Input {
        full_context_digest: h.proposal.full_context_digest.clone(),
        spec,
        context: context.ok_or_else(refused)?,
        decision,
        selection,
        offer,
        source,
        material,
    };
    Ok(Pinned {
        history: h,
        input,
        policy,
        capabilities: current_registration.request.capabilities.clone(),
        hot,
        versions,
        registration_id: registration.id,
        qualification,
        issuance_ceiling: ceiling,
    })
}
