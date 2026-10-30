//! Server side of updates: the maintenance gate, `/api/updates`, and the
//! release manifest builder. The server never touches Docker and never names
//! an image: it reads the updater's status files and drops small request files
//! into a spool the host updater consumes.

use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use aq_release::{
    ManifestError, ProtocolRange, ReleaseManifest, UpdateRequest, UpdateState, UpdateStatus,
};
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::admin_api::ApiJson;
use crate::authz::{CurrentUser, RequireOwner};
use crate::error::AppError;
use crate::{AppState, READABLE_STATE_FORMATS, STATE_FORMAT_VERSION, UPGRADE_FROM, version_info};

pub fn spool_dir(state: &AppState) -> Option<&Path> {
    state.config.updates.as_ref().map(|u| u.spool_dir.as_path())
}

fn maintenance_marker(spool: &Path) -> PathBuf {
    spool.join("status").join("maintenance")
}

fn under_maintenance(state: &AppState) -> bool {
    spool_dir(state).is_some_and(|s| maintenance_marker(s).exists())
}

fn maintenance_error() -> Response {
    AppError::Maintenance("Agent Q is being updated; try again in a few minutes".to_string())
        .into_response()
}

/// Admin listener: every non-GET/HEAD request is refused while the marker exists.
pub async fn admin_maintenance_gate(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let read_only = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !read_only && under_maintenance(&state) {
        return maintenance_error();
    }
    next.run(req).await
}

/// Worker listener: only enrollment is gated. Heartbeat and probe keep
/// working so workers keep their identity through an update.
pub async fn worker_maintenance_gate(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    if req.uri().path() == "/v1/worker/enroll" && under_maintenance(&state) {
        return maintenance_error();
    }
    next.run(req).await
}

