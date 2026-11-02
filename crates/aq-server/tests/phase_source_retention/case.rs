use crate::{actual_git, common};
use aq_admission::{
    Digest, Label, SessionIdentity, result_record,
    transport::{ProcessOutcome, ResultReport, WorkerIdentity},
};
use aq_server::profiled_execution::results;
use aq_worker_protocol::{SourceRole, TransferManifest, TransferStatus, TransferStatusRequest};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;
use tower::ServiceExt;
use uuid::Uuid;

async fn request(
    app: &Router,
    credential: &str,
    method: &str,
    path: &str,
    bytes: Vec<u8>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {credential}"))
                .header(
                    "content-type",
                    if method == "PUT" {
                        "application/octet-stream"
                    } else {
                        "application/json"
                    },
                )
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.len() < 65536);
    (status, serde_json::from_slice(&bytes).unwrap())
}
async fn post(
    app: &Router,
    token: &str,
    path: &str,
    value: &impl Serialize,
) -> (StatusCode, Value) {
    request(app, token, "POST", path, serde_json::to_vec(value).unwrap()).await
}
async fn chunk(
    app: &Router,
    token: &str,
    transfer: Uuid,
    file: &str,
    operation: Uuid,
    offset: usize,
    bytes: &[u8],
) -> Value {
    let path = format!(
        "/v1/execution/sources/{transfer}/{file}?operation_id={operation}&offset={offset}&digest={}",
        Digest::of(bytes).as_str()
    );
    let (status, body) = request(app, token, "PUT", &path, bytes.to_vec()).await;
    assert_eq!(status, 200, "{body}");
    body
}
async fn invariants(pool: &sqlx::PgPool, attempt: Uuid) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('hot',h.state,'record_revision',h.record_revision,'authority_revision',h.authority_revision,'grant_revision',h.grant_revision,'holds_task',a.holds_task,'effects',(SELECT count(*) FROM agent_graph_operations WHERE attempt_id=$1),'releases',(SELECT count(*) FROM profiled_verified_releases WHERE attempt_id=$1)) FROM execution_attempts a JOIN profiled_authority_hot h USING(attempt_id) WHERE attempt_id=$1")
        .bind(attempt).fetch_one(pool).await.unwrap()
}
// Exact published GitManifest field order, matching both the worker's source
// protocol and server import verifier. Raw uploaded bytes have an independent
// hash and need not share this canonical serialization's digest.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GitManifest {
    format_version: u32,
    repository_id: Uuid,
    repository_identity: String,
    binding_generation: u64,
    transfer_id: Uuid,
    object_format: String,
    base: aq_worker_protocol::ObjectId,
    source: aq_worker_protocol::ObjectId,
    prerequisites: Vec<aq_worker_protocol::ObjectId>,
    bundle_size: u64,
    bundle_sha256: Option<String>,
    producer_installation_id: Uuid,
    reference_only: bool,
    #[serde(default)]
    caller_correlation: Value,
}
#[tokio::test]
#[ignore = "explicit pinned actual Git owners; one coordinated cold Git lane and default free-space policy"]
async fn actual_phase_stopped_checkpoint_http_import_and_current_retention_are_not_legacy_gated() {
    let mut ready = Box::pin(common::presence::setup()).await;
    let mut actual = Box::pin(actual_git::Actual::prepare(&mut ready)).await;
    // Fixture edits precede synthetic terminal records. No real harness or
    // provider process exists; checkpointing below is physically local Git.
    std::fs::write(
        Path::new(actual.workspace["path"].as_str().unwrap()).join("source"),
        "actual retained phase fixture change\n",
    )
    .unwrap();
    let f = Box::pin(terminal::prepared_terminal_from_presence(ready)).await;
    let issued = terminal::issue(&f).await;
    let auth = terminal::auth(&f).await;
    let start = terminal::started_request(&f, &issued);
    terminal_owner::started::record(&f.c.env.env.state, &auth, &start)
        .await
        .unwrap();
    let stop = terminal::stop_request(&start);
    terminal_owner::stopped::record(&f.c.env.env.state, &auth, &stop)
        .await
        .unwrap();
    let state = &f.c.env.env.state;
    let pool = &f.c.env.env.pool;
    let credential = &f.c.env.env.credential;
    let binding = &start.original.binding;
    let SessionIdentity::Remote {
        node_id, boot_id, ..
    } = binding.session
    else {
        panic!()
    };
    let before = invariants(pool, binding.attempt_id).await;
    let mut tx = pool.begin().await.unwrap();
    let view =
        aq_server::execution::authority::load_source_locked(&mut tx, node_id, binding.attempt_id)
            .await
            .unwrap();
    let source = view.source.clone().unwrap();
    let attempt = view.attempt.clone();
    let spec_digest = view.execution_spec_digest.clone();
    tx.commit().await.unwrap();
    assert_eq!(source.base.hex, actual.base);
    assert_eq!(source.repository_id, actual.repo);
    assert!(view.stopped && view.holds_task);
    let (export, manifest_bytes, bundle) = actual.export_changed().await;
    let native: GitManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    let canonical_digest = Digest::of(&serde_json::to_vec(&native).unwrap());
    assert_eq!(serde_json::to_value(&native).unwrap(), export["manifest"]);
    let manifest = TransferManifest {
        transfer_id: Uuid::new_v4(),
        role: SourceRole::Checkpoint,
        checkpoint: native.source.clone(),
        attempt,
        resolved_spec_digest: spec_digest,
        source,
        artifact_id: native.transfer_id,
        manifest_digest: Digest::of(&manifest_bytes).as_str().into(),
        manifest_bytes: manifest_bytes.len() as u64,
        bundle_digest: Some(Digest::of(&bundle).as_str().into()),
        bundle_bytes: bundle.len() as u64,
    };
    let begin = aq_server::source_retention::BeginRequest {
        identity: aq_worker_protocol::NodeIdentity { node_id, boot_id },
        operation_id: Uuid::new_v4(),
        manifest: manifest.clone(),
    };
    let app = aq_server::build_worker_router(state.clone());
    let mut wrong = begin.clone();
    wrong.operation_id = Uuid::new_v4();
    wrong.manifest.resolved_spec_digest = "a".repeat(64);
    let (code, _) = post(&app, credential, "/v1/execution/sources/begin", &wrong).await;
    assert_eq!(code, 403);
    let (code, original_begin) =
        post(&app, credential, "/v1/execution/sources/begin", &begin).await;
    assert_eq!(code, 200, "{original_begin}");
    let finalize = TransferStatusRequest {
        identity: begin.identity.clone(),
        operation_id: Uuid::new_v4(),
        transfer_id: manifest.transfer_id,
    };
    assert_eq!(
        post(
            &app,
            credential,
            "/v1/execution/sources/finalize",
            &finalize
        )
        .await
        .0,
        409
    );
    let middle = manifest_bytes.len() / 2;
    let first_op = Uuid::new_v4();
    let first = chunk(
        &app,
        credential,
        manifest.transfer_id,
        "manifest",
        first_op,
        0,
        &manifest_bytes[..middle],
    )
    .await;
    assert_eq!(first["manifest_offset"], middle);
    assert_eq!(
        post(
            &app,
            credential,
            "/v1/execution/sources/finalize",
            &finalize
        )
        .await
        .0,
        409
    );
    assert_eq!(
        first,
        chunk(
            &app,
            credential,
            manifest.transfer_id,
            "manifest",
            first_op,
            0,
            &manifest_bytes[..middle]
        )
        .await
    );
    chunk(
        &app,
        credential,
        manifest.transfer_id,
        "manifest",
        Uuid::new_v4(),
        middle,
        &manifest_bytes[middle..],
    )
    .await;
    for (index, bytes) in bundle.chunks(256 * 1024).enumerate() {
        chunk(
            &app,
            credential,
            manifest.transfer_id,
            "bundle",
            Uuid::new_v4(),
            index * 256 * 1024,
            bytes,
        )
        .await;
    }
    let (code, body) = post(
        &app,
        credential,
        "/v1/execution/sources/finalize",
        &finalize,
    )
    .await;
    assert_eq!(code, 200, "{body}");
    let retained: TransferStatus = serde_json::from_value(body.clone()).unwrap();
    let receipt = retained.receipt.unwrap();
    assert_eq!(receipt.manifest_digest, canonical_digest.as_str());
    assert_eq!(receipt.transfer_id, manifest.transfer_id);
    assert_eq!(receipt.retention_handle, manifest.artifact_id);
    assert_eq!(receipt.source, manifest.checkpoint);
    assert_eq!(
        post(
            &app,
            credential,
            "/v1/execution/sources/finalize",
            &finalize
        )
        .await
        .1,
        body
    );
    assert_eq!(
        post(&app, credential, "/v1/execution/sources/status", &finalize)
            .await
            .1,
        body
    );
    assert_eq!(
        post(&app, credential, "/v1/execution/sources/begin", &begin)
            .await
            .1,
        original_begin,
        "begin replay keeps its immutable original response"
    );
    let report = ResultReport {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: WorkerIdentity { node_id, boot_id },
        binding: binding.clone(),
        report_id: Uuid::new_v4(),
        supersedes: None,
        outcome: ProcessOutcome::Success,
        stop: stop.original.evidence,
        source_receipt_ids: vec![receipt.receipt_id],
        pending_effect_ids: vec![],
        conclusion: Label::new(
            "Actual Git checkpoint retained; synthetic host claim; no settlement",
        )
        .unwrap(),
    };
    let (code, recorded) = post(&app, credential, "/v1/worker/profiled/results", &report).await;
    assert_eq!(code, 200, "{recorded}");
    let recorded: result_record::Receipt = serde_json::from_value(recorded).unwrap();
    recorded.validate_for(&report).unwrap();
    let mut tx = pool.begin().await.unwrap();
    let result = results::load_locked(state, &mut tx, report.report_id)
        .await
        .unwrap();
    let fact =
        aq_server::source_retention::load_confirmed_locked(&mut tx, receipt.receipt_id, &result)
            .await
            .unwrap();
    tx.commit().await.unwrap();
    aq_server::source_retention::result_retention::inspect_current(state, &fact)
        .await
        .unwrap()
        .validate_current(state, &fact)
        .unwrap();
    actual.worker.stop().await;
    std::fs::remove_dir_all(&actual.worker.root).unwrap();
    assert!(!actual.worker.root.exists());
    aq_server::source_retention::result_retention::inspect_current(state, &fact)
        .await
        .unwrap();
    let copy=actual.central.call(actual.repo,"artifact.export",json!({"base":manifest.source.base,"source":manifest.checkpoint,"retention_handle":manifest.artifact_id,"confirmed_prerequisites":[]})).await;
    let verify = tempfile::tempdir().unwrap();
    actual_git::git(verify.path(), &["init"]).await;
    let bundle_path = actual.central.artifact(&copy).join("objects.bundle");
    actual_git::git(
        verify.path(),
        &[
            "fetch",
            "--no-tags",
            bundle_path.to_str().unwrap(),
            &manifest.checkpoint.hex,
        ],
    )
    .await;
    assert_eq!(
        actual_git::git(
            verify.path(),
            &["show", &format!("{}:source", manifest.checkpoint.hex)]
        )
        .await,
        "actual retained phase fixture change"
    );
    let pin = actual
        .central
        .call(
            actual.repo,
            "retention.inspect",
            json!({"handle_id":manifest.artifact_id}),
        )
        .await;
    actual.central.call(actual.repo,"retention.release",json!({"handle_id":manifest.artifact_id,"expected_generation":pin["generation"],"refs":pin["refs"],"reason":"explicit source-retention fixture retirement"})).await;
    assert!(
        aq_server::source_retention::result_retention::inspect_current(state, &fact)
            .await
            .is_err(),
        "historical import is not current retention after release"
    );
    assert_eq!(before, invariants(pool, binding.attempt_id).await);
    assert!(before["holds_task"].as_bool().unwrap());
    actual.central.stop().await;
    // Keep the bounded private fixture state for audit even on success. The
    // external wrapper still must verify the WHOLE unique case session before
    // claiming cleanup; no uncertain process can race automatic TempDir removal.
    println!(
        "actual phase source retained checkpoint upload report and current-retention proof; no physical/provider/settlement claim"
    );
}
