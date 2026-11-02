//! Frozen nominal terminal fixture over actual old-profile owner records.
#[allow(dead_code)]
#[path = "phase_execution.rs"]
mod phase_fixture;
use aq_admission::preparation_v2::ExecutionReceipt;
use aq_phase_terminal_protocol as wire;
use aq_server::profiled_execution::{
    phase_execution as phase_owner, phase_terminal::qualification::TerminalQualification,
};
const QUALIFICATION_ID: &str = "aq.phase-terminal-qualification/test";
include!("phase_terminal_core.rs");
