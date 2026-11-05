//! Phase execution, refresh, results and terminal history.
//!
//! One integration test binary for this area: each former `tests/<name>.rs`
//! binary is a module here, so its tests are named `<name>::...`. Add new
//! tests to an existing module (or a new module listed below), never a new
//! test binary. See AGENTS.md "Build health".

// Some tests/support and tests/common fixture files are path-included by more
// than one module (each module was its own test binary before 2026-11-05).
#![allow(clippy::duplicate_mod)]

#[path = "../common/mod.rs"]
mod common;
#[allow(dead_code)]
#[path = "../common/phase_execution.rs"]
mod common_phase_execution;
#[allow(dead_code)]
#[path = "../common/phase_refresh_terminal.rs"]
mod common_phase_refresh_terminal;
#[allow(dead_code)]
#[path = "../common/phase_terminal.rs"]
mod common_phase_terminal;

mod phase_execution;
mod phase_execution_transport;
mod phase_initial_lease;
mod phase_never_granted;
mod phase_refresh_execution;
mod phase_refresh_terminal;
mod phase_results;
mod phase_source_retention;
mod phase_terminal_closure;
mod phase_terminal_fixture;
mod phase_terminal_history;
mod phase_terminal_started;
mod phase_terminal_stopped;
mod phase_terminal_transport;
mod terminal_tail;
