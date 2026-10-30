pub mod config;
pub mod identity;
pub mod store;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod tokens;
use aq_protocol::{SUPPORTED_WORKER_PROTOCOLS, VersionInfo};
use axum::{Json, Router, routing::get};
use serde_json::{Value, json};

pub const STATE_FORMAT_VERSION: u32 = 1;

pub fn version_info() -> VersionInfo {
    VersionInfo {
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        source_revision: option_env!("AQ_SOURCE_REVISION")
            .unwrap_or("dev")
            .to_string(),
        protocol_min: SUPPORTED_WORKER_PROTOCOLS.0,
        protocol_max: SUPPORTED_WORKER_PROTOCOLS.1,
        state_format_version: STATE_FORMAT_VERSION,
    }
}

/// Temporary health router; folded into both listeners later.
pub fn health_router() -> Router {
    Router::new().route("/healthz", get(healthz))
}

async fn healthz() -> Json<Value> {
    Json(json!({"status": "ok"}))
}
