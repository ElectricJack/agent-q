//! Immutable accepted phase provenance, not a live input or execution permit.
use super::*;
use aq_admission::{
    ArtifactRef, LaunchBinding,
    preparation_v2::{AcceptRequest, AcceptedReceipt, Proposal},
};
use sqlx::Row;
#[derive(Clone)]
pub(crate) struct AcceptedHistory {
    pub proposal: Proposal,
    pub request: AcceptRequest,
    pub receipt: AcceptedReceipt,
    pub policy: ArtifactRef,
}
pub(crate) async fn load_accepted_history_locked(
    tx: &mut Transaction<'_, Postgres>,
    binding: &LaunchBinding,
) -> Result<AcceptedHistory, AppError> {
    let proposal = authority::proposal_locked(tx, binding).await?;
    let (node, boot) = match binding.session {
        aq_admission::SessionIdentity::Remote {
            node_id, boot_id, ..
        } => (node_id, boot_id),
        _ => return Err(refused()),
    };
    let row=sqlx::query("SELECT a.request,p.policy FROM profiled_v2_accepts a JOIN profiled_v2_proposals p USING(attempt_id) WHERE a.attempt_id=$1 AND a.node_id=$2 AND p.node_id=$2 AND p.boot_id=$3 FOR SHARE OF a,p").bind(binding.attempt_id).bind(node).bind(boot).fetch_optional(&mut **tx).await.map_err(db)?.ok_or_else(refused)?;
    let request: AcceptRequest =
        serde_json::from_value(row.get("request")).map_err(|_| refused())?;
    let policy: ArtifactRef = serde_json::from_value(row.get("policy")).map_err(|_| refused())?;
    aq_session_protocol::Validate::validate(&policy.id).map_err(|_| refused())?;
    aq_session_protocol::Validate::validate(&policy.version).map_err(|_| refused())?;
    let receipt = authority::accepted_locked(tx, &proposal, &request)
        .await?
        .ok_or_else(refused)?;
    // Historical lease body/indices stay required after retrieval expiry or
    // revocation. Current admitted state is checked only by the live owner.
    let lease = &receipt.lease.materials[0];
    let actual=sqlx::query("SELECT m.* FROM admission_material_leases m JOIN profiled_launch_materials x ON x.lease_id=m.lease_id AND x.attempt_id=$2 WHERE m.lease_id=$1 FOR SHARE OF m,x").bind(lease.lease_id).bind(binding.attempt_id).fetch_optional(&mut **tx).await.map_err(db)?.ok_or_else(refused)?;
    if actual.get::<serde_json::Value, _>("lease") != value(lease)?
        || actual.get::<uuid::Uuid, _>("node_id") != node
        || actual.get::<uuid::Uuid, _>("boot_id") != boot
        || actual.get::<uuid::Uuid, _>("operation_id") != request.original.operation_id
        || actual.get::<String, _>("request_digest")
            != request.digest().map_err(|_| refused())?.as_str()
        || actual.get::<uuid::Uuid, _>("prospective_attempt_id") != binding.attempt_id
        || actual.get::<Option<uuid::Uuid>, _>("admitted_attempt_id") != Some(binding.attempt_id)
        || actual.get::<uuid::Uuid, _>("offer_id") != binding.offer_id
        || actual.get::<String, _>("credential_identity")
            != format!(
                "{}@{}",
                lease.credential_profile.id.as_str(),
                lease.credential_profile.version.as_str()
            )
        || actual.get::<i64, _>("credential_revision") as u64 != lease.credential_revision
        || actual.get::<String, _>("purpose") != lease.purpose.as_str()
        || actual.get::<i64, _>("issued_at_ms") as u64 != lease.issued_at_ms
        || actual.get::<i64, _>("expires_at_ms") as u64 != lease.expires_at_ms
    {
        return Err(refused());
    }
    Ok(AcceptedHistory {
        proposal,
        request,
        receipt,
        policy,
    })
}
pub(crate) async fn require_live_accepted_locked(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    auth: &crate::worker_api::AuthenticatedNode,
    h: &AcceptedHistory,
) -> Result<(), AppError> {
    authority::live_locked(state, tx, auth, &h.proposal, &h.request, &h.receipt).await
}
