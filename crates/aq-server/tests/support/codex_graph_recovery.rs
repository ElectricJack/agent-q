//! Test-only response-loss gate over the existing actual pinned Graph owner.
//! Install its URL BEFORE ActualSetup; retain the original actor/token/TTL.
//! It never retries, mutates SQL, creates a Graph receipt, or reconciles AQ.
//! The caller must separately assert first AQ `possible` + holds_task, then call
//! the SAME completion operation again and assert confirmed + released claim.
use super::real_graph::GraphTaskPin;
use anyhow::{Context, Result, ensure};
use aq_server::agent_access::AgentAccessConfig;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::{
    fs::OpenOptions,
    io::Read,
    net::{IpAddr, Ipv4Addr},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    pin::Pin,
    sync::Arc,
    task::{Context as TaskContext, Poll},
    time::{Duration, Instant},
};
use tokio::{
    sync::{Mutex, Semaphore, oneshot},
    task::JoinHandle,
};
use uuid::Uuid;

const MAX_BODY: usize = 65_536;
const MAX_MUTATION: usize = 8192;
const MAX_REQUESTS: usize = 256;
const IO_TIMEOUT: Duration = Duration::from_secs(2);
const GATE_LIFETIME: Duration = Duration::from_secs(180);
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn canonical(value: &Value) -> Result<Vec<u8>> {
    let mut value = value.clone();
    value.sort_all_objects();
    Ok(serde_json::to_vec(&value)?)
}
fn origin(text: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(text).context("Graph gate origin")?;
    ensure!(
        url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port().is_some_and(|p| p > 0)
            && url.username().is_empty()
            && url.password().is_none()
            && url.path() == "/"
            && url.query().is_none()
            && url.fragment().is_none(),
        "Graph gate requires numeric loopback origin"
    );
    let expected = format!("http://127.0.0.1:{}", url.port().unwrap());
    ensure!(
        text == expected || text == format!("{expected}/"),
        "Graph gate requires explicit numeric origin spelling"
    );
    Ok(url)
}
fn credential(config: &AgentAccessConfig) -> Result<HeaderValue> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(&config.graph_token_file)
        .context("Graph gate credential open")?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.mode() & 0o077 == 0 && meta.len() <= 4096,
        "Graph gate private bounded credential"
    );
    let mut text = String::new();
    file.take(4097).read_to_string(&mut text)?;
    let token = text.trim();
    ensure!(
        (32..=4096).contains(&token.len()) && !token.chars().any(char::is_whitespace),
        "Graph gate credential format"
    );
    let mut header = HeaderValue::from_str(&format!("Bearer {token}"))
        .context("Graph gate credential header")?;
    header.set_sensitive(true);
    Ok(header)
}
fn completion(bytes: &[u8], pin: &GraphTaskPin) -> Result<Value> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_MUTATION,
        "Graph gate bounded completion"
    );
    let value: Value = serde_json::from_slice(bytes).context("Graph gate completion JSON")?;
    ensure!(
        canonical(&value)? == bytes,
        "Graph gate requires exact canonical completion bytes"
    );
    let obj = value.as_object().context("Graph gate completion object")?;
    ensure!(
        obj.len() == 3
            && obj.contains_key("reason")
            && obj.contains_key("operations")
            && obj.contains_key("idempotency_key"),
        "Graph gate closed completion fields"
    );
    let key = value["idempotency_key"]
        .as_str()
        .context("Graph gate completion key")?;
    let op = key
        .strip_prefix("aq-settlement-v1:")
        .and_then(|v| v.parse::<Uuid>().ok())
        .filter(|v| !v.is_nil())
        .context("Graph gate completion identity")?;
    ensure!(
        key == format!("aq-settlement-v1:{op}"),
        "Graph gate canonical completion identity"
    );
    let reason = value["reason"]
        .as_str()
        .context("Graph gate completion reason")?;
    let report = reason
        .strip_prefix("Human-reviewed retained result ")
        .and_then(|v| v.parse::<Uuid>().ok())
        .filter(|v| !v.is_nil())
        .context("Graph gate original report reason")?;
    ensure!(
        reason == format!("Human-reviewed retained result {report}"),
        "Graph gate exact completion reason"
    );
    ensure!(
        value["operations"]
            == json!([{"op":"update_node","id":pin.task_id,"expected_version":pin.version,"patch":{"task_state":"completed"}}]),
        "Graph gate refuses nonoriginal task CAS"
    );
    Ok(value)
}
#[derive(Clone)]
struct Dispatch {
    wire: Vec<u8>,
    payload: Value,
    response: Value,
    lost: bool,
}
#[derive(Default)]
struct Audit {
    requests: usize,
    pending: bool,
    dispatches: Vec<Dispatch>,
    failed: bool,
}
struct Shared {
    client: reqwest::Client,
    upstream: reqwest::Url,
    auth: HeaderValue,
    pin: GraphTaskPin,
    deadline: Instant,
    lane: Semaphore,
    audit: Mutex<Audit>,
}
impl Shared {
    fn mutation_path(&self) -> String {
        format!("/v1/workspaces/{}/mutations", self.pin.workspace_id)
    }
    fn history_path(&self) -> String {
        format!(
            "/v1/workspaces/{}/nodes/{}/history?limit=64",
            self.pin.workspace_id, self.pin.task_id
        )
    }
    async fn read(&self, path: &str) -> Result<Value> {
        ensure!(
            Instant::now() < self.deadline,
            "Graph gate lifetime expired"
        );
        let response = self
            .client
            .get(self.upstream.join(path)?)
            .header(header::AUTHORIZATION, self.auth.clone())
            .header("X-AQ-Graph-API-Version", "1")
            .send()
            .await
            .context("Graph gate actual read unavailable")?;
        let (status, _, bytes) = bounded(response).await?;
        ensure!(status == StatusCode::OK, "Graph gate actual read refused");
        Ok(serde_json::from_slice(&bytes).context("Graph gate actual read JSON")?)
    }
    async fn forward(&self, request: Request) -> Result<Response> {
        let _permit = self
            .lane
            .try_acquire()
            .context("Graph gate concurrent request bound")?;
        ensure!(
            Instant::now() < self.deadline,
            "Graph gate lifetime expired"
        );
        let method = request.method().clone();
        let path = request
            .uri()
            .path_and_query()
            .context("Graph gate fixed route")?
            .as_str()
            .to_owned();
        ensure!(
            path.starts_with(&format!("/v1/workspaces/{}/", self.pin.workspace_id))
                && !path.contains(['\\', '#'])
                && !path.starts_with("//"),
            "Graph gate workspace route"
        );
        ensure!(
            request.headers().get(header::AUTHORIZATION) == Some(&self.auth)
                && request
                    .headers()
                    .get("X-AQ-Graph-API-Version")
                    .is_some_and(|v| v == "1"),
            "Graph gate exact original service credential"
        );
        let bytes = axum::body::to_bytes(request.into_body(), MAX_BODY)
            .await
            .context("Graph gate bounded request")?;
        let payload = if method == Method::POST {
            ensure!(
                path == self.mutation_path(),
                "Graph gate only original completion mutation"
            );
            Some(completion(&bytes, &self.pin)?)
        } else {
            ensure!(
                method == Method::GET && bytes.is_empty(),
                "Graph gate only read or exact completion"
            );
            None
        };
        {
            let mut audit = self.audit.lock().await;
            ensure!(
                !audit.failed && audit.requests < MAX_REQUESTS,
                "Graph gate request budget/failure"
            );
            audit.requests += 1;
            if let Some(ref payload) = payload {
                ensure!(
                    !audit.pending && audit.dispatches.len() < 2,
                    "Graph gate at most two serial owner dispatches"
                );
                if let Some(first) = audit.dispatches.first() {
                    ensure!(
                        first.wire.as_slice() == bytes.as_ref() && first.payload == *payload,
                        "Graph gate retry changed original bytes"
                    );
                }
                audit.pending = true;
            }
        }
        // Exactly one upstream request. No proxy retry and no synthetic receipt.
        let response = self
            .client
            .request(method, self.upstream.join(&path)?)
            .header(header::AUTHORIZATION, self.auth.clone())
            .header("X-AQ-Graph-API-Version", "1")
            .header(header::CONTENT_TYPE, "application/json")
            .body(bytes.clone())
            .send()
            .await
            .context("Graph gate actual forwarding unavailable")?;
        let (status, headers, body) = bounded(response).await?;
        if let Some(payload) = payload {
            ensure!(status.is_success(), "Graph gate actual completion refused");
            let receipt: Value = serde_json::from_slice(&body)
                .context("Graph gate actual committed receipt JSON")?;
            verify_receipt(&receipt, &self.pin)?;
            let mut audit = self.audit.lock().await;
            let lost = audit.dispatches.is_empty();
            audit.dispatches.push(Dispatch {
                wire: bytes.to_vec(),
                payload,
                response: receipt,
                lost,
            });
            audit.pending = false;
            if lost {
                // Full successful real response is already retained above. An
                // HTTP body transport error withholds it; no Graph error/receipt
                // is manufactured and Graph is never called again by this gate.
                return Ok(response_with(status, headers, Body::new(LostReply)));
            }
        }
        Ok(response_with(status, headers, Body::from(body)))
    }
}
fn verify_receipt(receipt: &Value, pin: &GraphTaskPin) -> Result<()> {
    let nodes = receipt["results"]
        .as_array()
        .context("Graph gate actual receipt results")?;
    ensure!(
        nodes.len() == 1
            && receipt["change_set_id"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .is_some_and(|v| !v.is_nil()),
        "Graph gate real mutation receipt identity"
    );
    verify_task(&nodes[0], pin)
}
fn verify_task(node: &Value, pin: &GraphTaskPin) -> Result<()> {
    ensure!(
        node["id"] == json!(pin.task_id)
            && node["workspace_id"] == json!(pin.workspace_id)
            && node["kind"] == "task"
            && node["task_state"] == "completed"
            && node["deleted"] == false
            && node["retired"] == false
            && node["version"]
                == json!(
                    pin.version
                        .checked_add(1)
                        .context("Graph gate version overflow")?
                )
            && node["content_revision"]
                == json!(
                    pin.content_revision
                        .checked_add(1)
                        .context("Graph gate revision overflow")?
                ),
        "Graph gate original CAS receipt differs"
    );
    Ok(())
}
async fn bounded(mut response: reqwest::Response) -> Result<(StatusCode, HeaderMap, Vec<u8>)> {
    let status = response.status();
    let mut headers = HeaderMap::new();
    let version = response
        .headers()
        .get("X-AQ-Graph-API-Version")
        .filter(|v| *v == "1")
        .context("Graph gate real version header")?
        .clone();
    headers.insert("X-AQ-Graph-API-Version", version);
    if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
        headers.insert(header::CONTENT_TYPE, content_type.clone());
    }
    ensure!(
        response
            .content_length()
            .is_none_or(|n| n <= MAX_BODY as u64),
        "Graph gate bounded response"
    );
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .context("Graph gate real response body")?
    {
        ensure!(
            bytes.len() + chunk.len() <= MAX_BODY,
            "Graph gate bounded response"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, headers, bytes))
}
fn response_with(status: StatusCode, headers: HeaderMap, body: Body) -> Response {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    response
}
struct LostReply;
impl http_body::Body for LostReply {
    type Data = Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut TaskContext<'_>,
    ) -> Poll<Option<std::result::Result<http_body::Frame<Bytes>, Self::Error>>> {
        Poll::Ready(Some(Err(std::io::Error::other(
            "fixture withheld committed Graph response",
        ))))
    }
}
async fn serve(State(shared): State<Arc<Shared>>, request: Request) -> Response {
    match tokio::time::timeout(IO_TIMEOUT, shared.forward(request)).await {
        Ok(Ok(response)) => response,
        Ok(Err(_)) | Err(_) => {
            shared.audit.lock().await.failed = true;
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
/// Owns only the forwarding listener/task. The real Graph process remains owned
/// by RealGraphFixture and must outlive this gate's explicit shutdown.
pub struct GraphRecoveryGate {
    shared: Arc<Shared>,
    config: AgentAccessConfig,
    baseline: Value,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<std::io::Result<()>>>,
}
impl GraphRecoveryGate {
    pub async fn start(original: &AgentAccessConfig, pin: &GraphTaskPin) -> Result<Self> {
        ensure!(
            !pin.workspace_id.is_nil()
                && !pin.task_id.is_nil()
                && pin.version > 0
                && pin.content_revision > 0
                && original
                    .mutation_actor_id
                    .as_ref()
                    .is_some_and(|v| !v.is_empty()),
            "Graph gate actual task and service pins"
        );
        let shared = Arc::new(Shared {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .timeout(IO_TIMEOUT)
                .build()?,
            upstream: origin(&original.graph_url)?,
            auth: credential(original)?,
            pin: pin.clone(),
            deadline: Instant::now() + GATE_LIFETIME,
            lane: Semaphore::new(4),
            audit: Mutex::new(Audit::default()),
        });
        let baseline = shared.read(&shared.history_path()).await?;
        history(&baseline)?;
        let listener = tokio::net::TcpListener::bind((IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).await?;
        let address = listener.local_addr()?;
        let mut config = original.clone();
        config.graph_url = format!("http://{address}");
        let (stop, stopped) = oneshot::channel();
        let app = Router::new().fallback(serve).with_state(shared.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
        });
        Ok(Self {
            shared,
            config,
            baseline,
            stop: Some(stop),
            task: Some(task),
        })
    }
    pub fn agent_access_config(&self) -> AgentAccessConfig {
        self.config.clone()
    }
    /// Call after first actual AQ completion returns possible, before retry.
    /// AQ claim/status assertions remain the actual integration caller's duty.
    pub async fn committed_loss(&self) -> Result<Value> {
        self.verify(1).await
    }
    /// Call only after a second actual AQ complete SAME operation returns confirmed.
    pub async fn verify_recovered(&self) -> Result<Value> {
        self.verify(2).await
    }
    async fn verify(&self, count: usize) -> Result<Value> {
        let audit = self.shared.audit.lock().await;
        ensure!(
            !audit.failed && !audit.pending && audit.dispatches.len() == count,
            "Graph gate exact actual dispatch count"
        );
        let first = &audit.dispatches[0];
        ensure!(first.lost, "Graph gate must lose first real success");
        if count == 2 {
            let second = &audit.dispatches[1];
            ensure!(
                !second.lost
                    && second.wire == first.wire
                    && second.payload == first.payload
                    && second.response == first.response,
                "Graph gate exact recovered request/real receipt"
            );
        }
        let payload = first.payload.clone();
        let wire_sha = sha(&first.wire);
        let receipt = first.response.clone();
        let requests = audit.requests;
        drop(audit);
        let after = self.shared.read(&self.shared.history_path()).await?;
        let before_items = history(&self.baseline)?;
        let after_items = history(&after)?;
        ensure!(
            after_items.len() == before_items.len() + 1
                && before_items.iter().all(|item| after_items.contains(item)),
            "Graph gate real history must gain exactly one preserved change"
        );
        let task = self
            .shared
            .read(&format!(
                "/v1/workspaces/{}/nodes/{}",
                self.shared.pin.workspace_id, self.shared.pin.task_id
            ))
            .await?;
        verify_task(&task, &self.shared.pin)?;
        let final_audit = self.shared.audit.lock().await;
        ensure!(
            !final_audit.failed && !final_audit.pending && final_audit.dispatches.len() == count,
            "Graph gate changed while final real observations were read"
        );
        Ok(
            json!({"profile":"actual_graph_commit_then_http_body_loss_v1","dispatches":count,"gate_requests":requests,"original_task":self.shared.pin,"canonical_payload":payload,"wire_sha256":wire_sha,"upstream_receipt":receipt,"first_successful_response_withheld":true,"history_before":self.baseline,"history_after":after,"actual_task":task,"authority":"observation_only_actual_AQ_owner_required"}),
        )
    }
    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
                Ok(result) => {
                    result.context("Graph gate task join")??;
                }
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    anyhow::bail!(
                        "Graph gate graceful drain exceeded; real Graph outcome may remain unknown"
                    );
                }
            }
        }
        Ok(())
    }
}
fn history(value: &Value) -> Result<&Vec<Value>> {
    ensure!(
        value.get("next_cursor").is_some_and(Value::is_null),
        "Graph gate requires complete bounded history"
    );
    let items = value["items"]
        .as_array()
        .context("Graph gate actual history items")?;
    ensure!(items.len() < 64, "Graph gate history bound");
    Ok(items)
}
impl Drop for GraphRecoveryGate {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pin() -> GraphTaskPin {
        GraphTaskPin {
            workspace_id: Uuid::from_u128(1),
            task_id: Uuid::from_u128(2),
            version: 1,
            content_revision: 1,
        }
    }
    fn payload() -> Value {
        json!({"idempotency_key":format!("aq-settlement-v1:{}",Uuid::from_u128(3)),"reason":format!("Human-reviewed retained result {}",Uuid::from_u128(4)),"operations":[{"op":"update_node","id":pin().task_id,"expected_version":1,"patch":{"task_state":"completed"}}]})
    }
    #[test]
    fn recovery_gate_requires_closed_original_canonical_completion() {
        let value = payload();
        assert_eq!(
            completion(&canonical(&value).unwrap(), &pin()).unwrap(),
            value
        );
        for bad in [
            json!({"extra":true}),
            json!({"idempotency_key":"aq-settlement-v1:00000000-0000-0000-0000-000000000000"}),
        ] {
            let mut changed = value.clone();
            for (key, value) in bad.as_object().unwrap() {
                changed[key] = value.clone();
            }
            assert!(completion(&canonical(&changed).unwrap(), &pin()).is_err());
        }
        let mut stale = value;
        stale["operations"][0]["expected_version"] = json!(2);
        assert!(completion(&canonical(&stale).unwrap(), &pin()).is_err());
    }
    #[test]
    fn recovery_gate_refuses_noncanonical_extra_effects_and_remote_origins() {
        let value = payload();
        assert!(completion(&serde_json::to_vec_pretty(&value).unwrap(), &pin()).is_err());
        let mut extra = value;
        extra["operations"]
            .as_array_mut()
            .unwrap()
            .push(json!({"op":"delete_node","id":pin().task_id}));
        assert!(completion(&canonical(&extra).unwrap(), &pin()).is_err());
        for url in [
            "http://localhost:1234",
            "https://127.0.0.1:1234",
            "http://127.0.0.1:1234/path",
            "http://127.0.0.1:1234/?secret=x",
            "http://user:pass@127.0.0.1:1234",
        ] {
            assert!(origin(url).is_err());
        }
        assert!(origin("http://127.0.0.1:1234").is_ok());
    }
    #[test]
    fn recovery_gate_requires_complete_history_and_actual_expected_receipt() {
        assert!(history(&json!({"items":[],"next_cursor":null})).is_ok());
        assert!(history(&json!({"items":[]})).is_err());
        assert!(history(&json!({"items":[],"next_cursor":"more"})).is_err());
        assert!(
            verify_receipt(
                &json!({"change_set_id":Uuid::new_v4(),"results":[]}),
                &pin()
            )
            .is_err()
        );
    }
}
