//! Native metadata is admitted beside the common incarnation identity. It is
//! never reconstructed from a config allowlist or a legacy fixture capability.
use crate::{AppState, error::AppError};
use aq_admission::{
    ArtifactRef, Digest,
    transport::{Register, TransportDocument},
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRegistration {
    pub schema_version: u32,
    pub request: Register,
    pub semantic_digest: Digest,
    pub qualification: ArtifactRef,
    pub configuration_revision: u64,
    pub registered_at_ms: u64,
}
#[derive(Debug, Clone)]
pub struct RegisteredRuntime {
    pub runtime: ArtifactRef,
    pub qualification: ArtifactRef,
    pub boot_id: Uuid,
}
pub fn semantic_digest(request: &Register) -> Result<Digest, AppError> {
    let mut value = serde_json::to_value(request).map_err(|_| AppError::Internal)?;
    value
        .as_object_mut()
        .ok_or(AppError::Internal)?
        .remove("operation_id");
    aq_selection::digest(&value).map_err(|_| AppError::Internal)
}
/// Caller already holds current node lock. This reads the actual admitted
/// metadata plus common identity; missing metadata always refuses. All current
/// machine authorization/credential checks remain the caller's responsibility.
pub async fn current_runtime_locked(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    boot: Uuid,
) -> Result<RegisteredRuntime, AppError> {
    let record: serde_json::Value =
        sqlx::query_scalar("SELECT state FROM execution_nodes WHERE node_id=$1 FOR SHARE")
            .bind(node)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|_| AppError::Internal)?
            .ok_or_else(|| AppError::Conflict("native runtime is not registered".into()))?;
    let metadata: NativeRegistration = serde_json::from_value(
        record
            .get("native")
            .cloned()
            .ok_or_else(|| AppError::Conflict("native runtime is not registered".into()))?,
    )
    .map_err(|_| AppError::Conflict("invalid native registration".into()))?;
    TransportDocument::Register(metadata.request.clone())
        .validate()
        .map_err(|_| AppError::Conflict("invalid native registration".into()))?;
    if metadata.schema_version != 1
        || metadata.configuration_revision == 0
        || metadata.registered_at_ms == 0
        || metadata.request.identity.node_id != node
        || metadata.request.identity.boot_id != boot
        || record["identity"]
            != serde_json::to_value(&metadata.request.identity).map_err(|_| AppError::Internal)?
        || metadata.semantic_digest != semantic_digest(&metadata.request)?
    {
        return Err(AppError::Conflict(
            "native registration scope mismatch".into(),
        ));
    }
    let config = state
        .config
        .profiled_execution
        .as_deref()
        .ok_or_else(|| AppError::Conflict("native policy is not configured".into()))?;
    let qualified = config
        .qualified(&metadata.request)
        .map_err(|_| AppError::Conflict("native runtime qualification unavailable".into()))?;
    if metadata.configuration_revision > config.revision
        || metadata.qualification != qualified.qualification
    {
        return Err(AppError::Conflict(
            "native runtime qualification changed".into(),
        ));
    }
    Ok(RegisteredRuntime {
        runtime: metadata.request.runtime,
        qualification: metadata.qualification,
        boot_id: boot,
    })
}