fn read_status(spool: &Path) -> Result<UpdateStatus, String> {
    let path = spool.join("status").join("status.json");
    let bytes = std::fs::read(&path).map_err(|e| format!("cannot read update status: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("update status is unreadable: {e}"))
}

/// `(state, active_version, available_version)` for `/api/status`.
pub fn status_summary(state: &AppState) -> Value {
    let Some(spool) = spool_dir(state) else {
        return json!({"state": "unavailable", "active_version": null, "available_version": null});
    };
    match read_status(spool) {
        Ok(s) => {
            let label = match s.state {
                UpdateState::Idle => "idle",
                UpdateState::InProgress { .. } => "in_progress",
                UpdateState::FailedNeedsOperator { .. } => "failed_needs_operator",
                UpdateState::Unknown => "unknown",
            };
            json!({
                "state": label,
                "active_version": s.active.map(|r| r.version.to_string()),
                "available_version": s.available.map(|r| r.version.to_string()),
            })
        }
        Err(_) => {
            json!({"state": "unavailable", "active_version": null, "available_version": null})
        }
    }
}

pub fn update_routes() -> Router<AppState> {
    Router::new().route("/api/updates", get(get_updates).post(request_update))
}

async fn get_updates(State(state): State<AppState>, _user: CurrentUser) -> Json<Value> {
    let unavailable = |detail: String| Json(json!({"state": "unavailable", "detail": detail}));
    let Some(spool) = spool_dir(&state) else {
        return unavailable("updates are not configured for this installation".into());
    };
    match read_status(spool) {
        Ok(s) => Json(serde_json::to_value(s).unwrap_or(Value::Null)),
        Err(detail) => unavailable(detail),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateBody {
    version: Option<String>,
}

#[derive(Serialize)]
struct Accepted {
    request_id: Uuid,
}

fn has_queued_request(requests: &Path) -> std::io::Result<bool> {
    for entry in std::fs::read_dir(requests)? {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with('.') && name.ends_with(".json") {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn request_update(
    State(state): State<AppState>,
    RequireOwner(user): RequireOwner,
    ApiJson(body): ApiJson<UpdateBody>,
) -> Result<(StatusCode, Json<Accepted>), AppError> {
    let spool = spool_dir(&state).ok_or_else(|| {
        AppError::UpdatesUnavailable("updates are not configured for this installation".into())
    })?;
    let version = body
        .version
        .map(|v| {
            Version::parse(&v)
                .map_err(|_| AppError::InvalidVersion(format!("{v:?} is not a version like 1.2.3")))
        })
        .transpose()?;
    let in_progress = || AppError::UpdateInProgress("an update is already in progress".into());
    // A missing or unreadable status does not block: the updater validates
    // and records the request itself.
    if let Ok(s) = read_status(spool)
        && matches!(s.state, UpdateState::InProgress { .. })
    {
        return Err(in_progress());
    }
    let requests = spool.join("requests");
    match has_queued_request(&requests) {
        Ok(true) => return Err(in_progress()),
        Ok(false) => {}
        Err(e) => {
            tracing::error!(error = %e, "cannot read update spool");
            return Err(AppError::UpdatesUnavailable(
                "the update spool is not available".into(),
            ));
        }
    }
    let request = UpdateRequest {
        id: Uuid::new_v4(),
        version,
        requested_by: user.email.clone(),
        requested_at: (state.clock)(),
    };
    write_request(&requests, &request).map_err(|e| {
        tracing::error!(error = %e, "cannot write update request");
        AppError::Internal
    })?;
    tracing::info!(requester = %user.id, request_id = %request.id, "update requested");
    Ok((
        StatusCode::ACCEPTED,
        Json(Accepted {
            request_id: request.id,
        }),
    ))
}

/// Temp file (dotfile, ignored by the updater), mode 0640, then rename.
fn write_request(requests: &Path, request: &UpdateRequest) -> std::io::Result<()> {
    let tmp = requests.join(format!(".tmp-{}", request.id));
    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o640)
            .open(&tmp)?;
        // The umask may have narrowed the mode; set it explicitly.
        f.set_permissions(std::fs::Permissions::from_mode(0o640))?;
        f.write_all(&serde_json::to_vec(request).map_err(std::io::Error::other)?)?;
        f.sync_all()?;
        std::fs::rename(&tmp, requests.join(format!("{}.json", request.id)))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[derive(Debug, thiserror::Error)]
pub enum BuildManifestError {
    #[error(
        "--version {requested} does not match this binary's version {actual}; \
         bump the workspace version or pass the matching tag"
    )]
    VersionMismatch { requested: String, actual: String },
    #[error("--version {0:?} is not a valid semantic version")]
    BadVersion(String),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
}

/// Builds the manifest describing exactly this binary. Validated by a
/// round trip through `ReleaseManifest::from_json`, so bad image refs fail here.
pub fn build_manifest(
    server_image: &str,
    worker_image: &str,
    version: &str,
    source_revision: &str,
) -> Result<ReleaseManifest, BuildManifestError> {
    let actual = env!("CARGO_PKG_VERSION");
    let parsed =
        Version::parse(version).map_err(|_| BuildManifestError::BadVersion(version.into()))?;
    if parsed.to_string() != actual {
        return Err(BuildManifestError::VersionMismatch {
            requested: version.into(),
            actual: actual.into(),
        });
    }
    let info = version_info();
    let manifest = ReleaseManifest {
        manifest_version: 1,
        version: parsed,
        source_revision: source_revision.into(),
        images: aq_release::ImageRefs {
            server: server_image.into(),
            worker: worker_image.into(),
        },
        protocol: ProtocolRange {
            min: info.protocol_min,
            max: info.protocol_max,
        },
        state_format: STATE_FORMAT_VERSION,
        readable_state_formats: READABLE_STATE_FORMATS.to_vec(),
        upgrade_from: UPGRADE_FROM.parse().expect("UPGRADE_FROM is a valid range"),
        created_at: Utc::now(),
    };
    ReleaseManifest::from_json(&manifest.to_canonical_json())?;
    Ok(manifest)
}
