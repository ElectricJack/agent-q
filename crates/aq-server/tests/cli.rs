use std::process::Command;

use aq_server::config;
use aq_server::identity::verifier_from_config;
use aq_server::test_support::TestDb;
use aq_server::{AppState, run_with_listeners, system_clock};
use std::sync::Arc;

#[test]
fn serve_without_database_url_exits_2() {
    let out = Command::new(env!("CARGO_BIN_EXE_aq-server"))
        .args(["serve", "--vault", "../../vault.example"])
        .env_remove("DATABASE_URL")
        .env_remove("AQ_VAULT_DIR")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("DATABASE_URL is not set"), "{err}");
}

#[tokio::test]
async fn admin_and_worker_listen_on_separate_ports() {
    let db = TestDb::create().await;
    let store = db.store().await;
    let config = config::load(std::path::Path::new("../../vault.example")).unwrap();
    let state = AppState {
        identity: verifier_from_config(&config.auth, reqwest::Client::new()),
        config: Arc::new(config),
        store,
        clock: system_clock(),
    };
    let admin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let worker = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (a, w) = (admin.local_addr().unwrap(), worker.local_addr().unwrap());
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(run_with_listeners(state, admin, worker, async {
        let _ = rx.await;
    }));
    let http = reqwest::Client::new();

    let r = http
        .get(format!("http://{w}/api/status"))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
    let r = http
        .post(format!("http://{a}/v1/worker/enroll"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 404);
    assert_eq!(
        http.get(format!("http://{a}/healthz"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        http.get(format!("http://{w}/healthz"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    // Admin API is reachable on the admin port (dev identity).
    assert_eq!(
        http.get(format!("http://{a}/api/me"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );

    tx.send(()).unwrap();
    server.await.unwrap().unwrap();
}
