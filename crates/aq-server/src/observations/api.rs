//! Explicitly constructed route pending ordered migration/runtime registration.
//! Worker bearer credentials only; no source/policy registration on this surface.
use super::*;
use crate::{admin_api::ApiJson, authz::CurrentUser, error::AppError};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, header},
    routing::{get, post},
};

pub fn routes(mut state: AppState) -> Router<AppState> {
    state.store = state.store.observation_view();
    Router::new()
        .route("/v1/worker/observations/batch", post(batch))
        .layer(DefaultBodyLimit::max(MAX_BATCH_BYTES))
        .with_state(state)
}
pub fn admin_routes(mut state: AppState) -> Router<AppState> {
    state.store = state.store.observation_view();
    Router::new()
        .route("/api/observations/sessions/{attempt}/events", get(events))
        .route("/api/observations/sessions/{attempt}/usage", get(usage))
        .with_state(state)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    #[serde(default)]
    after_sequence: u64,
    through_sequence: Option<u64>,
    #[serde(default = "page_limit")]
    limit: u32,
}
fn page_limit() -> u32 {
    16
}
async fn events(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(attempt): Path<Uuid>,
    Query(page): Query<Page>,
) -> std::result::Result<
    (
        [(header::HeaderName, &'static str); 1],
        Json<inspection::EventsPage>,
    ),
    AppError,
> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(
            inspection::events(
                &state,
                user.id,
                attempt,
                page.after_sequence,
                page.through_sequence,
                page.limit,
            )
            .await?,
        ),
    ))
}
async fn usage(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(attempt): Path<Uuid>,
) -> std::result::Result<
    (
        [(header::HeaderName, &'static str); 1],
        Json<inspection::SessionUsage>,
    ),
    AppError,
> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(inspection::usage(&state, user.id, attempt).await?),
    ))
}
impl From<Error> for AppError {
    fn from(error: Error) -> Self {
        match error {
            Error::Refused => Self::Forbidden("observation source is not admitted".into()),
            Error::Conflict => Self::Conflict("observation identity or cursor conflicts".into()),
            Error::Capacity => Self::Maintenance("observation retention quota exhausted".into()),
            Error::Invalid => Self::BadRequest("invalid observation document".into()),
            Error::Authority(v) => v.into(),
            Error::Database(_) => Self::Internal,
        }
    }
}
async fn batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(packet): ApiJson<Packet>,
) -> std::result::Result<([(header::HeaderName, &'static str); 1], Json<Receipt>), AppError> {
    let node = crate::worker_api::authenticate(&state, &headers).await?;
    let receipt = ingest(&state, node.id, &packet).await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(receipt)))
}
