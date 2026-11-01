//! Explicitly injected owner only; production/default routers do not install it.
use super::{policy::InstalledPolicy, service};
use crate::{AppState, error::AppError, profiled_execution::control_api::Error};
use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State, rejection::BytesRejection},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::post,
};
use std::sync::Arc;
pub(crate) fn routes(state: AppState, policy: Arc<InstalledPolicy>) -> Router<AppState> {
    Router::new()
        .route(
            "/v1/worker/profiled/launch-closure",
            post(record).layer(DefaultBodyLimit::max(
                aq_admission::granted_closure::MAX_BYTES,
            )),
        )
        .layer(Extension(policy))
        .with_state(state.control_view())
}
async fn record(
    State(state): State<AppState>,
    Extension(policy): Extension<Arc<InstalledPolicy>>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, Error> {
    let node = crate::worker_api::authenticate_context(&state, &headers).await?;
    let body = body.map_err(|_| AppError::PayloadTooLarge("closure request bound".into()))?;
    let request = aq_admission::granted_closure::decode_request(&body)
        .map_err(|_| AppError::BadRequest("invalid closure request".into()))?;
    let receipt = service::record(&state, &policy, &node, &request).await?;
    let mut response = Json(receipt).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    Ok(response)
}
