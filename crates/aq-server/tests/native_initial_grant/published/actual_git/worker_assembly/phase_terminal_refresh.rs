//! Refresh terminal wrapper over actual new lifecycle records.
pub(super) use aq_phase_terminal_protocol::refresh_v1 as wire;
use aq_server::profiled_execution::phase_refresh_terminal::qualification::TerminalQualification;
const QUALIFICATION_ID: &str =
    "aq.phase-terminal-refresh-qualification/actual-worker0.4.0-process-test";
include!("phase_terminal_core.rs");
