//! Human global-conversation admission over AQ-11 mail, never runtime delivery.
mod reads;
pub mod reservations;
mod writes;
use super::{Envelope as Mail, append, digest, receipt, replay, sender_key};
use crate::{AppState, authz::RequireOwner, error::AppError, store::User};
use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest as _;
use sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgPoolOptions};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use uuid::Uuid;
type Result<T> = std::result::Result<T, AppError>;
const CONTRACT: &str = "human-supervisor-conversation-1";
const MAX_OPERATIONS: i64 = 4096;
const MAX_BYTES: i64 = 64 * 1024 * 1024;
const PAGE_BYTES: usize = 262144;
fn invalid() -> AppError {
    AppError::BadRequest("invalid bounded supervisor conversation request".into())
}
fn unavailable() -> AppError {
    AppError::IdentityUnavailable("supervisor conversation unavailable".into())
}
fn conflict() -> AppError {
    AppError::Conflict("operation identity conflicts; inspect original operation".into())
}
fn missing() -> AppError {
    AppError::NotFound("conversation or operation not found".into())
}
fn db(_: sqlx::Error) -> AppError {
    unavailable()
}
fn hash(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)))
}
fn positive(s: &str) -> Result<i64> {
    if s.starts_with('0') || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    s.parse::<i64>().ok().filter(|n| *n > 0).ok_or_else(invalid)
}
fn cursor(s: &str) -> Result<i64> {
    if s == "0" { Ok(0) } else { positive(s) }
}
fn stamp(state: &AppState) -> Result<u64> {
    u64::try_from((state.clock)().timestamp_millis())
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(unavailable)
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Conversation {
    id: Uuid,
    revision: String,
    label: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Runtime {
    state: String,
    binding: Option<Value>,
    reason: String,
}
fn runtime() -> Runtime {
    Runtime {
        state: "unavailable".into(),
        binding: None,
        reason: "central_runtime_not_connected".into(),
    }
}
struct Service {
    pool: PgPool,
    gate: Semaphore,
}
impl Service {
    fn new(state: &AppState) -> Self {
        Self {
            pool: PgPoolOptions::new()
                .max_connections(2)
                .acquire_timeout(Duration::from_millis(500))
                .connect_lazy_with(
                    state
                        .store
                        .pool
                        .connect_options()
                        .as_ref()
                        .clone()
                        .application_name("aq-supervisor-conversation"),
                ),
            gate: Semaphore::new(2),
        }
    }
    async fn tx(&self) -> Result<Transaction<'_, Postgres>> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("SELECT set_config('statement_timeout','1000ms',true),set_config('lock_timeout','500ms',true)").execute(&mut *tx).await.map_err(db)?;
        Ok(tx)
    }
}
pub fn routes(state: &AppState) -> Router<AppState> {
    let service = Arc::new(Service::new(state));
    Router::new()
        .route("/api/human/conversations/global", get(reads::discovery))
        .route(
            "/api/human/conversations/{conversation}/messages",
            get(reads::history).post(writes::enqueue),
        )
        .route(
            "/api/human/conversations/{conversation}/operations/{operation}",
            get(writes::lookup),
        )
        .layer(DefaultBodyLimit::max(131072))
        .layer(Extension(service.clone()))
        .layer(middleware::from_fn_with_state(service, gate))
}
async fn gate(State(service): State<Arc<Service>>, request: Request, next: Next) -> Response {
    let Ok(_permit) = service.gate.try_acquire() else {
        return unavailable().into_response();
    };
    let mut response = match tokio::time::timeout(Duration::from_secs(4), next.run(request)).await {
        Ok(r) => r,
        Err(_) => unavailable().into_response(),
    };
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
}
async fn owner(tx: &mut Transaction<'_, Postgres>, user: &User) -> Result<()> {
    let row = sqlx::query("SELECT role,email,issuer,subject FROM users WHERE id=$1 FOR SHARE")
        .bind(user.id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db)?
        .ok_or_else(missing)?;
    if row.get::<String, _>("role") != "owner"
        || row.get::<String, _>("email") != user.email
        || row.get::<Option<String>, _>("issuer") != user.issuer
        || row.get::<Option<String>, _>("subject") != user.subject
    {
        return Err(AppError::Forbidden(
            "current owner identity required".into(),
        ));
    }
    Ok(())
}
async fn definition(
    tx: &mut Transaction<'_, Postgres>,
    id: Option<Uuid>,
    write: bool,
) -> Result<Conversation> {
    let sql = if write {
        "SELECT conversation_id,revision FROM global_supervisor_conversation WHERE singleton FOR UPDATE"
    } else {
        "SELECT conversation_id,revision FROM global_supervisor_conversation WHERE singleton FOR SHARE"
    };
    let row = sqlx::query(sql)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db)?
        .ok_or_else(unavailable)?;
    let actual: Uuid = row.get("conversation_id");
    let revision: i64 = row.get("revision");
    if actual.is_nil() || revision != 1 {
        return Err(unavailable());
    }
    if id.is_some_and(|i| i != actual) {
        return Err(missing());
    }
    Ok(Conversation {
        id: actual,
        revision: revision.to_string(),
        label: "Global supervisor".into(),
    })
}
fn recipient(conversation: Uuid) -> Value {
    json!({"kind":"global_supervisor","conversation_id":conversation})
}
fn key(conversation: Uuid) -> Result<String> {
    digest(&recipient(conversation)).map_err(|_| unavailable())
}
fn author(id: Uuid) -> Value {
    json!({"kind":"owner","user_id":id})
}
