//! Historical protocol origin, separate from current launch authority.
//!
//! Existing readers explicitly accept only their qualified legacy variant. An
//! accepted no-launch receipt is never converted into a legacy preparation or
//! initial execution receipt, even if a hot row happens to contain a Grant.
use super::{accepted_preparation, initial_grant};
use crate::error::AppError;
use aq_admission::LaunchBinding;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) enum InitialAuthorityHistory {
    Legacy(Box<initial_grant::OriginalGrant>),
    AcceptedNoLaunch(Box<accepted_preparation::AcceptedHistory>),
    PhaseExecution(Box<super::phase_execution::history::PhaseExecutionHistory>),
}
fn unsupported() -> AppError {
    AppError::Conflict("execution protocol origin is not supported by this owner".into())
}
async fn has_accepted_origin_locked(
    tx: &mut Transaction<'_, Postgres>,
    binding: &LaunchBinding,
) -> Result<bool, AppError> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM profiled_v2_accepts WHERE attempt_id=$1)")
        .bind(binding.attempt_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| AppError::Internal)
}
/// Call under the exact attempt authority lock. This is an origin fence, not an
/// existence or current permission claim; malformed phase state also refuses.
pub(crate) async fn require_legacy_origin_locked(
    tx: &mut Transaction<'_, Postgres>,
    binding: &LaunchBinding,
) -> Result<(), AppError> {
    if has_accepted_origin_locked(tx, binding).await? {
        return Err(unsupported());
    }
    Ok(())
}
pub(crate) async fn load_locked(
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    binding: &LaunchBinding,
    grant_id: Uuid,
) -> Result<InitialAuthorityHistory, AppError> {
    if has_accepted_origin_locked(tx, binding).await? {
        let history = accepted_preparation::load_accepted_history_locked(tx, binding).await?;
        if history.request.original.identity.node_id != node
            || history.proposal.original.offered.offer.binding != *binding
        {
            return Err(unsupported());
        }
        if let Some(execution) =
            super::phase_execution::history::load_optional_locked(tx, node, binding, grant_id)
                .await?
        {
            return Ok(InitialAuthorityHistory::PhaseExecution(Box::new(execution)));
        }
        return Ok(InitialAuthorityHistory::AcceptedNoLaunch(Box::new(history)));
    }
    Ok(InitialAuthorityHistory::Legacy(Box::new(
        initial_grant::load_legacy_original_grant_locked(tx, node, binding, grant_id).await?,
    )))
}
pub(crate) async fn load_legacy_initial_grant_locked(
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    binding: &LaunchBinding,
    grant_id: Uuid,
) -> Result<initial_grant::OriginalGrant, AppError> {
    match load_locked(tx, node, binding, grant_id).await? {
        InitialAuthorityHistory::Legacy(grant) => Ok(*grant),
        InitialAuthorityHistory::PhaseExecution(history) => {
            let _original_receipt = history.receipt;
            Err(unsupported())
        }
        InitialAuthorityHistory::AcceptedNoLaunch(history) => {
            // Read the actual typed origin rather than accepting a caller tag.
            let _original_receipt = history.receipt;
            Err(unsupported())
        }
    }
}
