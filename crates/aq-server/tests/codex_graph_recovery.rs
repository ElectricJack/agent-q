//! Pure and synthetic-HTTP transport checks only. This does NOT run Graph,
//! Codex, tmux, Git, AQ settlement, SQL authority or a real mutation.
mod common;
#[allow(dead_code)]
#[path = "support/real_graph.rs"]
mod real_graph;
#[allow(dead_code)]
#[path = "support/codex_graph_recovery.rs"]
mod recovery;

use aq_server::agent_access::AgentAccessConfig;
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Request, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use std::{os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, oneshot},
    task::JoinHandle,
};
use uuid::Uuid;

struct SyntheticHttp {
    pin: real_graph::GraphTaskPin,
    posts: Mutex<Vec<Vec<u8>>>,
    receipt: Value,
}
async fn synthetic_http(State(state): State<Arc<SyntheticHttp>>, request: Request) -> Response {
    assert_eq!(request.headers()["X-AQ-Graph-API-Version"], "1");
    assert_eq!(
        request.headers()["authorization"],
        format!("Bearer {}", "t".repeat(40))
    );
    let workspace = state.pin.workspace_id;
    let history = format!(
        "/v1/workspaces/{workspace}/nodes/{}/history",
        state.pin.task_id
    );
    let mutation = format!("/v1/workspaces/{workspace}/mutations");
    let value = if request.method() == Method::GET && request.uri().path() == history {
        // Merely a closed transport fixture response, never an actual Graph fact.
        json!({"items":[],"next_cursor":null})
    } else if request.method() == Method::POST && request.uri().path() == mutation {
        let bytes = axum::body::to_bytes(request.into_body(), 8192)
            .await
            .unwrap();
        state.posts.lock().await.push(bytes.to_vec());
        state.receipt.clone()
    } else {
        return StatusCode::NOT_FOUND.into_response();
    };
    ([("X-AQ-Graph-API-Version", "1")], Json(value)).into_response()
}
struct HttpOwner {
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<std::io::Result<()>>>,
}
impl HttpOwner {
    async fn close(mut self) {
        let _ = self.stop.take().unwrap().send(());
        let mut task = self.task.take().unwrap();
        match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
            Ok(result) => result.unwrap().unwrap(),
            Err(_) => {
                task.abort();
                let _ = task.await;
                panic!("synthetic HTTP listener failed to drain");
            }
        }
    }
}
impl Drop for HttpOwner {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[tokio::test]
async fn synthetic_http_gate_loses_first_body_without_automatic_client_or_upstream_retry() {
    let pin = real_graph::GraphTaskPin {
        workspace_id: Uuid::from_u128(11),
        task_id: Uuid::from_u128(12),
        version: 1,
        content_revision: 1,
    };
    let receipt = json!({"change_set_id":Uuid::from_u128(13),"results":[{"id":pin.task_id,"workspace_id":pin.workspace_id,"kind":"task","task_state":"completed","deleted":false,"retired":false,"version":2,"content_revision":2}]});
    let state = Arc::new(SyntheticHttp {
        pin: pin.clone(),
        posts: Mutex::new(vec![]),
        receipt: receipt.clone(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = oneshot::channel();
    let app = Router::new()
        .fallback(synthetic_http)
        .with_state(state.clone());
    let owner = HttpOwner {
        stop: Some(stop),
        task: Some(tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
        })),
    };
    let dir = tempfile::tempdir().unwrap();
    let token = dir.path().join("synthetic-http-token");
    std::fs::write(&token, "t".repeat(40)).unwrap();
    std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config = AgentAccessConfig {
        graph_url: origin.clone(),
        graph_token_file: token.clone(),
        mutation_actor_id: Some("synthetic-http-transport-only".into()),
        credential_ttl_ms: 10000,
    };
    let gate = recovery::GraphRecoveryGate::start(&config, &pin)
        .await
        .unwrap();
    let forwarded = gate.agent_access_config();
    assert_eq!(forwarded.graph_token_file, token);
    assert_eq!(forwarded.mutation_actor_id, config.mutation_actor_id);
    assert_eq!(forwarded.credential_ttl_ms, config.credential_ttl_ms);
    assert_ne!(forwarded.graph_url, origin);
    let mut payload = json!({"idempotency_key":format!("aq-settlement-v1:{}",Uuid::from_u128(14)),"reason":format!("Human-reviewed retained result {}",Uuid::from_u128(15)),"operations":[{"op":"update_node","id":pin.task_id,"expected_version":1,"patch":{"task_state":"completed"}}]});
    payload.sort_all_objects();
    let wire = serde_json::to_vec(&payload).unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let url = format!(
        "{}/v1/workspaces/{}/mutations",
        forwarded.graph_url, pin.workspace_id
    );
    let post = || {
        client
            .post(&url)
            .bearer_auth("t".repeat(40))
            .header("X-AQ-Graph-API-Version", "1")
            .header("content-type", "application/json")
            .body(wire.clone())
    };
    match post().send().await {
        Err(_) => {} // Hyper may close before flushing headers for a body error.
        Ok(response) => {
            assert!(response.status().is_success());
            assert!(
                response.bytes().await.is_err(),
                "first real forwarding body must be lost"
            );
        }
    }
    assert_eq!(
        *state.posts.lock().await,
        vec![wire.clone()],
        "neither forwarding nor caller may retry automatically"
    );
    // Explicit SECOND caller invocation; no AQ owner/history authority claimed.
    let response = post().send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let returned: Value = response.json().await.unwrap();
    assert_eq!(returned, receipt);
    assert_eq!(*state.posts.lock().await, vec![wire.clone(), wire]);
    gate.shutdown().await.unwrap();
    owner.close().await;
}
