//! Machine routes: enrollment, heartbeat and probe. Bearer auth only; no
//! cookies, so no CSRF guard. Mounted on the worker listener only.

use aq_protocol::{
    CompatibilityError, EnrollRequest, EnrollResponse, HeartbeatRequest, HeartbeatResponse,
    ProbeRequest, ProbeResponse, SUPPORTED_WORKER_PROTOCOLS, UNSUPPORTED_PROTOCOL,
};
use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, header};
use axum::routing::post;

use crate::admin_api::ApiJson;
use crate::error::AppError;
use crate::store::{NewNode, Node};
use crate::tokens::{new_node_credential, verifier};
use crate::{AppState, version_info};

const MAX_BODY_BYTES: usize = 16 * 1024;

pub fn worker_routes() -> Router<AppState> {
    Router::new()
        .route("/v1/worker/enroll", post(enroll))
        .route("/v1/worker/heartbeat", post(heartbeat))
        .route("/v1/worker/probe", post(probe))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

fn check_protocol(version: u32) -> Result<(), AppError> {
    let (min, max) = SUPPORTED_WORKER_PROTOCOLS;
    if (min..=max).contains(&version) {
        return Ok(());
    }
    Err(AppError::UnsupportedProtocol(CompatibilityError {
        error: UNSUPPORTED_PROTOCOL.to_string(),
        message: format!(
            "worker protocol {version} is not supported; this server accepts {min}\u{2013}{max}. \
             Install a compatible aq-worker release."
        ),
        supported_min: min,
        supported_max: max,
    }))
}

fn bad(msg: &str) -> AppError {
    AppError::BadRequest(msg.to_string())
}

/// Resolves the bearer credential to a live (not revoked) node.
async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Node, AppError> {
    let invalid = || AppError::InvalidCredential("missing or invalid worker credential".into());
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|t| t.starts_with("aqn_"))
        .ok_or_else(invalid)?;
    let node = state
        .store
        .find_node_by_credential(&verifier(token))
        .await?
        .ok_or_else(invalid)?;
    if node.revoked_at.is_some() {
        return Err(AppError::CredentialRevoked);
    }
    Ok(node)
}

async fn enroll(
    State(state): State<AppState>,
    ApiJson(req): ApiJson<EnrollRequest>,
) -> Result<Json<EnrollResponse>, AppError> {
    check_protocol(req.protocol_version)?;
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(bad("name must be 1-64 characters"));
    }
    if req.worker_version.is_empty() || req.worker_version.chars().count() > 64 {
        return Err(bad("worker_version must be 1-64 characters"));
    }
    let credential = new_node_credential();
    let node = state
        .store
        .redeem_enrollment(
            &verifier(&req.code),
            NewNode {
                name: name.to_string(),
                credential_hash: verifier(&credential),
                worker_version: req.worker_version,
                protocol_version: req.protocol_version,
            },
            (state.clock)(),
        )
        .await?
        .ok_or(AppError::EnrollmentInvalid)?;
    tracing::info!(node_id = %node.id, "worker enrolled");
    Ok(Json(EnrollResponse {
        node_id: node.id,
        credential,
        heartbeat_interval_secs: state.config.workers.heartbeat_interval_secs,
    }))
}

async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(req): ApiJson<HeartbeatRequest>,
) -> Result<Json<HeartbeatResponse>, AppError> {
    check_protocol(req.protocol_version)?;
    if req.worker_version.is_empty() || req.worker_version.chars().count() > 64 {
        return Err(bad("worker_version must be 1-64 characters"));
    }
    // Revocation is checked inside `authenticate`, before anything is written.
    let node = authenticate(&state, &headers).await?;
    state
        .store
        .record_heartbeat(
            node.id,
            &req.worker_version,
            req.protocol_version,
            (state.clock)(),
        )
        .await?;
    Ok(Json(HeartbeatResponse {
        server_version: version_info().server_version,
        heartbeat_interval_secs: state.config.workers.heartbeat_interval_secs,
    }))
}

async fn probe(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(req): ApiJson<ProbeRequest>,
) -> Result<Json<ProbeResponse>, AppError> {
    authenticate(&state, &headers).await?;
    if req.nonce.chars().count() > 64 {
        return Err(bad("nonce must be at most 64 characters"));
    }
    Ok(Json(ProbeResponse {
        nonce: req.nonce,
        server_time: (state.clock)(),
    }))
}
