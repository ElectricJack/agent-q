//! `/readyz`: loopback-only readiness for the host updater, plus the
//! `readiness` subcommand's client.

use std::net::SocketAddr;

use aq_release::ReadinessReport;
use axum::Json;
use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::config::AuthConfig;
use crate::error::AppError;
use crate::{AppState, version_info};

pub async fn readyz(State(state): State<AppState>, req: Request) -> Response {
    let (mut parts, _) = req.into_parts();
    // Fail closed: no peer address (e.g. a router not served with connect
    // info) is treated as non-loopback.
    // The extractor also honours `MockConnectInfo`, which tests rely on.
    let loopback = ConnectInfo::<SocketAddr>::from_request_parts(&mut parts, &())
        .await
        .is_ok_and(|ConnectInfo(a)| a.ip().to_canonical().is_loopback());
    if !loopback {
        return AppError::NotFound("no such route".into()).into_response();
    }
    let info = version_info();
    let db_ok = state.store.ping(std::time::Duration::from_secs(2)).await;
    let report = ReadinessReport {
        version: info
            .server_version
            .parse()
            .expect("CARGO_PKG_VERSION is semver"),
        source_revision: info.source_revision,
        state_format: info.state_format_version,
        protocol_min: info.protocol_min,
        protocol_max: info.protocol_max,
        database: if db_ok { "ok" } else { "error" }.into(),
        auth_mode: match state.config.auth {
            AuthConfig::Dev { .. } => "dev",
            AuthConfig::CloudflareAccess { .. } => "cloudflare_access",
        }
        .into(),
    };
    let status = if db_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report)).into_response()
}
