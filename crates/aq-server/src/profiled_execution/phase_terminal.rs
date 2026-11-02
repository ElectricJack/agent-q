//! Qualified phase lifecycle reports and history on the authenticated worker listener.
// Shared files intentionally instantiate the distinct nominal terminal types.
#![allow(clippy::duplicate_mod)]
pub mod closure;
pub(crate) mod common;
pub mod history;
pub mod qualification;
pub mod routes;
pub mod started;
pub mod stopped;

pub(super) mod capacity;
use super::{accepted_preparation as accepted_owner, phase_execution as phase_owner};
use aq_phase_terminal_protocol as wire;
const QUALIFICATION_PREFIX: &str = "aq.phase-terminal-qualification/";
async fn load_phase_history(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    node: uuid::Uuid,
    binding: &aq_admission::LaunchBinding,
    grant: uuid::Uuid,
) -> Result<Box<phase_owner::history::PhaseExecutionHistory>, crate::error::AppError> {
    match super::initial_authority::load_locked(tx, node, binding, grant).await? {
        super::initial_authority::InitialAuthorityHistory::PhaseExecution(h) => Ok(h),
        _ => Err(common::conflict()),
    }
}
