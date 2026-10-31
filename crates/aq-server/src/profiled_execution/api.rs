//! Native admission wire on the machine listener; no legacy fixture decoder.
use crate::{AppState, error::AppError};
use aq_admission::transport::{TransportDocument, decode_transport};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::HeaderMap,
    routing::post,
};
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/v1/worker/profiled/register", post(register))
        .layer(DefaultBodyLimit::max(65_536))
        .with_state(state.control_view())
}
async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<TransportDocument>, AppError> {
    let auth = crate::worker_api::authenticate_context(&state, &headers).await?;
    let TransportDocument::Register(request) = decode_transport(&body)
        .map_err(|_| AppError::BadRequest("invalid native registration document".into()))?
    else {
        return Err(AppError::BadRequest(
            "expected native registration document".into(),
        ));
    };
    Ok(Json(TransportDocument::Register(
        super::registration::register(&state, &auth, &request).await?,
    )))
}
