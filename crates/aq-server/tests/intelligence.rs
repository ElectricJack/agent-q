//! Synthetic owner registry qualification. No model/provider calls or real keys.
use aq_selection::{ArtifactRef, AuthorizedCredential, Digest, Text};
use aq_server::{
    AppState,
    config::Config,
    identity::verifier_from_config,
    intelligence::config::{CredentialConfig, IntelligenceConfig},
    test_support::TestDb,
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{os::unix::fs::PermissionsExt, sync::Arc};
use tower::ServiceExt;
use uuid::Uuid;
fn credential() -> AuthorizedCredential {
    AuthorizedCredential {
        profile: ArtifactRef {
            id: Text::new("fixture-account").unwrap(),
            version: Text::new("1").unwrap(),
            digest: Digest::of(b"fixture-credential-definition"),
        },
        provider: "synthetic-provider".into(),
        account_reference: "account-a".into(),
        endpoint_reference: "endpoint-a".into(),
        revision: 1,
        enabled: true,
        provisionable: false,
        material_owner: None,
    }
}
fn registry(path: Option<std::path::PathBuf>) -> IntelligenceConfig {
    IntelligenceConfig {
        revision: 1,
        credentials: vec![CredentialConfig {
            credential: credential(),
            allowed_node_ids: vec![Uuid::from_u128(10)],
            secret_file: path,
        }],
        qualified_harnesses: vec![],
    }
}
fn config() -> Config {
    toml::from_str(
        r#"format_version=1
environment="development"
[server]
admin_bind="127.0.0.1:0"
worker_bind="127.0.0.1:0"
public_origin="http://127.0.0.1:8080"
[auth]
mode="dev"
dev_email="owner@localhost"
[bootstrap]
owner_email="owner@localhost"
enabled=true
"#,
    )
    .unwrap()
}
async fn setup(reg: IntelligenceConfig) -> (TestDb, AppState, Router) {
    let db = TestDb::create().await;
    let store = db.store().await;
    let time = Utc.timestamp_millis_opt(1_000_000).unwrap();
    store
        .bootstrap_owner("owner@localhost", "dev", "owner@localhost", time)
        .await
        .unwrap();
    store
        .invite_user("viewer@localhost", aq_server::store::Role::Viewer, time)
        .await
        .unwrap();
    let mut cfg = config();
    cfg.intelligence = Some(Arc::new(reg.load().unwrap()));
    let state = AppState {
        identity: verifier_from_config(&cfg.auth, reqwest::Client::new()),
        config: Arc::new(cfg),
        store,
        clock: Arc::new(|| Utc.timestamp_millis_opt(1_000_000).unwrap()),
        agent_access: None,
        source_retention: None,
    };
    (db, state.clone(), aq_server::build_admin_router(state))
}
async fn call(
    app: &Router,
    path: &str,
    body: Option<Value>,
    owner: bool,
    csrf: bool,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header("content-type", "application/json")
        .header(
            "x-aq-dev-email",
            if owner {
                "owner@localhost"
            } else {
                "viewer@localhost"
            },
        );
    if csrf {
        req = req.header("X-AQ-Request", "1");
    }
    let response = app
        .clone()
        .oneshot(
            req.body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
fn observation() -> Value {
    let c = credential();
    json!({"operation_id":Uuid::new_v4(),"expected_revision":1,"sequence":1,"scope":{"credential_revision":1,"provider":c.provider,"credential_profile":c.profile,"account_reference":c.account_reference,"endpoint_reference":c.endpoint_reference,"model":"model-a","node_id":null},"state":"quota_unavailable","ttl_ms":1000,"cooldown_ms":1000})
}
#[test]
fn credential_loading_is_private_bounded_regular_and_never_claims_missing_keys() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("key");
    assert!(!registry(Some(file.clone())).load().unwrap().credentials[0].provisionable);
    std::fs::write(&file, b"synthetic-only-key").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let loaded = registry(Some(file.clone())).load().unwrap();
    assert!(loaded.credentials[0].provisionable);
    assert!(!format!("{loaded:?}").contains("synthetic-only-key"));
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(registry(Some(file.clone())).load().is_err());
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(&file, vec![b'x'; 65_537]).unwrap();
    assert!(registry(Some(file.clone())).load().is_err());
    std::fs::remove_file(&file).unwrap();
    nix::unistd::mkfifo(
        &file,
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )
    .unwrap();
    assert!(registry(Some(file.clone())).load().is_err());
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink("missing", &file).unwrap();
    assert!(registry(Some(file)).load().is_err());
}
#[tokio::test]
async fn manual_observation_has_authenticated_provenance_exact_replay_and_expiry_tombstones() {
    let (_db, state, app) = setup(registry(None)).await;
    assert_eq!(
        call(
            &app,
            "/api/intelligence/register",
            Some(json!({})),
            false,
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app,
            "/api/intelligence/register",
            Some(json!({})),
            true,
            false
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(
            &app,
            "/api/intelligence/register",
            Some(json!({})),
            true,
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    let body = observation();
    let (status, first) = call(
        &app,
        "/api/intelligence/observations",
        Some(body.clone()),
        true,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["source"]["kind"], "operator");
    assert_eq!(first["revision"], 2);
    assert_eq!(
        call(
            &app,
            "/api/intelligence/observations",
            Some(body.clone()),
            true,
            true
        )
        .await
        .1,
        first
    );
    let mut changed = body.clone();
    changed["state"] = "available".into();
    assert_eq!(
        call(
            &app,
            "/api/intelligence/observations",
            Some(changed),
            true,
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut stale = body.clone();
    stale["operation_id"] = json!(Uuid::new_v4());
    stale["expected_revision"] = 2.into();
    assert_eq!(
        call(
            &app,
            "/api/intelligence/observations",
            Some(stale.clone()),
            true,
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    stale["sequence"] = 2.into();
    stale["scope"]["account_reference"] = "foreign-account".into();
    assert_eq!(
        call(
            &app,
            "/api/intelligence/observations",
            Some(stale),
            true,
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut unknown = body.clone();
    unknown["principal_id"] = json!(Uuid::new_v4());
    assert_eq!(
        call(
            &app,
            "/api/intelligence/observations",
            Some(unknown),
            true,
            true
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut reconfigured = (*state.config).clone();
    let mut empty = registry(None);
    empty.revision = 2;
    empty.credentials.clear();
    reconfigured.intelligence = Some(Arc::new(empty.load().unwrap()));
    let app2 = aq_server::build_admin_router(AppState {
        config: Arc::new(reconfigured),
        ..state.clone()
    });
    assert_eq!(
        call(
            &app2,
            "/api/intelligence/register",
            Some(json!({})),
            true,
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app2,
            "/api/intelligence/observations",
            Some(body),
            true,
            true
        )
        .await
        .1,
        first
    );
    let (_, view) = call(&app2, "/api/intelligence/registry", None, true, true).await;
    assert_eq!(view["accounting"], "unavailable");
    assert_eq!(view["credentials"], json!([]));
    assert!(!view.to_string().contains("private_fingerprints"));
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("rotation");
    std::fs::write(&key, b"synthetic-rotated-key").unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut rotated = registry(Some(key));
    rotated.revision = 3;
    let mut cfg = (*state.config).clone();
    cfg.intelligence = Some(Arc::new(rotated.load().unwrap()));
    let app3 = aq_server::build_admin_router(AppState {
        config: Arc::new(cfg),
        ..state.clone()
    });
    assert_eq!(
        call(
            &app3,
            "/api/intelligence/register",
            Some(json!({})),
            true,
            true
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    rotated.credentials[0].credential.revision = 2;
    let mut cfg = (*state.config).clone();
    cfg.intelligence = Some(Arc::new(rotated.load().unwrap()));
    let app4 = aq_server::build_admin_router(AppState {
        config: Arc::new(cfg),
        ..state
    });
    assert_eq!(
        call(
            &app4,
            "/api/intelligence/register",
            Some(json!({})),
            true,
            true
        )
        .await
        .0,
        StatusCode::OK
    );
}

async fn machine(app: &Router, path: &str, credential: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {credential}"))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn installed_inventory_is_current_boot_bound_does_not_authorize_keys_and_legacy_feedback_refuses()
 {
    let (_db, mut state, _) = setup(registry(None)).await;
    let time = (state.clock)();
    let owner: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE role='owner'")
        .fetch_one(&_db.pool().await)
        .await
        .unwrap();
    let token = aq_server::tokens::new_node_credential();
    state
        .store
        .create_enrollment(
            "intelligence-fixture",
            owner,
            time,
            time + chrono::Duration::minutes(5),
        )
        .await
        .unwrap();
    let node = state
        .store
        .redeem_enrollment(
            "intelligence-fixture",
            aq_server::store::NewNode {
                name: "fixture".into(),
                credential_hash: aq_server::tokens::verifier(&token),
                worker_version: "0.1.0".into(),
                protocol_version: 1,
            },
            time,
        )
        .await
        .unwrap()
        .unwrap();
    let identity = aq_worker_protocol::NodeIdentity {
        node_id: node.id,
        boot_id: Uuid::new_v4(),
    };
    state
        .store
        .execution_register(&aq_worker_protocol::RegisterRequest {
            identity: identity.clone(),
            operation_id: Uuid::new_v4(),
            protocol_version: 2,
            worker_version: "0.1.0".into(),
            capabilities: vec![aq_worker_protocol::FAKE_HARNESS_CAPABILITY.into()],
        })
        .await
        .unwrap();
    let mut reg = registry(None);
    reg.credentials[0].allowed_node_ids = vec![node.id];
    let mut cfg = (*state.config).clone();
    cfg.intelligence = Some(Arc::new(reg.load().unwrap()));
    state.config = Arc::new(cfg);
    let admin = aq_server::build_admin_router(state.clone());
    assert_eq!(
        call(
            &admin,
            "/api/intelligence/register",
            Some(json!({})),
            true,
            true
        )
        .await
        .0,
        StatusCode::OK
    );
    let worker = aq_server::build_worker_router(state.clone());
    let c = credential();
    let body = json!({"operation_id":Uuid::new_v4(),"boot_id":identity.boot_id,"sequence":1,"ttl_ms":1000,"qualifications":[],"credentials":[{"profile":c.profile,"credential_revision":1,"available":true}]});
    let (status, first) = machine(
        &worker,
        "/v1/worker/intelligence/inventory",
        &token,
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["credential_bindings"][0]["available"], true);
    // Node presence is separate from owner missing-key provisioning authority.
    let (_, view) = call(&admin, "/api/intelligence/registry", None, true, true).await;
    assert_eq!(view["credentials"][0]["provisionable"], false);
    assert_eq!(
        machine(
            &worker,
            "/v1/worker/intelligence/inventory",
            &token,
            body.clone()
        )
        .await
        .1,
        first
    );
    let mut changed = body.clone();
    changed["ttl_ms"] = 2000.into();
    assert_eq!(
        machine(
            &worker,
            "/v1/worker/intelligence/inventory",
            &token,
            changed
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let mut stale = body.clone();
    stale["operation_id"] = json!(Uuid::new_v4());
    stale["boot_id"] = json!(Uuid::new_v4());
    assert_eq!(
        machine(&worker, "/v1/worker/intelligence/inventory", &token, stale)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let feedback = json!({"schema_version":1,"operation_id":Uuid::new_v4(),"attempt_id":Uuid::new_v4(),"selection_digest":Digest::of(b"unsealed"),"node_id":node.id,"boot_id":identity.boot_id,"classifier":c.profile,"sequence":1,"failure":"quota","lifecycle_event_id":null});
    let (status, error) =
        machine(&worker, "/v1/worker/intelligence/failure", &token, feedback).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(error.to_string().contains("no actual sealed"));
    assert_eq!(
        machine(
            &worker,
            "/v1/worker/intelligence/inventory",
            "aqn_foreign",
            body
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
