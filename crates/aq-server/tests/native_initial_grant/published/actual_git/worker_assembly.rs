//! Frozen independent worker fixture; historical format10/old profile evidence stays explicit.
// Shared files intentionally instantiate nominal old/new proof records.
#![allow(clippy::duplicate_mod)]
const REFRESH_PROFILE: bool = false;
const JOURNAL_FORMAT: u64 = 10;
const PHASE_CONFIG_FIELD: &str = "phase_preparation";
const WORKER_BINARY_ENV: &str = "AQ_TEST_NATIVE_WORKER";
const WORKER_DIGEST_ENV: &str = "AQ_TEST_NATIVE_WORKER_SHA256";
#[path = "worker_assembly/phase.rs"]
mod phase;
#[path = "worker_assembly/phase_assessment.rs"]
mod phase_assessment;
#[path = "worker_assembly/phase_history.rs"]
mod phase_history;
#[path = "worker_assembly/phase_host.rs"]
mod phase_host;
#[path = "worker_assembly/phase_output.rs"]
mod phase_output;
#[path = "worker_assembly/phase_terminal.rs"]
mod phase_terminal;
mod refresh_identity {
    pub(super) async fn configure(_: &mut super::Case) {
        unreachable!("frozen fixture must preserve its identity");
    }
}
include!("worker_assembly_core.rs");
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit pinned native worker smoke binary and actual aq-git/Git"]
async fn actual_worker_downloads_installs_hydrates_and_prepares_against_real_daemon() {
    run_worker(Mode::Legacy).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit pinned phase-capable worker smoke binary and actual aq-git/Git"]
async fn actual_worker_durably_accepts_new_phase_and_replays_after_proposal_expiry() {
    run_worker(Mode::Accepted).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit pinned phase-input worker smoke binary and actual aq-git/Git"]
async fn actual_worker_prepares_phase_inputs_after_proposal_expiry() {
    run_worker(Mode::Inputs).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires explicit pinned phase execution worker and actual aq-git/Git"]
async fn actual_worker_receives_durable_phase_execution_from_actual_issuer() {
    run_worker(Mode::Execution).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned worker, aq-git/Git, host helper, synthetic ELF and actual tmux"]
async fn actual_worker_phase_execution_launches_and_stops_actual_tmux_host() {
    run_worker(Mode::Host).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned history-capable worker and actual aq-git/Git"]
async fn actual_worker_reads_history_after_withheld_first_execution_reply() {
    run_worker(Mode::LostExecutionReply).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned actual worker/Git/tmux and qualified phase terminal owner"]
async fn actual_worker_reports_real_host_lifecycle_and_persists_daemon_acknowledgments() {
    run_worker(Mode::Terminal).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned output-capable worker, periodic ELF, Git and tmux"]
async fn historical_state33_actual_worker_output_reaches_authorized_tail_after_physical_stop() {
    // This frozen phase-terminal fixture starts at the genuine ordered32
    // prefix and uses its existing33 setup. It is not latest-format startup.
    aq_server::test_support::with_state32_store(Box::pin(run_worker(Mode::Output))).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires pinned output-capable worker, periodic ELF, Git and tmux"]
async fn current_store_actual_worker_output_reaches_authorized_tail_after_physical_stop() {
    // Use actual current Store startup. Prepare requires this compiled schema
    // and the terminal capacity owners installed by the ordered migrations.
    run_worker(Mode::Output).await;
}
