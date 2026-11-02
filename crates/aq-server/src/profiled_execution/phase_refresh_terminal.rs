//! Explicit refresh lifecycle/history owner over actual retained refresh records.
// Shared files intentionally instantiate the distinct nominal terminal types.
#![allow(clippy::duplicate_mod)]
#[path = "phase_terminal/closure.rs"]
pub mod closure;
#[path = "phase_terminal/common.rs"]
pub(crate) mod common;
#[path = "phase_terminal/history.rs"]
pub mod history;
#[path = "phase_terminal/qualification.rs"]
pub mod qualification;
#[path = "phase_terminal/started.rs"]
pub mod started;
#[path = "phase_terminal/stopped.rs"]
pub mod stopped;
use super::{accepted_refresh as accepted_owner, phase_refresh_execution as phase_owner};
use aq_phase_terminal_protocol::refresh_v1 as wire;
const QUALIFICATION_PREFIX: &str = "aq.phase-terminal-refresh-qualification/";
async fn load_phase_history(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    node: uuid::Uuid,
    binding: &aq_admission::LaunchBinding,
    grant: uuid::Uuid,
) -> Result<Box<phase_owner::history::PhaseExecutionHistory>, crate::error::AppError> {
    match super::initial_authority::load_locked(tx, node, binding, grant).await? {
        super::initial_authority::InitialAuthorityHistory::PhaseRefreshExecution(h) => Ok(h),
        _ => Err(common::conflict()),
    }
}
