//! Explicitly constructed route pending ordered migration/runtime registration.
//! Worker bearer credentials only; no source/policy registration on this surface.
use super::*;
use crate::{admin_api::ApiJson, error::AppError};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, header},
    routing::post,
};

pub fn routes(mut state: AppState) -> Router<AppState> {
    state.store = state.store.observation_view();
    Router::new()
        .route("/v1/worker/observations/batch", post(batch))
        .layer(DefaultBodyLimit::max(MAX_BATCH_BYTES))
        .with_state(state)
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
