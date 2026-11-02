//! Refresh nominal terminal fixture; synthetic proof DTOs are not kernel proof.
#[allow(dead_code)]
#[path = "phase_refresh_execution.rs"]
mod phase_fixture;
use aq_admission::preparation_refresh_v1::ExecutionReceipt;
use aq_phase_terminal_protocol::refresh_v1 as wire;
use aq_server::profiled_execution::{
    phase_refresh_execution as phase_owner,
    phase_refresh_terminal::qualification::TerminalQualification,
};
const QUALIFICATION_ID: &str = "aq.phase-terminal-refresh-qualification/test";
include!("phase_terminal_core.rs");
