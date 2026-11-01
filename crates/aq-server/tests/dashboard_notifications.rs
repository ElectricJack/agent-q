//! Local-development notification owner qualification. Disposable persisted
//! messages have no endpoint or route, and cannot deliver outside this process.
mod common;

use aq_lifecycle_api::Scope;
use aq_server::notifications::persistence::SendRequest;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

async fn read(app: &Router, path: &str, email: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::get(path)
                .header("x-aq-dev-email", email)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&bytes)})),
    )
}

fn notification_capability(bootstrap: &Value) -> &Value {
    bootstrap["data"]["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cap| cap["name"] == "notifications.read")
        .unwrap()
}

#[tokio::test]
async fn mounted_notifications_read_current_principal_self_without_external_delivery() {
    let f = common::profiled::fixture().await;
    assert!(f.state.config.notifications.is_none());
    let viewer = Uuid::new_v4();
    let pool = f.db.pool().await;
    sqlx::query("INSERT INTO users(id,email,role,issuer,subject,created_at) VALUES($1,'notification-viewer@localhost','viewer','dev','notification-viewer@localhost',now())")
        .bind(viewer).execute(&pool).await.unwrap();
    let scope = Scope {
        project_id: Uuid::new_v4(),
        workspace_id: Uuid::new_v4(),
    };
    sqlx::query("INSERT INTO project_execution_bindings(project_id,revision,binding,validated_at_ms,binding_observation) VALUES($1,1,$2,1,'{}')")
        .bind(scope.project_id).bind(json!({"enabled":true,"graph_workspace_id":scope.workspace_id}))
        .execute(&pool).await.unwrap();
    let service = aq_server::notifications::api::service(&f.state);
    service.install(Uuid::new_v4()).await.unwrap();
    let mut owner_message = None;
    let mut viewer_message = None;
    for recipient in [f.actor, viewer] {
        let request = SendRequest {
            operation_id: Uuid::new_v4(),
            message_id: Uuid::new_v4(),
            revision: 1,
            scope: scope.clone(),
            recipient_user_id: recipient,
            summary: if recipient == f.actor {
                "Local fixture: review the deployment plan"
            } else {
                "Local fixture: viewer private notification"
            }
            .into(),
            ttl_ms: 60_000,
        };
        let receipt = service.enqueue(f.actor, request.clone()).await.unwrap();
        assert_eq!(receipt["external_state"], "no_external_route");
        assert_eq!(receipt["delivery_ids"], json!([]));
        if recipient == f.actor {
            owner_message = Some(request.message_id);
        } else {
            viewer_message = Some(request.message_id);
        }
    }
    let owner_message = owner_message.unwrap();
    let viewer_message = viewer_message.unwrap();
    let app = aq_server::build_admin_router(f.state.clone());
    let standalone = aq_server::dashboard_api::routes(f.state.clone()).with_state(f.state.clone());
    assert_eq!(
        read(&standalone, "/api/notifications", "owner@localhost")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    for email in ["owner@localhost", "notification-viewer@localhost"] {
        let (status, bootstrap) = read(&app, "/api/v1/bootstrap", email).await;
        assert_eq!(status, StatusCode::OK, "{bootstrap}");
        let cap = notification_capability(&bootstrap);
        assert_eq!(cap["enabled"], true);
        assert_eq!(cap["version"], "1");
        let (_, bootstrap) = read(&standalone, "/api/v1/bootstrap", email).await;
        assert_eq!(notification_capability(&bootstrap)["enabled"], false);
        let (status, inbox) = read(&app, "/api/notifications", email).await;
        assert_eq!(status, StatusCode::OK, "{inbox}");
        assert_eq!(inbox["items"].as_array().unwrap().len(), 1);
        let expected = if email == "owner@localhost" {
            owner_message
        } else {
            viewer_message
        };
        assert_eq!(
            inbox["items"][0]["message"]["message_id"],
            expected.to_string()
        );
        let (status, detail) = read(&app, &format!("/api/notifications/{expected}"), email).await;
        assert_eq!(status, StatusCode::OK, "{detail}");
        assert_eq!(detail["external_state"], "no_external_route");
        assert_eq!(detail["deliveries"], json!([]));
    }
    assert_eq!(
        read(
            &app,
            &format!("/api/notifications?recipient={}", f.actor),
            "notification-viewer@localhost"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        read(
            &app,
            &format!("/api/notifications/{owner_message}"),
            "notification-viewer@localhost"
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    optional_browser(&f, owner_message).await;
    let destinations: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_destinations")
        .fetch_one(&pool)
        .await
        .unwrap();
    let deliveries: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_deliveries")
        .fetch_one(&pool)
        .await
        .unwrap();
    let routes: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_routes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!((destinations, deliveries, routes), (0, 0, 0));
}

/// Opt-in only: serve this actual persisted notification owner and an explicitly
/// supplied verified bundle. Ordinary tests need no sibling frontend or Node.
async fn optional_browser(f: &common::profiled::Fixture, message: Uuid) {
    let Some(script) = std::env::var_os("AQ_TEST_NOTIFICATION_BROWSER_SCRIPT") else {
        assert!(std::env::var_os("AQ_TEST_NOTIFICATION_DASHBOARD_DIR").is_none());
        return;
    };
    let bundle = std::env::var_os("AQ_TEST_NOTIFICATION_DASHBOARD_DIR")
        .expect("explicit notification probe requires a compiled dashboard bundle");
    let assets =
        aq_server::dashboard_assets::DashboardAssets::load(std::path::Path::new(&bundle)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let mut state = f.state.clone();
    let mut config = (*state.config).clone();
    config.server.public_origin = origin.clone();
    state.config = Arc::new(config);
    let router = aq_server::build_admin_router_with_dashboard_and_policy_owners(
        state,
        None,
        None,
        Arc::new(assets),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut probe = tokio::process::Command::new("node")
        .arg(script)
        .env("AQ_LIVE_URL", origin)
        .env("AQ_LIVE_NOTIFICATION_ID", message.to_string())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(25), probe.wait()).await;
    server.abort();
    assert!(
        result
            .expect("bounded actual notification browser probe")
            .unwrap()
            .success()
    );
}
