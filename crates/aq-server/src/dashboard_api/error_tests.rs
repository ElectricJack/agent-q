use super::*;
#[tokio::test]
async fn common_error_envelope_bounds_strings_and_covers_required_statuses() {
    use http_body_util::BodyExt;
    for status in [401, 403, 404, 409, 410, 413, 422, 426, 429, 503] {
        let response = ApiError::for_status(StatusCode::from_u16(status).unwrap()).into_response();
        assert_eq!(response.status().as_u16(), status);
        let body: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(body["request_id"].is_string());
        assert!(body["message"].as_str().unwrap().len() <= 256);
        assert!(body["code"].as_str().unwrap().len() <= 64);
        assert!(body["retryable"].is_boolean());
    }
}

#[tokio::test]
async fn common_error_normalization_preserves_protocol_clock_and_retry_headers() {
    use axum::{Router, body::Body, http::Request, middleware::from_fn, routing::get};
    use tower::ServiceExt;
    let app = Router::new()
        .route(
            "/api/v1/example",
            get(|| async {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [("retry-after", "1"), ("aq-terminal-server-time-ms", "123")],
                    "private diagnostic",
                )
            }),
        )
        .layer(from_fn(normalize_errors));
    let response = app
        .oneshot(Request::get("/api/v1/example").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers().get("retry-after").unwrap(), "1");
    assert_eq!(
        response
            .headers()
            .get("aq-terminal-server-time-ms")
            .unwrap(),
        "123"
    );
}
