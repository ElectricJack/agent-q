//! Explicit negotiated machine-only report routes; no execution authority reply.
use crate::{AppState, error::AppError};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State, rejection::BytesRejection},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::post,
};
struct Error(AppError);
impl From<AppError> for Error {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let mut r = self.0.into_response();
        r.headers_mut()
            .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
        r
    }
}
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/v1/worker/profiled/inventory-v2", post(inventory))
        .route("/v1/worker/profiled/material-presence", post(report))
        .layer(DefaultBodyLimit::max(
            aq_admission::scoped_presence::MAX_REQUEST_BYTES,
        ))
        .with_state(state.presence_view())
}
async fn inventory(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, Error> {
    let auth = crate::worker_api::authenticate_context(&state, &headers).await?;
    let body = body.map_err(|_| AppError::PayloadTooLarge("typed inventory body bound".into()))?;
    let request = aq_admission::scoped_presence::decode_inventory(&body)
        .map_err(|_| AppError::BadRequest("invalid typed inventory".into()))?;
    let receipt = super::inventory::record(&state, &auth, &request).await?;
    Ok(no_store(Json(receipt).into_response()))
}
async fn report(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, Error> {
    let auth = crate::worker_api::authenticate_context(&state, &headers).await?;
    let body = body.map_err(|_| AppError::PayloadTooLarge("scoped presence body bound".into()))?;
    let request = aq_admission::scoped_presence::decode_report(&body)
        .map_err(|_| AppError::BadRequest("invalid scoped presence".into()))?;
    let receipt = super::report::record(&state, &auth, &request).await?;
    Ok(no_store(Json(receipt).into_response()))
}
fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}
