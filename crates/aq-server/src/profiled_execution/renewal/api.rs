//! Current-machine renewal on a bounded lane independent of bulk preparation.
use super::super::control_api::{Error, nonce};
use crate::{AppState, error::AppError};
use aq_admission::{control::ControlReply, transport::Renew};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State, rejection::BytesRejection},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::post,
};
pub fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/v1/worker/profiled/renew",
            post(renew).layer(DefaultBodyLimit::max(16_384)),
        )
        .with_state(state.renewal_view())
}
async fn renew(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Response, Error> {
    let nonce = nonce(&headers)?;
    let auth = crate::delegated_permits::runtime::authenticate_runner(
        &state,
        &headers,
        aq_admission_sandbox::sandbox::RunnerOperation::Renew,
        None,
    )
    .await?;
    let body = body.map_err(|_| AppError::PayloadTooLarge("renewal body bound".into()))?;
    if auth.delegated().is_some() {
        use aq_admission_sandbox::sandbox::{PhaseProfile, SandboxDocument};
        let SandboxDocument::RunnerRenewalRequest(request) = serde_json::from_slice(&body)
            .map_err(|_| AppError::BadRequest("invalid delegated renewal document".into()))?
        else {
            return Err(AppError::BadRequest("expected runner renewal request".into()).into());
        };
        crate::delegated_permits::runtime::require_document(&auth, &request)?;
        let reply = if let Some(reply) = crate::delegated_permits::incarnation::renew_after_reboot(
            &state, &auth, &request, nonce,
        )
        .await?
        {
            reply
        } else {
            match request.authority.profile {
                PhaseProfile::PhaseV2 => {
                    crate::profiled_execution::phase_execution::renew_delegated(
                        &state, &auth, &request, nonce,
                    )
                    .await?
                }
                PhaseProfile::PhaseRefreshV1 => {
                    crate::profiled_execution::phase_refresh_execution::renew_delegated(
                        &state, &auth, &request, nonce,
                    )
                    .await?
                }
            }
        };
        let mut response = Json(SandboxDocument::RunnerRenewalResponse(reply)).into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
        return Ok(response);
    }
    let request: Renew = serde_json::from_slice(&body)
        .map_err(|_| AppError::BadRequest("invalid renewal document".into()))?;
    let reply = super::service::renew(&state, &auth, &request, nonce).await?;
    let mut response = Json(ControlReply::Renewed(reply)).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    Ok(response)
}
