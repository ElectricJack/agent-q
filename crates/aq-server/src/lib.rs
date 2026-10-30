pub mod admin_api;
pub mod authz;
pub mod config;
pub mod error;
pub mod identity;
pub mod store;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod tokens;
pub mod worker_api;
use crate::config::Config;
use crate::identity::IdentityVerifier;
use crate::store::Store;
use aq_protocol::{SUPPORTED_WORKER_PROTOCOLS, VersionInfo};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::http::{HeaderValue, header};
use axum::middleware::{Next, from_fn, from_fn_with_state};
use axum::response::IntoResponse;
use axum::response::Response;
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

async fn harden(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

async fn not_found() -> Response {
    error::AppError::NotFound("no such route".into()).into_response()
}

async fn method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(aq_protocol::ApiError {
            error: "method_not_allowed".into(),
            message: "method not allowed for this route".into(),
        }),
    )
        .into_response()
}

/// Human routes plus `/healthz`. Worker routes are not mounted here.
///
/// `csrf_guard` is layered over the whole merged router (fallbacks included),
/// so it covers every admin route regardless of declaration order. A POST to
/// an unmatched path therefore gets 403 `csrf_rejected` unless it carries
/// the CSRF header.
pub fn build_admin_router(state: AppState) -> Router {
    Router::new()
        .merge(admin_api::admin_routes())
        .route("/healthz", get(healthz))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(from_fn_with_state(state.clone(), authz::csrf_guard))
        .layer(from_fn(harden))
        .with_state(state)
}

/// Machine routes plus `/healthz`. No human routes are mounted here.
pub fn build_worker_router(state: AppState) -> Router {
    Router::new()
        .merge(worker_api::worker_routes())
        .route("/healthz", get(healthz))
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        .layer(from_fn(harden))
        .with_state(state)
}

/// Serves both routers until `shutdown` resolves, then drains gracefully.
pub async fn run_with_listeners(
    state: AppState,
    admin: tokio::net::TcpListener,
    worker: tokio::net::TcpListener,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> anyhow::Result<()> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        shutdown.await;
        let _ = tx.send(true);
    });
    let wait = |mut rx: tokio::sync::watch::Receiver<bool>| async move {
        let _ = rx.wait_for(|stop| *stop).await;
    };
    let admin_app = build_admin_router(state.clone());
    let worker_app = build_worker_router(state);
    tokio::try_join!(
        async {
            axum::serve(admin, admin_app)
                .with_graceful_shutdown(wait(rx.clone()))
                .await
        },
        async {
            axum::serve(worker, worker_app)
                .with_graceful_shutdown(wait(rx.clone()))
                .await
        },
    )?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutdown signal received");
}

/// Binds both listeners and serves until SIGTERM or ctrl-c.
pub async fn run(config: Config, store: Store) -> anyhow::Result<()> {
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let admin = tokio::net::TcpListener::bind(config.server.admin_bind).await?;
    let worker = tokio::net::TcpListener::bind(config.server.worker_bind).await?;
    tracing::info!(admin = %admin.local_addr()?, worker = %worker.local_addr()?, "listening");
    let state = AppState {
        identity: identity::verifier_from_config(&config.auth, http),
        config: Arc::new(config),
        store,
        clock: system_clock(),
    };
    run_with_listeners(state, admin, worker, shutdown_signal()).await
}
