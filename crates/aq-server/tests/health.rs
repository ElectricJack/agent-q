use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_ok() {
    let resp = aq_server::health_router()
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json, serde_json::json!({"status": "ok"}));
}

#[test]
fn version_info_reports_protocol_range() {
    let v = aq_server::version_info();
    assert_eq!((v.protocol_min, v.protocol_max), (1, 1));
    assert_eq!(v.server_version, env!("CARGO_PKG_VERSION"));
}
