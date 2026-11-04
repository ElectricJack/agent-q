//! D6 follow-up: a Host7 check.request on a production-constructed service
//! reaches the node check executor, the node (played here over the worker
//! API with the fixture's machine credential) claims and stops it, the
//! server writes `profiled_check_evidence` only from that reported stop, and
//! operation.get completes from it. The retained source row is synthetic
//! (validation only, not real Git).
use super::*;
use aq_playbook_host::runner::{Intent, IntentStatus};
use aq_server::{
    playbooks::{
        domain::{GrantScope, digest},
        host_adapters::WorkGraph,
        review_domain::{CatalogProfile, HOST7_CAPABILITIES},
        service::*,
    },
    protected_storage::{KeyFile, Keyring, ProtectionConfig},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use std::{collections::BTreeSet, os::unix::fs::PermissionsExt};
use tower::ServiceExt;

struct NoGraph;
#[async_trait::async_trait]
impl WorkGraph for NoGraph {
    async fn node(&self, _: Uuid, _: Uuid) -> aq_server::playbooks::domain::Result<Option<Value>> {
        Ok(None)
    }
    async fn mutate(
        &self,
        _: Uuid,
        _: Value,
    ) -> aq_server::playbooks::domain::Result<(u16, Value)> {
        panic!("check requests never mutate the graph")
    }
}

/// Synthetic retained checkpoint for `source` (the auto_settlement56 shape).
async fn retain_source(f: &Fixture, report: &ResultReport, source: Uuid) {
    let transfer_id = Uuid::new_v4();
    let artifact_id = Uuid::new_v4();
    let manifest = aq_worker_protocol::TransferManifest {
        transfer_id,
        role: aq_worker_protocol::SourceRole::Checkpoint,
        checkpoint: f.profiled.source.base.clone(),
        attempt: aq_worker_protocol::AttemptRef {
            attempt_id: report.binding.attempt_id,
            session_id: match report.binding.session {
                aq_session_protocol::SessionIdentity::Remote { session_id, .. } => session_id,
                _ => unreachable!(),
            },
            generation: report.binding.assignment_generation,
        },
        resolved_spec_digest: report.binding.execution_spec_digest.as_str().into(),
        source: f.profiled.source.clone(),
        artifact_id,
        manifest_digest: "a".repeat(64),
        bundle_digest: None,
        manifest_bytes: 1,
        bundle_bytes: 0,
    };
    let receipt = aq_worker_protocol::CentralRetentionReceipt {
        receipt_id: source,
        transfer_id,
        repository_id: manifest.source.repository_id,
        binding_generation: manifest.source.binding_generation,
        source: manifest.checkpoint.clone(),
        retention_handle: artifact_id,
        manifest_digest: "b".repeat(64),
    };
    let raw = json!({"repository_id":receipt.repository_id,"binding_generation":receipt.binding_generation,"transfer_id":artifact_id,"retention_handle":artifact_id,"manifest_digest":receipt.manifest_digest,"source":receipt.source,"retention_state":"active","historical":false,"receiver_installation_id":Uuid::new_v4()});
    sqlx::query("INSERT INTO source_transfers(transfer_id,attempt_id,node_id,repository_id,artifact_id,import_operation_id,manifest,state,receipt,central_receipt) VALUES($1,$2,$3,$4,$5,$6,$7,'retained',$8,$9)")
        .bind(transfer_id).bind(manifest.attempt.attempt_id).bind(f.node.node_id)
        .bind(manifest.source.repository_id).bind(artifact_id).bind(source)
        .bind(json!(manifest)).bind(json!(receipt)).bind(raw)
        .execute(&f.db.pool().await).await.unwrap();
}

async fn host7(f: &Fixture) -> (Service, RuntimeOwner, Uuid, GrantScope, i64) {
    let l = &f.selection.lineage;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key");
    std::fs::write(&path, [9_u8; 32]).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let keys = Keyring::load(&ProtectionConfig {
        active_key_id: "test".into(),
        keys: vec![KeyFile {
            id: "test".into(),
            file: path,
        }],
    })
    .unwrap();
    // The production binding: present because profiled execution is configured.
    let checks = aq_server::check_attempts::production_executor(&f.state);
    assert!(checks.is_some());
    let service = Service::new_host7(
        &f.state,
        Namespace {
            namespace_id: Uuid::new_v4(),
            generation: 1,
            name: format!("fixture-{}", Uuid::new_v4()),
            workspace_id: l.workspace_id,
            project_id: l.project_id,
            owner_user_id: f.actor,
        },
        Arc::new(keys),
        Arc::new(NoGraph),
        checks,
        None,
    )
    .await
    .unwrap();
    service.initialize(f.actor).await.unwrap();
    let now = (f.state.clock)().timestamp_millis();
    let scope = GrantScope {
        workspace_id: l.workspace_id,
        project_id: l.project_id,
        recipients: vec![],
        capabilities: BTreeSet::from(HOST7_CAPABILITIES.map(String::from)),
    };
    let grant = Uuid::new_v4();
    service
        .grant(f.actor, grant, scope.clone(), now + 10_000)
        .await
        .unwrap();
    let runtime = service.adopt_runtime(Uuid::new_v4(), 0).await.unwrap();
    (service, runtime, grant, scope, now)
}

fn intent(grant: Uuid, scope: &GrantScope, now: i64, id: &str, name: &str, input: Value) -> Intent {
    Intent {
        id: id.into(),
        run: "synthetic-run".into(),
        step: name.replace('.', "-"),
        invocation: 1,
        deadline_at_ms: now + 5000,
        command: CatalogProfile::Host7.registry().commands[name].clone(),
        input_digest: digest(&input).unwrap(),
        input,
        authority: CatalogProfile::Host7.authority(grant, 1, scope).unwrap(),
        status: IntentStatus::Dispatched,
        cancel_requested: false,
        cancel_dispatched: false,
        handle: None,
        result: None,
        reason: None,
        late_results: vec![],
        completion_event: None,
    }
}

async fn node_post(f: &Fixture, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = aq_server::build_worker_router(f.state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", f.credential))
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 65536)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn host7_check_request_runs_on_node_and_records_reported_evidence() {
    let (f, _, mut report) = Box::pin(prepare_with_policy(true, true, None, false, None)).await;
    let source = Uuid::new_v4();
    report.source_receipt_ids.push(source);
    results::record(&f.state, &context(&f).await, &report)
        .await
        .unwrap();
    retain_source(&f, &report, source).await;
    let pool = f.db.pool().await;
    let mut tx = pool.begin().await.unwrap();
    let recorded = results::load_locked(&f.state, &mut tx, report.report_id)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    let check = recorded.outcome.required_checks.first().unwrap().clone();
    let validator = json!({"id": check.validator.id.as_str(),
        "version": check.validator.version.as_str(),
        "digest": format!("sha256:{}", check.validator.digest.as_str())});
    let checkpoint = f.profiled.source.base.hex.clone();
    let subject = json!({"id": recorded.spec.task.task_id.to_string(),
        "revision": recorded.outcome.task.version});
    let input = json!({"check": {"id": check.id.as_str(), "validator": validator},
        "source": {"format": "sha1", "hex": checkpoint}, "subject": subject});

    let (service, runtime, grant, scope, now) = host7(&f).await;
    let dispatch = |id: &str, name: &str, input: Value| {
        let i = intent(grant, &scope, now, id, name, input);
        let (service, runtime) = (&service, &runtime);
        async move { service.host_dispatch(runtime, grant, 1, &i).await.unwrap() }
    };
    // A subject that no current result holds is not a required check.
    let mut stranger = input.clone();
    stranger["subject"]["id"] = json!(Uuid::new_v4().to_string());
    let refused = dispatch("check-0", "check.request", stranger).await;
    assert_eq!(refused.outcome, "refused");
    assert_eq!(refused.output["reason"], "not_required");

    let accepted = dispatch("check-1", "check.request", input.clone()).await;
    assert_eq!(accepted.outcome, "accepted", "{accepted:?}");
    let handle = accepted.output["handle"].as_str().unwrap().to_string();
    let operation: Uuid = handle.parse().unwrap();
    let get = |n: u32| {
        dispatch(
            &format!("get-{n}"),
            "operation.get",
            json!({"handle": handle}),
        )
    };
    assert_eq!(get(1).await.outcome, "running");
    let state: String =
        sqlx::query_scalar("SELECT state FROM check_attempts WHERE operation_id=$1")
            .bind(operation)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, "queued");

    // The node that ran the attempt claims it, then reports one stop.
    let (status, claimed) = node_post(&f, "/v1/worker/checks/claim", json!({"limit": 16})).await;
    assert_eq!(status, StatusCode::OK, "{claimed}");
    let claimed = claimed["checks"].as_array().unwrap().clone();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0]["operation_id"], json!(operation));
    assert_eq!(claimed[0]["attempt_id"], json!(report.binding.attempt_id));
    assert_eq!(claimed[0]["source"]["hex"], json!(checkpoint));
    assert_eq!(claimed[0]["execution_profile"], "aq.check.sandbox/v1");
    let stop = json!({"operation_id": operation, "stop": {"kind": "exited", "code": 0},
        "executor_version": "aq-check-executor/1.0.0",
        "inputs_digest": format!("sha256:{}", "c".repeat(64)), "inputs_complete": true,
        "result_artifacts": [{"id": "check-log", "version": "1",
            "digest": format!("sha256:{}", "d".repeat(64))}]});
    let (status, body) = node_post(&f, "/v1/worker/checks/stop", stop.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Idempotent per operation: the same stop is accepted, a different one conflicts.
    assert_eq!(
        node_post(&f, "/v1/worker/checks/stop", stop.clone())
            .await
            .0,
        StatusCode::OK
    );
    let mut other = stop.clone();
    other["stop"]["code"] = json!(1);
    assert_eq!(
        node_post(&f, "/v1/worker/checks/stop", other).await.0,
        StatusCode::CONFLICT
    );

    // Evidence derives from the reported stop, bound to the attempt and source.
    let evidence: (String, String, String, Uuid) = sqlx::query_as("SELECT verdict,checkpoint,validator_digest,source_operation_id FROM profiled_check_evidence WHERE attempt_id=$1 AND check_id=$2")
        .bind(report.binding.attempt_id)
        .bind(check.id.as_str())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        evidence,
        (
            "passed".into(),
            checkpoint.clone(),
            check.validator.digest.as_str().to_owned(),
            source
        )
    );

    // operation.get completes from the executor-reported stop.
    let completed = get(2).await;
    assert_eq!(completed.outcome, "completed", "{completed:?}");
    assert_eq!(completed.output["outcome"], "passed");
    let output = &completed.output["output"];
    assert_eq!(
        output["receipt"]["exit"],
        json!({"code": 0, "stop": "exited"})
    );
    assert_eq!(
        output["receipt"]["executor_version"],
        "aq-check-executor/1.0.0"
    );
    // Nothing is left for the node to claim.
    let (_, again) = node_post(&f, "/v1/worker/checks/claim", json!({"limit": 1})).await;
    assert_eq!(again["checks"], json!([]));
}
