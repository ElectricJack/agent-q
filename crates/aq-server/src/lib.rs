pub mod authz;
pub mod config;
pub mod error;
pub mod identity;
pub mod store;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod tokens;
use crate::config::Config;
use crate::identity::IdentityVerifier;
use crate::store::Store;
use aq_protocol::{SUPPORTED_WORKER_PROTOCOLS, VersionInfo};
use axum::{Json, Router, routing::get};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::sync::Arc;

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

/// Source of "now". Every expiry and offline computation reads this so tests
/// can use a fixed clock.
pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(Utc::now)
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub store: Store,
    pub identity: Arc<dyn IdentityVerifier>,
    pub clock: Clock,
}
