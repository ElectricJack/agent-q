use std::sync::Arc;

use aq_protocol::{EnrollRequest, HeartbeatRequest, PROTOCOL_VERSION};
use aq_server::config::Config;
use aq_server::identity::verifier_from_config;
use aq_server::store::Store;
use aq_server::test_support::TestDb;
use aq_server::{AppState, run_with_listeners, system_clock};
use aq_worker::client::{CallError, Client, run_loop};
use aq_worker::config::WorkerConfig;
use serde_json::Value;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

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

struct Running {
    admin: String,
    worker: String,
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl Running {
    async fn shutdown(self) {
        let _ = self.stop.send(());
        self.task.await.unwrap();
    }
}

async fn start(store: Store) -> Running {
    let cfg = config();
    let state = AppState {
        identity: verifier_from_config(&cfg.auth, reqwest::Client::new()),
        config: Arc::new(cfg),
        store,
        clock: system_clock(),
    };
    let a = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let w = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let admin = format!("http://{}", a.local_addr().unwrap());
    let worker = format!("http://{}", w.local_addr().unwrap());
    let (stop, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        run_with_listeners(state, a, w, async {
            let _ = rx.await;
        })
        .await
        .unwrap();
    });
    Running {
        admin,
        worker,
        stop,
        task,
    }
}

async fn admin_post(http: &reqwest::Client, url: String) -> reqwest::Response {
    http.post(url)
        .header("X-AQ-Request", "1")
        .send()
        .await
        .unwrap()
}

async fn workers(http: &reqwest::Client, admin: &str) -> Vec<Value> {
    http.get(format!("{admin}/api/workers"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

fn hb() -> HeartbeatRequest {
    HeartbeatRequest {
        worker_version: "0.1.0".into(),
        protocol_version: PROTOCOL_VERSION,
    }
}

#[tokio::test]
async fn lifecycle_enroll_heartbeat_restart_revoke() {
    let db = TestDb::create().await;
    let s1 = start(db.store().await).await;
    let http = reqwest::Client::new();

    // Bootstrap the owner (first dev login) and mint an enrollment code.
    assert!(
        http.get(format!("{}/api/me", s1.admin))
            .send()
            .await
            .unwrap()
            .status()
            .is_success()
    );
    let code: Value = admin_post(&http, format!("{}/api/enrollments", s1.admin))
        .await
        .json()
        .await
        .unwrap();
    let code = code["code"].as_str().unwrap().to_string();

    let enroller = Client::new(&s1.worker, None, None, None).unwrap();
    let enrolled = enroller
        .enroll(&EnrollRequest {
            code,
            name: "box".into(),
            worker_version: "0.1.0".into(),
            protocol_version: PROTOCOL_VERSION,
        })
        .await
        .unwrap();
    let saved = WorkerConfig {
        server_url: s1.worker.clone(),
        node_id: enrolled.node_id,
        credential: enrolled.credential.clone(),
        cf_client_id: None,
        cf_client_secret: None,
    };
    Client::from_config(&saved)
        .unwrap()
        .heartbeat(&hb())
        .await
        .unwrap();
    let list = workers(&http, &s1.admin).await;
    assert_eq!(list[0]["id"], enrolled.node_id.to_string());

    // Restart: same database, new store, new ports.
    s1.shutdown().await;
    let down = Client::from_config(&saved).unwrap().heartbeat(&hb()).await;
    assert!(matches!(down, Err(CallError::Transient(_))), "{down:?}");

    let s2 = start(db.store().await).await;
    let restarted = WorkerConfig {
        server_url: s2.worker.clone(),
        ..saved
    };
    let client = Client::from_config(&restarted).unwrap();
    client.heartbeat(&hb()).await.unwrap();
    let list = workers(&http, &s2.admin).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], enrolled.node_id.to_string());

    // The run loop stops on clean shutdown...
    let (tx, rx) = oneshot::channel::<()>();
    let c2 = Client::from_config(&restarted).unwrap();
    let looped = tokio::spawn(async move {
        run_loop(&c2, async {
            let _ = rx.await;
        })
        .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    tx.send(()).unwrap();
    assert!(looped.await.unwrap().is_ok());

    // ...and revocation is fatal for both a direct call and the loop.
    let r = admin_post(
        &http,
        format!("{}/api/workers/{}/revoke", s2.admin, enrolled.node_id),
    )
    .await;
    assert!(r.status().is_success());
    let e = client.heartbeat(&hb()).await.unwrap_err();
    assert!(matches!(e, CallError::Revoked), "{e:?}");
    let e = run_loop(&client, std::future::pending::<()>())
        .await
        .unwrap_err();
    assert!(matches!(e, CallError::Revoked), "{e:?}");
    s2.shutdown().await;
}
