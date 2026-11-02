//! Real private Git export/import + phase/source/result SQL owners. Host proof,
//! material installation and Graph observations remain explicit synthetic facts.
#[path = "phase_source_retention/git.rs"]
mod actual_git;
mod common;
#[allow(dead_code)]
#[path = "common/phase_terminal.rs"]
mod frozen_fixture;
#[allow(dead_code)]
#[path = "common/phase_refresh_terminal.rs"]
mod refresh_fixture;
mod frozen {
    use super::frozen_fixture as terminal;
    use aq_server::profiled_execution::phase_terminal as terminal_owner;
    include!("phase_source_retention/case.rs");
}
mod refresh {
    use super::refresh_fixture as terminal;
    use aq_server::profiled_execution::phase_refresh_terminal as terminal_owner;
    include!("phase_source_retention/case.rs");
}
