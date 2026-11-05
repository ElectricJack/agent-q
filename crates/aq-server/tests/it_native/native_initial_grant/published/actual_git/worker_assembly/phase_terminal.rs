//! Frozen terminal wrapper over actual old lifecycle records.
pub(super) use aq_phase_terminal_protocol as wire;
use aq_server::profiled_execution::phase_terminal::qualification::TerminalQualification;
const QUALIFICATION_ID: &str = "aq.phase-terminal-qualification/actual-process-test";
include!("phase_terminal_core.rs");
