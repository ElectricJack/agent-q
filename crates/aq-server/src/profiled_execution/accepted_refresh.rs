//! Explicit registry-refresh owner; the frozen phase owner remains separate.
//! Shared files instantiate the same actual I/O/clock/association algorithms on
//! nominal refresh types. No old DTO is manufactured to invoke old validation.
// Shared owner files intentionally instantiate distinct nominal protocol types.
// A module alias would cross the old/new contract boundary instead.
#![allow(clippy::duplicate_mod)]
#[path = "accepted_preparation/accept.rs"]
mod accept;
#[path = "accepted_refresh/authority.rs"]
mod authority;
#[path = "accepted_preparation/history.rs"]
mod history;
#[path = "accepted_preparation/policy.rs"]
pub(super) mod policy;
#[path = "accepted_preparation/proposal.rs"]
mod proposal;
use super::accepted_preparation::{begin, db, now, quota, request_permit, require_schema, value};
use crate::{AppState, error::AppError};
pub use accept::accept;
use aq_admission::{ArtifactRef, MaterialLease, preparation_refresh_v1 as phase};
use aq_selection::AvailabilityObservation;
pub(crate) use authority::{PreparationAuthority, load_preparing_locked};
pub(crate) use history::{
    AcceptedHistory, load_accepted_history_locked, require_live_accepted_locked,
};
pub use policy::{PolicyDefinition, policy_for};
pub use proposal::{ProposalRequest, propose};
use sqlx::{Postgres, Row, Transaction};
const POLICY_PREFIX: &str = "aq.accepted-preparation-refresh-policy/";
pub(super) fn enabled(state: &AppState) -> bool {
    policy::enabled(state)
}

fn refused() -> AppError {
    AppError::Conflict("registry refresh preparation is not currently available".into())
}

fn validate_registry_at_proposal(
    decision: &crate::intelligence::resolver::Decision,
    registry: &ArtifactRef,
) -> Result<(), AppError> {
    let current = phase::registry_revision(registry).map_err(|_| refused())?;
    let selected = decision.outcome.resolved.as_ref().ok_or_else(refused)?;
    if decision.inputs.registry_revision == 0
        || selected.registry_revision == 0
        || current < decision.inputs.registry_revision
        || current < selected.registry_revision
        || decision
            .inputs
            .observations
            .iter()
            .any(|o| o.revision > current)
    {
        return Err(refused());
    }
    Ok(())
}

/// Caller holds the actual node/attempt/project owner locks. The existing
/// intelligence fence locks the real aggregate and validates loaded config.
/// This value is an observed owner ref, never a reconstructed selection ref.
pub(super) async fn read_registry_locked(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
) -> Result<ArtifactRef, AppError> {
    let fenced = crate::intelligence::fence_current_registry(state, tx).await?;
    let row = sqlx::query(
        "SELECT revision,config_digest FROM intelligence_registry WHERE singleton FOR SHARE",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(db)?;
    let revision = u64::try_from(row.get::<i64, _>("revision")).map_err(|_| refused())?;
    if revision == 0 || revision != fenced {
        return Err(refused());
    }
    let registry = ArtifactRef {
        id: aq_admission::Text::new("aq.intelligence-registry").map_err(|_| refused())?,
        version: aq_admission::Text::new(revision.to_string()).map_err(|_| refused())?,
        digest: serde_json::from_value(serde_json::json!(row.get::<String, _>("config_digest")))
            .map_err(|_| refused())?,
    };
    phase::registry_revision(&registry).map_err(|_| refused())?;
    let raw: Vec<serde_json::Value> = sqlx::query_scalar("SELECT observation FROM intelligence_observations ORDER BY scope_source_digest LIMIT 1025 FOR SHARE")
        .fetch_all(&mut **tx).await.map_err(db)?;
    if raw.len() > 1024 {
        return Err(refused());
    }
    for value in raw {
        let observed: AvailabilityObservation =
            serde_json::from_value(value).map_err(|_| refused())?;
        observed.validate().map_err(|_| refused())?;
        if observed.revision > revision {
            return Err(refused());
        }
    }
    Ok(registry)
}

async fn registry_at_accept(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    proposal: &phase::Proposal,
) -> Result<ArtifactRef, AppError> {
    let registry = read_registry_locked(state, tx).await?;
    phase::validate_registry_chain(&proposal.choice.registry, &registry, &registry, &registry)
        .map_err(|_| refused())?;
    Ok(registry)
}

async fn validate_live_registry(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    proposal: &phase::Proposal,
    receipt: &phase::AcceptedReceipt,
) -> Result<(), AppError> {
    let registry = read_registry_locked(state, tx).await?;
    phase::validate_registry_chain(
        &proposal.choice.registry,
        &receipt.lease.registry_at_accept,
        &registry,
        &registry,
    )
    .map_err(|_| refused())
}

fn accepted_lease(
    q: &phase::AcceptRequest,
    p: &phase::Proposal,
    material: &MaterialLease,
    time: u64,
    expiry: u64,
    lease_id: uuid::Uuid,
    registry_at_accept: ArtifactRef,
) -> Result<phase::AcceptedPreparationLease, AppError> {
    Ok(phase::AcceptedPreparationLease {
        lease_id: uuid::Uuid::new_v5(&lease_id, b"accepted-preparation-registry-refresh-v1"),
        revision: 1,
        identity: q.original.identity.clone(),
        binding: q.original.acceptance.binding.clone(),
        guardian_id: q.original.guardian_id,
        launch_id: q.original.launch_id,
        hold: q.hold.clone(),
        proposal_digest: q.proposal_digest.clone(),
        policy_digest: p.policy.digest().map_err(|_| refused())?,
        choice_digest: p.choice.digest().map_err(|_| refused())?,
        issued_at_ms: time,
        expires_at_ms: expiry,
        materials: vec![material.clone()],
        registry_at_accept,
    })
}

/// Existing installed backing remains checkable after retrieval expires. The
/// caller must independently own a current, still-live original execution grant.
pub(super) async fn require_installed_material_locked(
    state: &AppState,
    auth: &crate::worker_api::AuthenticatedNode,
    tx: &mut Transaction<'_, Postgres>,
    history: &AcceptedHistory,
) -> Result<(), AppError> {
    authority::current_policy_locked(state, tx, auth, &history.proposal).await?;
    authority::material_locked(tx, &history.request, &history.receipt).await
}
