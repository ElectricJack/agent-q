//! Dashboard aggregate HTTP behavior with disposable owner fixtures.
mod common;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn read(app: &Router, path: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::get(path)
                .header("x-aq-dev-email", "owner@localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn quota_unknown_is_not_zero() {
    use aq_selection::{ArtifactRef, AuthorizedCredential, Text};
    use aq_server::intelligence::config::{CredentialConfig, IntelligenceConfig};
    use std::sync::Arc;
    let mut f = common::profiled::fixture().await;
    let mut config = (*f.state.config).clone();
    let credential = AuthorizedCredential {
        profile: ArtifactRef {
            id: Text::new("profile").unwrap(),
            version: Text::new("1").unwrap(),
            digest: aq_admission::Digest::of(b"public"),
        },
        provider: "unsupported".into(),
        account_reference: "account".into(),
        endpoint_reference: "private-search-canary".into(),
        revision: 1,
        enabled: true,
        provisionable: false,
        material_owner: None,
    };
    let mut second = credential.clone();
    second.profile.id = Text::new("other-profile").unwrap();
    let credentials = [credential, second]
        .into_iter()
        .map(|credential| CredentialConfig {
            credential,
            allowed_node_ids: vec![],
            secret_file: None,
            delivery_mode: None,
        })
        .collect();
    config.intelligence = Some(Arc::new(
        IntelligenceConfig {
            revision: 1,
            credentials,
            ..Default::default()
        }
        .load()
        .unwrap(),
    ));
    f.state.config = Arc::new(config);
    let pool = f.db.pool().await;
    sqlx::query("INSERT INTO users(id,email,role,issuer,subject,created_at) VALUES($1,'quota-viewer@localhost','viewer','dev','quota-viewer@localhost',now())")
        .bind(uuid::Uuid::new_v4()).execute(&pool).await.unwrap();
    let app = aq_server::build_admin_router(f.state.clone());
    let viewer_response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/provider-usage")
                .header("x-aq-dev-email", "quota-viewer@localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(viewer_response.status(), StatusCode::FORBIDDEN);
    let (status, body) = read(&app, "/api/v1/provider-usage").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let accounts = body["data"].as_array().unwrap();
    assert_eq!(accounts.len(), 1);
    let usage = &accounts[0];
    assert_eq!(usage["scope"], "provider_account");
    let window = &usage["windows"][0];
    assert_eq!(window["state"], "unknown");
    for field in ["used_percent", "used", "limit"] {
        assert!(window[field].is_null(), "{field}: {window}");
    }
    assert!(!body.to_string().contains("private-search-canary"));
    let (_, other) = read(
        &app,
        &format!(
            "/api/v1/provider-usage?scope=project:{}",
            uuid::Uuid::new_v4()
        ),
    )
    .await;
    assert_eq!(body["data"], other["data"]);
    let (_, bootstrap) = read(&app, "/api/v1/bootstrap").await;
    assert!(
        bootstrap["data"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "provider.quota" && v["enabled"] == true)
    );
}

#[tokio::test]
async fn overview_empty_and_bounded_project_sources_are_truthful() {
    use serde_json::json;
    use uuid::Uuid;
    let f = common::profiled::fixture().await;
    let app = aq_server::build_admin_router(f.state.clone());
    let (status, empty) = read(&app, "/api/v1/overview").await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["data"]["counts"]["running"]["value"], 0);
    assert_eq!(empty["data"]["counts"]["running"]["accuracy"], "exact");
    assert!(empty["data"]["counts"]["ready"]["value"].is_null());
    assert!(empty["data"]["counts"]["needs_you"]["value"].is_null());
    assert!(empty["data"]["attention"].as_array().unwrap().is_empty());
    common::profiled::insert(&f).await;
    let pool = f.db.pool().await;
    for _ in 0..9 {
        let project = Uuid::new_v4();
        sqlx::query("INSERT INTO project_execution_bindings(project_id,revision,binding,validated_at_ms,binding_observation) SELECT $1,revision,jsonb_set(binding,'{project_id}',$2),validated_at_ms,binding_observation FROM project_execution_bindings WHERE project_id=$3").bind(project).bind(json!(project)).bind(f.selection.lineage.project_id).execute(&pool).await.unwrap();
    }
    let (_, bounded) = read(&app, "/api/v1/overview").await;
    assert_eq!(bounded["data"]["in_motion"].as_array().unwrap().len(), 8);
    assert_eq!(
        bounded["data"]["counts"]["running"]["accuracy"],
        "estimated"
    );
    assert!(
        bounded["data"]["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["source"] == "projects"
                && s["coverage"]["reason"] == "project_sample_truncated")
    );
}

#[tokio::test]
async fn overview_ready_owner_preserves_samples_failures_and_binding_fence() {
    use serde_json::json;
    use std::{
        os::unix::fs::PermissionsExt,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    let mut f = common::profiled::fixture().await;
    common::profiled::insert(&f).await;
    let mode = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let workspace = f.profiled.graph.workspace_id;
    let mut node: Value = serde_json::from_str::<Value>(include_str!(
        "../../../protocol/human-graph/v1/examples/task.json"
    ))
    .unwrap()["data"]
        .clone();
    node["workspace_id"] = json!(workspace);
    node["metadata"] = json!({});
    node["version"] = json!(1);
    node["content_revision"] = json!(1);
    let server_mode = mode.clone();
    let reached = entered.clone();
    let proceed = release.clone();
    let graph = Router::new().route(
        "/v1/workspaces/{workspace}/ready",
        axum::routing::get(move || {
            let node = node.clone();
            let mode = server_mode.clone();
            let reached = reached.clone();
            let proceed = proceed.clone();
            async move {
                let mode = mode.load(Ordering::SeqCst);
                if mode == 3 {
                    reached.notify_one();
                    proceed.notified().await;
                }
                (
                    if mode == 2 {
                        StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        StatusCode::OK
                    },
                    [("X-AQ-Graph-API-Version", "1")],
                    axum::Json(
                        json!({"items":[node],"next_cursor":if mode==1 {Some("next")} else {None}}),
                    ),
                )
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, graph).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let token = dir.path().join("token");
    std::fs::write(&token, "s".repeat(32)).unwrap();
    std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
    f.state.agent_access = Some(Arc::new(
        aq_server::agent_access::Runtime::new(&aq_server::agent_access::AgentAccessConfig {
            graph_url: origin,
            graph_token_file: token,
            mutation_actor_id: None,
            credential_ttl_ms: 120000,
        })
        .await
        .unwrap(),
    ));
    let app = aq_server::build_admin_router(f.state.clone());
    for (value, accuracy) in [(0, "exact"), (1, "estimated")] {
        mode.store(value, Ordering::SeqCst);
        let (status, body) = read(&app, "/api/v1/overview").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["counts"]["ready"]["value"], 1, "{body}");
        assert_eq!(body["data"]["counts"]["ready"]["accuracy"], accuracy);
        // The active Graph task is ready; it does not become a running attempt.
        assert_eq!(body["data"]["counts"]["running"]["value"], 0);
        assert!(
            !body["data"]["attention"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["kind"] == "blocked")
        );
        assert!(
            body["data"]["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["source"] == "graph_ready"
                    && v["observation"]["observed_at"] == "1970-01-01T00:16:40.000Z")
        );
    }
    mode.store(2, Ordering::SeqCst);
    let (_, body) = read(&app, "/api/v1/overview").await;
    assert!(body["data"]["counts"]["ready"]["value"].is_null());
    assert_eq!(body["data"]["counts"]["ready"]["accuracy"], "unknown");
    mode.store(3, Ordering::SeqCst);
    let pending_app = app.clone();
    let pending = tokio::spawn(async move { read(&pending_app, "/api/v1/overview").await });
    entered.notified().await;
    sqlx::query("UPDATE project_execution_bindings SET revision=revision+1 WHERE project_id=$1")
        .bind(f.selection.lineage.project_id)
        .execute(&f.db.pool().await)
        .await
        .unwrap();
    release.notify_one();
    assert_eq!(pending.await.unwrap().0, StatusCode::FORBIDDEN);
    server.abort();
}

#[tokio::test]
async fn overview_workers_use_configured_threshold_and_project_association() {
    use serde_json::json;
    use std::sync::Arc;
    let mut f = common::profiled::fixture().await;
    common::profiled::insert(&f).await;
    let pool = f.db.pool().await;
    sqlx::query(
        "UPDATE nodes SET last_seen_at=to_timestamp($2::double precision/1000) WHERE id=$1",
    )
    .bind(f.node.node_id)
    .bind((common::profiled::NOW - 61_000) as i64)
    .execute(&pool)
    .await
    .unwrap();
    let mut config = (*f.state.config).clone();
    config.workers.offline_after_secs = 120;
    f.state.config = Arc::new(config.clone());
    let app = aq_server::build_admin_router(f.state.clone());
    let (_, body) = read(&app, "/api/v1/overview").await;
    assert!(
        !body["data"]["attention"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["kind"] == "worker_offline")
    );
    config.workers.offline_after_secs = 60;
    f.state.config = Arc::new(config);
    let app = aq_server::build_admin_router(f.state.clone());
    let (_, body) = read(&app, "/api/v1/overview").await;
    let worker = body["data"]["attention"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["kind"] == "worker_offline")
        .expect("scoped stale worker");
    let revision: String = sqlx::query_scalar("SELECT xmin::text FROM nodes WHERE id=$1")
        .bind(f.node.node_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(worker["subject"]["id"], json!(f.node.node_id));
    assert_eq!(worker["subject"]["revision"], revision);
    assert_eq!(
        worker["project"]["id"],
        json!(f.selection.lineage.project_id)
    );
    assert_eq!(worker["since"], "1970-01-01T00:16:39.000Z");
    assert!(worker["action"].is_null());
    let (_, excluded) = read(
        &app,
        &format!("/api/v1/overview?scope=project:{}", uuid::Uuid::new_v4()),
    )
    .await;
    assert!(!excluded.to_string().contains(&f.node.node_id.to_string()));
}

#[tokio::test]
async fn overview_uncertain_operations_are_principal_and_scope_bound() {
    use serde_json::json;
    use uuid::Uuid;
    let f = common::profiled::fixture().await;
    common::profiled::insert(&f).await;
    let pool = f.db.pool().await;
    let other = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,role,issuer,subject,created_at) VALUES($1,'other@localhost','owner','dev','other@localhost',now())").bind(other).execute(&pool).await.unwrap();
    let own = Uuid::new_v4();
    let foreign = Uuid::new_v4();
    let scope = json!({"kind":"project","project_id":f.selection.lineage.project_id});
    let digest = format!("sha256:{}", "a".repeat(64));
    // Synthetic retained journal fixtures, read through the journal owner. These
    // do not claim a dispatched domain action or manufacture domain receipts.
    for (id, principal) in [(own, f.actor), (foreign, other)] {
        let response = json!({"operation_id":id,"disposition":"unknown","request_digest":digest,"scope":scope,"created_at":"1970-01-01T00:16:40.000Z","updated_at":"1970-01-01T00:16:40.000Z","subject_refs":[],"receipt_refs":[],"affected_entities":[],"recovery":"lookup_original"});
        sqlx::query("INSERT INTO dashboard_operations(operation_id,principal_id,kind,scope,request_digest,disposition,response,created_at,updated_at) VALUES($1,$2,'task.create',$3,$4,'unknown',$5,to_timestamp(1000),to_timestamp(1000))").bind(id).bind(principal).bind(&scope).bind(&digest).bind(response).execute(&pool).await.unwrap();
    }
    let app = aq_server::build_admin_router(f.state.clone());
    let (_, body) = read(&app, "/api/v1/overview").await;
    let operation = body["data"]["attention"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["kind"] == "uncertain_operation")
        .unwrap();
    assert_eq!(operation["subject"]["id"], json!(own));
    assert_eq!(operation["subject"]["revision"], digest);
    assert_eq!(body["pending_operations"], json!([own]));
    assert!(!body.to_string().contains(&foreign.to_string()));
    let (_, excluded) = read(
        &app,
        &format!("/api/v1/overview?scope=project:{}", Uuid::new_v4()),
    )
    .await;
    assert!(!excluded.to_string().contains(&own.to_string()));
}
