#![cfg(feature = "loopback-development")]
use axum::{Router, routing::post};
use serde_json::{Value, json};
use std::{
    process::Stdio,
    sync::{Arc, Mutex},
};
#[tokio::test]
async fn actual_cli_posts_saved_identity_once_with_csrf_and_preserves_unknown_error() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let data = received.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let expected_origin = origin.clone();
    let app = Router::new().route(
        "/api/notifications",
        post(
            move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| {
                let data = data.clone();
                let origin = expected_origin.clone();
                async move {
                    assert_eq!(headers["x-aq-request"], "1");
                    assert_eq!(headers["origin"], origin);
                    assert_eq!(headers["x-aq-dev-email"], "owner@localhost");
                    data.lock().unwrap().push(body);
                    (
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        "private-secret-body-must-not-be-printed",
                    )
                }
            },
        ),
    );
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("send.json");
    let id = uuid::Uuid::new_v4();
    let body = json!({"operation_id":id,"message_id":uuid::Uuid::new_v4(),"revision":1,"scope":{"project_id":uuid::Uuid::new_v4(),"workspace_id":uuid::Uuid::new_v4()},"recipient_user_id":uuid::Uuid::new_v4(),"summary":"Please inspect the review","ttl_ms":60000});
    std::fs::write(&path, serde_json::to_vec(&body).unwrap()).unwrap();
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_aq-notify"))
        .args([
            "--origin",
            &origin,
            "--dev-email",
            "owner@localhost",
            "send",
            "--request",
        ])
        .arg(&path)
        .stdin(Stdio::null())
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    let err = String::from_utf8(output.stderr).unwrap();
    assert!(err.contains("not retried automatically"));
    assert!(!err.contains("private-secret-body"));
    assert_eq!(*received.lock().unwrap(), vec![body]);
    server.abort();
}
#[tokio::test]
async fn actual_cli_refuses_malformed_or_oversized_saved_requests_without_http() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.json");
    for bytes in [b"{\"unexpected\":true}".to_vec(), vec![b' '; 16385]] {
        std::fs::write(&path, bytes).unwrap();
        let result = tokio::process::Command::new(env!("CARGO_BIN_EXE_aq-notify"))
            .args([
                "--origin",
                "http://127.0.0.1:1",
                "--dev-email",
                "owner@localhost",
                "send",
                "--request",
            ])
            .arg(&path)
            .output()
            .await
            .unwrap();
        assert!(!result.status.success());
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.contains("request") || error.contains("JSON"));
        assert!(!error.contains("transport"));
    }
}
