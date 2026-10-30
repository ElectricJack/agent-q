use std::net::SocketAddr;
use std::sync::Arc;

use aq_release::ReadinessReport;
use aq_server::config::Config;
use aq_server::identity::verifier_from_config;
use aq_server::test_support::TestDb;
use aq_server::{AppState, build_admin_router, build_worker_router, system_clock};
use axum::Router;
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

fn config() -> Config {
    toml::from_str(
        r#"
format_version = 1
environment = "development"
[server]
admin_bind = "127.0.0.1:0"
worker_bind = "127.0.0.1:0"
public_origin = "http://127.0.0.1:8080"
[auth]
mode = "dev"
dev_email = "owner@localhost"
[bootstrap]
owner_email = "owner@localhost"
enabled = true
"#,
    )
    .unwrap()
}

async fn state(db: &TestDb) -> AppState {
    let config = config();
    AppState {
        identity: verifier_from_config(&config.auth, reqwest::Client::new()),
        config: Arc::new(config),
        store: db.store().await,
        clock: system_clock(),
    }
}

async fn get(app: Router, peer: Option<&str>) -> (StatusCode, Value) {
    let app = match peer {
        Some(p) => app.layer(MockConnectInfo(p.parse::<SocketAddr>().unwrap())),
        None => app,
    };
    let resp = app
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn readyz_loopback_only() {
    let db = TestDb::create().await;
    let st = state(&db).await;
    for router in [
        build_admin_router(st.clone()),
        build_worker_router(st.clone()),
    ] {
        let (s, _) = get(router.clone(), Some("10.0.0.5:4000")).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        let (s, _) = get(router.clone(), None).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "no ConnectInfo fails closed");
        let (s, v) = get(router.clone(), Some("127.0.0.1:4000")).await;
        assert_eq!(s, StatusCode::OK);
        let report: ReadinessReport = serde_json::from_value(v).unwrap();
        assert_eq!(report.database, "ok");
        assert_eq!(report.version.to_string(), env!("CARGO_PKG_VERSION"));
        assert_eq!(report.state_format, aq_server::STATE_FORMAT_VERSION);
        assert_eq!(report.auth_mode, "dev");
        let (s, _) = get(router, Some("[::1]:4000")).await;
        assert_eq!(s, StatusCode::OK);
    }
}

#[tokio::test]
async fn readyz_reports_database_error() {
    let db = TestDb::create().await;
    let st = state(&db).await;
    st.store.close().await;
    let (s, v) = get(build_admin_router(st), Some("127.0.0.1:4000")).await;
    assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE);
    let report: ReadinessReport = serde_json::from_value(v).unwrap();
    assert_eq!(report.database, "error");
}

#[tokio::test]
async fn readyz_answers_loopback_over_real_listener() {
    let db = TestDb::create().await;
    let st = state(&db).await;
    let admin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let worker = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (a, w) = (admin.local_addr().unwrap(), worker.local_addr().unwrap());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(aq_server::run_with_listeners(st, admin, worker, async {
        let _ = rx.await;
    }));
    // The subcommand's stdout must be pure JSON even with trace logging on.
    let vault = tempfile::tempdir().unwrap();
    let toml = std::fs::read_to_string("../../vault.example/config.toml")
        .unwrap()
        .replace("0.0.0.0:8080", &a.to_string());
    std::fs::write(vault.path().join("config.toml"), toml).unwrap();
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_aq-server"))
        .env("RUST_LOG", "trace")
        .args(["readiness", "--vault"])
        .arg(vault.path())
        .output()
        .await
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: ReadinessReport = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout not pure JSON: {e}\n{}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    assert_eq!(report.database, "ok");
    for addr in [a, w] {
        let r = reqwest::get(format!("http://{addr}/readyz")).await.unwrap();
        assert_eq!(r.status(), 200);
        let report: ReadinessReport = r.json().await.unwrap();
        assert_eq!(report.database, "ok");
    }
    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}

#[tokio::test]
async fn ping_times_out_instead_of_hanging() {
    let db = TestDb::create().await;
    let st = state(&db).await;
    assert!(st.store.ping(std::time::Duration::from_secs(5)).await);
    assert!(!st.store.ping(std::time::Duration::from_nanos(1)).await);
}
