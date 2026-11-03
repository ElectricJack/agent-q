//! Durable delegated authority, exercised with the injected server clock.
mod common;
use aq_admission_sandbox::sandbox::*;
use aq_server::delegated_permits::{CredentialKey, runtime};
use axum::http::HeaderMap;
use uuid::Uuid;

async fn setup() -> (
    common::profiled::Fixture,
    DelegatedPermit,
    PhaseExecutionAuthority,
) {
    let (f, start) = common::native_started::setup(true).await;
    let authority = PhaseExecutionAuthority {
        profile: PhaseProfile::PhaseV2,
        receipt_digest: aq_admission_sandbox::Digest::of(b"retained execution receipt"),
        grant: aq_session_protocol::ProfiledGrant {
            binding: start.binding.clone(),
            grant_id: start.grant_id,
            expires_at_ms: common::profiled::NOW + 1000,
        },
        issued_at_ms: common::profiled::NOW,
        hard_deadline_ms: common::profiled::NOW + 30_000,
    };
    let id = Uuid::new_v4();
    let p = DelegatedPermit {
        schema_version: 1,
        permit_id: id,
        credential: CredentialKey::new([7; 32]).credential(id).unwrap(),
        binding: PermitBinding {
            attempt_id: start.binding.attempt_id,
            grant_generation: start.binding.assignment_generation,
            sandbox_id: Uuid::new_v4(),
            channel_id: Uuid::new_v4(),
            node: aq_admission_sandbox::transport::WorkerIdentity {
                node_id: f.node.node_id,
                boot_id: f.node.boot_id,
            },
        },
        scope: PermitScope::Execution,
        issued_at_ms: authority.issued_at_ms,
        expires_at_ms: authority.grant.expires_at_ms,
        hard_deadline_ms: authority.hard_deadline_ms,
    };
    let pool = f.db.pool().await;
    let mut tx = pool.begin().await.unwrap();
    runtime::install_execution_locked(&mut tx, Uuid::new_v4(), [42; 32], &p, &authority)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    (f, p, authority)
}
fn headers(p: &DelegatedPermit) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        "authorization",
        format!("Bearer {}", p.credential.expose()).parse().unwrap(),
    );
    h.insert(
        "aq-runner-channel",
        serde_json::to_string(&p.binding).unwrap().parse().unwrap(),
    );
    h
}
#[tokio::test]
async fn delegated_runner_scope_is_rechecked_after_authentication() {
    let (f, p, _) = setup().await;
    let h = headers(&p);
    let auth = runtime::authenticate_runner(
        &f.state,
        &h,
        RunnerOperation::Observe,
        Some(p.binding.attempt_id),
    )
    .await
    .unwrap();
    assert!(
        runtime::authenticate_runner(&f.state, &h, RunnerOperation::Observe, Some(Uuid::new_v4()))
            .await
            .is_err()
    );
    assert!(
        aq_server::worker_api::authenticate_context(&f.state, &h)
            .await
            .is_err()
    );
    f.time
        .store(p.expires_at_ms as i64, std::sync::atomic::Ordering::SeqCst);
    let pool = f.db.pool().await;
    let mut tx = pool.begin().await.unwrap();
    assert!(
        aq_server::worker_api::lock_current_node(&mut tx, &auth)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
}

#[allow(dead_code)]
#[path = "common/phase_refresh_terminal.rs"]
mod phase_refresh_fixture;

async fn request(
    state: &aq_server::AppState,
    path: &str,
    headers: &HeaderMap,
    body: serde_json::Value,
) -> axum::response::Response {
    use tower::ServiceExt;
    let mut request = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("aq-delivery-nonce", Uuid::new_v4().to_string());
    for (name, value) in headers {
        request = request.header(name, value);
    }
    aq_server::build_worker_router(state.clone())
        .oneshot(
            request
                .body(axum::body::Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn accept_issues_and_grant_atomically_upgrades_same_permit() {
    use aq_admission::preparation_refresh_v1::Document;
    let mut f = phase_refresh_fixture::prepared_terminal().await;
    let mut config = (*f.c.env.env.state.config).clone();
    config.delegated_permits = Some(std::sync::Arc::new(CredentialKey::new([7; 32])));
    f.c.env.env.state.config = std::sync::Arc::new(config);
    let state = &f.c.env.env.state;
    let q = PermitIssuanceRequest {
        schema_version: 1,
        operation_id: f.q.original.operation_id,
        profile: PhaseProfile::PhaseRefreshV1,
        accept_request_digest: serde_json::from_value(
            serde_json::to_value(f.q.digest().unwrap()).unwrap(),
        )
        .unwrap(),
        attempt_id: f.q.original.acceptance.binding.attempt_id,
        grant_generation: f.q.original.acceptance.binding.assignment_generation,
        sandbox_id: Uuid::new_v4(),
        node: serde_json::from_value(serde_json::to_value(&f.q.original.identity).unwrap())
            .unwrap(),
    };
    let mut h = HeaderMap::new();
    h.insert(
        "authorization",
        format!("Bearer {}", f.c.env.env.credential)
            .parse()
            .unwrap(),
    );
    h.insert(
        "aq-permit-issuance",
        serde_json::to_string(&q).unwrap().parse().unwrap(),
    );
    let body = serde_json::to_value(Document::Accept(Box::new(f.q.clone()))).unwrap();
    let response = request(state, "/v1/worker/profiled-v2/accept", &h, body.clone()).await;
    assert_eq!(response.status(), 200);
    let issued: PermitIssuanceResponse =
        serde_json::from_slice(response.headers()["aq-permit-issuance"].as_bytes()).unwrap();
    assert_eq!(response.headers()["aq-permit-revision"], "1");
    issued
        .validate_for(&q, (state.clock)().timestamp_millis() as u64)
        .unwrap();
    let replay = request(state, "/v1/worker/profiled-v2/accept", &h, body.clone()).await;
    assert_eq!(replay.status(), 200);
    assert!(response.headers()["aq-permit-issuance"] == replay.headers()["aq-permit-issuance"]);
    let mut changed = q.clone();
    changed.sandbox_id = Uuid::new_v4();
    h.insert(
        "aq-permit-issuance",
        serde_json::to_string(&changed).unwrap().parse().unwrap(),
    );
    assert_eq!(
        request(state, "/v1/worker/profiled-v2/accept", &h, body)
            .await
            .status(),
        409
    );
    let h = headers(&issued.permit);
    assert!(
        runtime::authenticate_runner(state, &h, RunnerOperation::Started, None)
            .await
            .is_err()
    );
    assert!(
        runtime::authenticate_runner(state, &h, RunnerOperation::Prepared, None)
            .await
            .is_ok()
    );
    let upgrade = PermitUpgradeRequest {
        schema_version: 1,
        operation_id: f.prepared.original.operation_id,
        profile: q.profile,
        prepare_request_digest: serde_json::from_value(
            serde_json::to_value(f.prepared.digest().unwrap()).unwrap(),
        )
        .unwrap(),
        accepted_receipt_digest: issued.authority.accepted_receipt_digest.clone(),
        permit_id: issued.permit.permit_id,
        binding: issued.permit.binding.clone(),
    };
    let mut h = h;
    h.insert(
        "aq-permit-upgrade",
        serde_json::to_string(&upgrade).unwrap().parse().unwrap(),
    );
    let body = serde_json::to_value(Document::Prepare(Box::new(f.prepared.clone()))).unwrap();
    let response = request(state, "/v1/worker/profiled-v2/prepare", &h, body.clone()).await;
    let status = response.status();
    if status != 200 {
        let bytes = axum::body::to_bytes(response.into_body(), 16384)
            .await
            .unwrap();
        panic!("prepare: {status} {}", String::from_utf8_lossy(&bytes));
    }
    let upgraded: PermitUpgradeResponse =
        serde_json::from_slice(response.headers()["aq-permit-upgrade"].as_bytes()).unwrap();
    let installed_revision: u64 = response.headers()["aq-permit-revision"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(installed_revision, 2);
    upgraded
        .validate_for(
            &upgrade,
            &issued.authority,
            &issued.permit,
            (state.clock)().timestamp_millis() as u64,
        )
        .unwrap();
    assert_eq!(upgraded.permit.permit_id, issued.permit.permit_id);
    assert!(upgraded.permit.credential.expose() == issued.permit.credential.expose());
    let replay = request(state, "/v1/worker/profiled-v2/prepare", &h, body).await;
    assert_eq!(replay.status(), 200);
    assert!(response.headers()["aq-permit-upgrade"] == replay.headers()["aq-permit-upgrade"]);
    assert!(
        runtime::authenticate_runner(
            state,
            &headers(&upgraded.permit),
            RunnerOperation::Started,
            None
        )
        .await
        .is_ok()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM delegated_permits")
        .fetch_one(&f.c.env.env.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let stored: String = sqlx::query_scalar("SELECT state::text FROM delegated_permit_current")
        .fetch_one(&f.c.env.env.pool)
        .await
        .unwrap();
    assert!(!stored.contains(upgraded.permit.credential.expose()));
    let bytes = axum::body::to_bytes(response.into_body(), 131072)
        .await
        .unwrap();
    let Document::ExecutionDelivery(delivery) =
        aq_admission::preparation_refresh_v1::decode(&bytes).unwrap()
    else {
        panic!("wrong execution response")
    };
    let started = phase_refresh_fixture::started_request(&f, &delivery.receipt);
    let mut started_headers = headers(&upgraded.permit);
    started_headers.insert("aq-phase-terminal-api-version", "1".parse().unwrap());
    let response = request(
        state,
        "/v1/worker/profiled-v2/started",
        &started_headers,
        serde_json::to_value(&started).unwrap(),
    )
    .await;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 16384)
        .await
        .unwrap();
    assert_eq!(status, 200, "started: {}", String::from_utf8_lossy(&bytes));
    exercise_same_boot_renewal(&f, &upgraded, &started, installed_revision).await;
}

async fn exercise_same_boot_renewal(
    f: &phase_refresh_fixture::Prepared,
    upgraded: &PermitUpgradeResponse,
    started: &aq_phase_terminal_protocol::refresh_v1::StartedRequest,
    installed_revision: u64,
) {
    use std::sync::atomic::Ordering;
    let state = &f.c.env.env.state;
    let clock = f.c.controlled_clock.as_ref().unwrap();
    clock.fetch_add(1, Ordering::SeqCst);
    let q = RunnerRenewalRequest {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        authority: upgraded.authority.clone(),
        permit: upgraded.permit.clone(),
        revision: installed_revision,
    };
    let body = serde_json::to_value(SandboxDocument::RunnerRenewalRequest(q.clone())).unwrap();
    let h = headers(&q.permit);
    let response = request(state, "/v1/worker/profiled/renew", &h, body.clone()).await;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 16384)
        .await
        .unwrap();
    assert_eq!(status, 200, "renew: {}", String::from_utf8_lossy(&bytes));
    let SandboxDocument::RunnerRenewalResponse(reply) = serde_json::from_slice(&bytes).unwrap()
    else {
        panic!("wrong response kind")
    };
    reply
        .validate_for(
            &q,
            &PermitContext {
                binding: q.permit.binding.clone(),
                phase: GrantPhase::Granted,
                grant_expires_at_ms: q.permit.expires_at_ms,
                hard_deadline_ms: q.permit.hard_deadline_ms,
            },
            reply.delivery.as_ref().unwrap().nonce,
            clock.load(Ordering::SeqCst) as u64,
        )
        .unwrap();
    let RenewalReply::Renewed {
        permit: renewed, ..
    } = &reply.reply
    else {
        panic!("not renewed")
    };
    assert_eq!(reply.revision, 3);
    assert!(renewed.expires_at_ms > q.permit.expires_at_ms);
    assert_eq!(renewed.hard_deadline_ms, q.permit.hard_deadline_ms);
    assert!(renewed.credential.expose() == q.permit.credential.expose());
    let mut reconstructed = state.clone();
    reconstructed.store = f.c.env.env._db.store().await;
    let replay = request(
        &reconstructed,
        "/v1/worker/profiled/renew",
        &h,
        body.clone(),
    )
    .await;
    assert_eq!(replay.status(), 200);
    let bytes = axum::body::to_bytes(replay.into_body(), 16384)
        .await
        .unwrap();
    let SandboxDocument::RunnerRenewalResponse(replay) = serde_json::from_slice(&bytes).unwrap()
    else {
        panic!("wrong replay kind")
    };
    assert_eq!(replay.revision, reply.revision);
    assert!(
        serde_json::to_value(&replay.reply).unwrap() == serde_json::to_value(&reply.reply).unwrap()
    );
    let mut stale = q.clone();
    stale.operation_id = Uuid::new_v4();
    assert_eq!(
        request(
            state,
            "/v1/worker/profiled/renew",
            &h,
            serde_json::to_value(SandboxDocument::RunnerRenewalRequest(stale)).unwrap()
        )
        .await
        .status(),
        409
    );
    let mut changed = q.clone();
    changed.revision += 1;
    assert_eq!(
        request(
            state,
            "/v1/worker/profiled/renew",
            &h,
            serde_json::to_value(SandboxDocument::RunnerRenewalRequest(changed)).unwrap()
        )
        .await
        .status(),
        409
    );
    let snapshot: serde_json::Value = sqlx::query_scalar("SELECT jsonb_build_object('hot',(SELECT state FROM profiled_authority_hot LIMIT 1),'permit',(SELECT state FROM delegated_permit_current LIMIT 1),'ops',(SELECT count(*) FROM delegated_operations))")
        .fetch_one(&f.c.env.env.pool).await.unwrap();
    assert_eq!(
        snapshot["hot"]["grant"]["expires_at_ms"],
        renewed.expires_at_ms
    );
    let retained: String =
        sqlx::query_scalar("SELECT evidence::text FROM delegated_operations WHERE kind='renewal'")
            .fetch_one(&f.c.env.env.pool)
            .await
            .unwrap();
    assert!(!retained.contains(renewed.credential.expose()));
    let stopped = phase_refresh_fixture::stop_request(started);
    let mut stop_headers = headers(renewed);
    stop_headers.insert("aq-phase-terminal-api-version", "1".parse().unwrap());
    let response = request(
        state,
        "/v1/worker/profiled-v2/stopped",
        &stop_headers,
        serde_json::to_value(stopped).unwrap(),
    )
    .await;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 16384)
        .await
        .unwrap();
    assert_eq!(
        status,
        200,
        "stopped after renewal: {}",
        String::from_utf8_lossy(&bytes)
    );
    assert_eq!(
        request(state, "/v1/worker/profiled/renew", &h, body)
            .await
            .status(),
        403
    );
    let after: serde_json::Value = sqlx::query_scalar("SELECT jsonb_build_object('hot',(SELECT state FROM profiled_authority_hot LIMIT 1),'permit',(SELECT state FROM delegated_permit_current LIMIT 1),'ops',(SELECT count(*) FROM delegated_operations))")
        .fetch_one(&f.c.env.env.pool).await.unwrap();
    assert_eq!(after["permit"], snapshot["permit"]);
    assert_eq!(after["ops"], snapshot["ops"]);
    assert_eq!(after["hot"]["status"], "finished");
}

#[tokio::test]
async fn reconstructed_auth_fences_revocation_and_stale_boot() {
    let (f, p, _) = setup().await;
    let h = headers(&p);
    let pool = f.db.pool().await;
    let mut restarted = f.state.clone();
    restarted.store = f.db.store().await;
    runtime::authenticate_runner(&restarted, &h, RunnerOperation::Observe, None)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO delegated_node_boots(node_id,boot_id,registration) VALUES($1,$2,'{}')",
    )
    .bind(p.binding.node.node_id)
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        runtime::authenticate_runner(&restarted, &h, RunnerOperation::Observe, None)
            .await
            .is_err()
    );
    sqlx::query("UPDATE delegated_node_boots SET boot_id=$2 WHERE node_id=$1")
        .bind(p.binding.node.node_id)
        .bind(p.binding.node.boot_id)
        .execute(&pool)
        .await
        .unwrap();
    let auth = runtime::authenticate_runner(&restarted, &h, RunnerOperation::Observe, None)
        .await
        .unwrap();
    sqlx::query("UPDATE nodes SET revoked_at=now() WHERE id=$1")
        .bind(p.binding.node.node_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        aq_server::worker_api::lock_current_node(&mut tx, &auth)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    assert!(
        runtime::authenticate_runner(&restarted, &h, RunnerOperation::Observe, None)
            .await
            .is_err()
    );
}
