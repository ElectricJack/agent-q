//! Real terminal-route observations for the disposable Codex fixture. Never
//! creates registration, ACKs, frames, a Watch receipt, or stopped authority.
#![allow(dead_code)]
use anyhow::{Context, Result, ensure};
use aq_server::AppState;
use aq_session_protocol::Validate;
use aq_terminal_relay::{
    api::{MutationReply, TerminalInfo},
    draft::{Effect, Envelope, PeerUpload, UploadAck},
};
use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;
#[derive(Clone)]
pub struct Exchange {
    pub upload: PeerUpload,
    pub ack: UploadAck,
}
#[derive(Clone, Default)]
pub struct Observed {
    pub uploads: Arc<Mutex<Vec<Exchange>>>,
    pub polls: Arc<Mutex<Vec<Envelope>>>,
    pub statuses: Arc<Mutex<Vec<(String, u16)>>>,
}
impl Observed {
    pub async fn inspect(&self, request: Request, next: Next) -> Response {
        let path = request.uri().path().to_owned();
        let (request, upload) = if path == "/v1/terminal-relay/upload" {
            let (parts, body) = request.into_parts();
            let bytes = to_bytes(body, 262144)
                .await
                .expect("bounded real upload observation");
            let upload: PeerUpload = serde_json::from_slice(&bytes).expect("actual typed upload");
            upload.validate().expect("actual upload shape");
            (Request::from_parts(parts, Body::from(bytes)), Some(upload))
        } else {
            (request, None)
        };
        let response = next.run(request).await;
        {
            let mut statuses = self.statuses.lock().unwrap();
            assert!(statuses.len() < 256);
            statuses.push((path.clone(), response.status().as_u16()));
        }
        if response.status() != StatusCode::OK
            || (upload.is_none()
                && !matches!(
                    path.as_str(),
                    "/v1/terminal-relay/poll" | "/v1/terminal-relay/output-poll"
                ))
        {
            return response;
        }
        let (parts, body) = response.into_parts();
        let bytes = to_bytes(body, 262144)
            .await
            .expect("bounded actual output response");
        if let Some(upload) = upload {
            let ack: UploadAck = serde_json::from_slice(&bytes).expect("actual UploadAck");
            ack.validate().expect("actual ack shape");
            assert_eq!(ack.version, upload.version);
            assert_eq!(ack.capability, upload.capability);
            assert_eq!(ack.peer, upload.peer);
            assert_eq!(ack.exchange_id, upload.exchange_id);
            assert_eq!(ack.cursor, upload.cursor);
            let mut values = self.uploads.lock().unwrap();
            assert!(values.len() < 64);
            values.push(Exchange { upload, ack });
        } else {
            let envelope: Envelope =
                serde_json::from_slice(&bytes).expect("actual node-pulled envelope");
            envelope.validate().expect("actual envelope shape");
            if path == "/v1/terminal-relay/output-poll" {
                assert!(
                    envelope
                        .effects
                        .iter()
                        .all(|e| matches!(e, Effect::Watch(_)))
                );
            }
            let mut values = self.polls.lock().unwrap();
            assert!(values.len() < 64);
            values.push(envelope);
        }
        Response::from_parts(parts, Body::from(bytes))
    }
}
async fn human(state: &AppState, path: &str, body: Option<Value>) -> Result<(HeaderMap, Value)> {
    let mut builder = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header("x-aq-dev-email", "owner@localhost");
    if body.is_some() {
        builder = builder
            .header("content-type", "application/json")
            .header("x-aq-request", "1")
            .header("origin", &state.config.server.public_origin);
    }
    let response = aq_server::build_admin_router(state.clone())
        .oneshot(
            builder.body(Body::from(
                body.map(|v| serde_json::to_vec(&v))
                    .transpose()?
                    .unwrap_or_default(),
            ))?,
        )
        .await?;
    ensure!(
        response.status() == StatusCode::OK,
        "actual protected terminal route refused"
    );
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 524288).await?;
    Ok((headers, serde_json::from_slice(&bytes)?))
}
pub struct Watched {
    pub info: TerminalInfo,
    pub subscription: Uuid,
}
pub async fn watch_registered(state: &AppState, terminal_id: Uuid) -> Result<Watched> {
    let path = format!("/api/v1/terminals/{terminal_id}");
    let (headers, actual) = human(state, &path, None).await?;
    let info: TerminalInfo = serde_json::from_value(actual)?;
    ensure!(
        !info.terminal_policy.input_enabled && info.control.lease.is_none(),
        "output-only terminal required"
    );
    let now: u64 = headers["aq-terminal-server-time-ms"].to_str()?.parse()?;
    let subscription = Uuid::new_v4();
    let mutation = aq_server::terminal::service::Mutation::Watch(aq_session_api::WatchRequest {
        terminal: info.terminal.clone(),
        operation_id: subscription,
        expected_session_revision: info.session_revision,
        expires_at_ms: now.checked_add(15000).context("watch deadline overflow")?,
        after_sequence: None,
    });
    let (_, reply) = human(state, &format!("{path}/mutations"), Some(json!(mutation))).await?;
    let MutationReply::Watch {
        subscription_id, ..
    } = serde_json::from_value(reply)?
    else {
        anyhow::bail!("actual Watch receipt required")
    };
    ensure!(subscription_id == subscription, "Watch operation identity");
    Ok(Watched { info, subscription })
}
pub async fn verify_tail(
    state: &AppState,
    watched: &Watched,
    observed: &Observed,
) -> Result<Value> {
    let exchanges = observed.uploads.lock().unwrap().clone();
    ensure!(!exchanges.is_empty(), "real accepted upload required");
    let frames: Vec<_> = exchanges
        .iter()
        .flat_map(|e| e.upload.frames.iter().cloned())
        .collect();
    ensure!(!frames.is_empty(), "actual output frames required");
    ensure!(
        frames
            .windows(2)
            .all(|pair| pair[1].sequence == pair[0].sequence + 1),
        "actual contiguous output"
    );
    for frame in &frames {
        frame.validate()?;
        ensure!(
            frame.terminal == watched.info.terminal,
            "actual frame terminal"
        );
    }
    let pulled=observed.polls.lock().unwrap().iter().any(|p|p.effects.iter().any(|e|matches!(e,Effect::Watch(w) if w.terminal==watched.info.terminal && w.subscription_id==watched.subscription)));
    ensure!(pulled, "actual node-outbound Watch required");
    ensure!(
        observed
            .statuses
            .lock()
            .unwrap()
            .iter()
            .any(|(path, status)| path == "/v1/terminal-relay/output-poll" && *status == 200),
        "actual output-only node poll required"
    );
    let (_, reply) = human(
        state,
        &format!(
            "/api/v1/terminals/{}/tail?subscription_id={}&after_sequence=0",
            watched.info.terminal.terminal_id, watched.subscription
        ),
        None,
    )
    .await?;
    let tail: aq_server::terminal::tail::TailReply = serde_json::from_value(reply)?;
    ensure!(
        tail.terminal == watched.info.terminal && tail.subscription_id == watched.subscription,
        "authorized tail identity"
    );
    ensure!(
        tail.frames == frames && tail.gaps.is_empty() && !tail.more,
        "byte/digest/sequence-exact accepted tail"
    );
    ensure!(
        tail.availability == aq_server::terminal::tail::Availability::ResidentTail,
        "real resident tail required"
    );
    Ok(
        json!({"terminal":tail.terminal,"subscription":tail.subscription_id,"accepted_uploads":exchanges.len(),"frames":frames.iter().map(|f|json!({"sequence":f.sequence,"bytes":f.byte_length,"digest":f.digest})).collect::<Vec<_>>(),"total_bytes":frames.iter().map(|f|f.bytes.len()).sum::<usize>(),"tail_next_sequence":tail.next_sequence,"tail_disconnected":tail.disconnected}),
    )
}
