//! Explicit worker0.4.0/FORMAT11 refresh physical qualification, static peer only.
// Shared files intentionally instantiate nominal old/new proof records.
#![allow(clippy::duplicate_mod)]
const REFRESH_PROFILE: bool = true;
const JOURNAL_FORMAT: u64 = 11;
const PHASE_CONFIG_FIELD: &str = "refresh_preparation";
const WORKER_BINARY_ENV: &str = "AQ_TEST_REFRESH_WORKER";
const WORKER_DIGEST_ENV: &str = "AQ_TEST_REFRESH_WORKER_SHA256";
#[path = "worker_assembly/phase_refresh.rs"]
mod phase;
#[path = "worker_assembly/phase_assessment.rs"]
mod phase_assessment;
#[path = "worker_assembly/phase_history.rs"]
mod phase_history;
#[path = "worker_assembly/phase_host.rs"]
mod phase_host;
#[path = "worker_assembly/phase_output.rs"]
mod phase_output;
#[path = "worker_assembly/phase_terminal_refresh.rs"]
mod phase_terminal;
#[path = "worker_assembly/refresh_identity.rs"]
mod refresh_identity;
include!("worker_assembly_core.rs");
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicitly pinned worker0.4.0/FORMAT11, Git/tmux/static peer; no model/provider"]
async fn current_store_refresh_worker_output_reaches_real_watch_upload_ack_and_authorized_tail() {
    run_worker(Mode::Output).await;
}
