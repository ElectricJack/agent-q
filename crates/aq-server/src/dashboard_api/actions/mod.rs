//! Durable finite action boundary. Production intentionally registers no kinds.
pub mod contract;
pub mod journal;
mod json;
mod registry;
use super::{DashboardUser, contract::Scope, errors::ApiError};
use crate::{AppState, store::User};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    routing::{get, post},
};
pub use contract::*;
pub use registry::{ActionKind, Registry};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// A committed receipt survives cancellation by an outer request deadline.
/// This is request-local evidence only; the SQL journal remains authoritative.
#[derive(Clone, Default)]
pub(crate) struct AdmissionReceipt(Arc<Mutex<Option<OperationResponse>>>);
impl AdmissionReceipt {
    fn record(&self, value: OperationResponse) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(value);
    }
    pub(crate) fn retained_response(&self) -> Option<axum::response::Response> {
        use axum::response::IntoResponse;
        let value = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()?;
        Some((operation_status(&value.data), Json(value)).into_response())
    }
}
fn operation_status(operation: &Operation) -> StatusCode {
    if matches!(
        operation.disposition,
        Disposition::Pending | Disposition::Unknown
    ) {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    }
}

pub fn routes(registry: Arc<Registry>) -> Router<AppState> {
    Router::new()
        .route("/api/v1/actions", post(post_action))
        .route("/api/v1/operations/{id}", get(get_operation))
        .layer(DefaultBodyLimit::max(65536))
        .layer(Extension(registry))
}
async fn post_action(
    State(state): State<AppState>,
    DashboardUser(user): DashboardUser,
    Extension(registry): Extension<Arc<Registry>>,
    receipt: Option<Extension<AdmissionReceipt>>,
    body: Result<Json<json::UniqueJson>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, Json<OperationResponse>), ApiError> {
    let Json(json::UniqueJson(raw)) = body.map_err(|e| ApiError::for_status(e.status()))?;
    let request: ActionRequest = serde_json::from_value(raw).map_err(|_| ApiError::invalid())?;
    let operation = submit_observed(
        &state,
        &registry,
        &user,
        request,
        receipt.as_ref().map(|r| &r.0),
    )
    .await?;
    let status = operation_status(&operation);
    let response = response(&state, operation);
    if let Some(receipt) = receipt {
        receipt.record(response.clone());
    }
    Ok((status, Json(response)))
}
async fn get_operation(
    State(state): State<AppState>,
    DashboardUser(user): DashboardUser,
    Extension(registry): Extension<Arc<Registry>>,
    path: Result<Path<Uuid>, axum::extract::rejection::PathRejection>,
) -> Result<Json<OperationResponse>, ApiError> {
    let Path(id) = path.map_err(|_| ApiError::invalid())?;
    let lookup = journal::lookup_for_authorization(&state.store.pool, user.id, id).await?;
    let adapter = registry
        .get(&lookup.kind)
        .map_err(|_| ApiError::for_status(StatusCode::NOT_FOUND))?;
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        adapter.authorize_lookup(&state, &user, &lookup.scope, lookup.operation.as_ref()),
    )
    .await
    .map_err(|_| ApiError::unavailable())?
    .map_err(|error| {
        if matches!(error.status().as_u16(), 401 | 403 | 404) {
            ApiError::for_status(StatusCode::NOT_FOUND)
        } else {
            error
        }
    })?;
    Ok(Json(response(
        &state,
        lookup.operation.ok_or_else(|| journal::retired(id))?,
    )))
}
fn response(state: &AppState, operation: Operation) -> OperationResponse {
    use super::contract::{Completeness, Coverage, Observation, Quality, Timestamp};
    let now = (state.clock)().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    OperationResponse {
        api_version: ActionVersion::V1,
        request_id: Uuid::new_v4(),
        server_time: Timestamp(now.clone()),
        scope: operation.scope.clone(),
        pending_operations: if matches!(
            operation.disposition,
            Disposition::Pending | Disposition::Unknown
        ) {
            vec![operation.operation_id]
        } else {
            vec![]
        },
        data: operation,
        observation: Observation {
            observed_at: Timestamp(now.clone()),
            fresh_until: Timestamp(now),
            source: "aq_operation_journal".into(),
            quality: Quality::Reported,
            coverage: Completeness::Complete,
        },
        coverage: Coverage {
            state: Completeness::Complete,
            reason: None,
        },
    }
}
#[cfg(test)]
async fn submit(
    state: &AppState,
    registry: &Registry,
    user: &User,
    request: ActionRequest,
) -> Result<Operation, ApiError> {
    submit_observed(state, registry, user, request, None).await
}
async fn submit_observed(
    state: &AppState,
    registry: &Registry,
    user: &User,
    request: ActionRequest,
    receipt: Option<&AdmissionReceipt>,
) -> Result<Operation, ApiError> {
    request.validate()?;
    let adapter = registry.get(&request.kind)?;
    let scope = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        adapter.authorize(state, user, &request),
    )
    .await
    .map_err(|_| ApiError::unavailable())??;
    let digest = canonical_digest(&request, &scope)?;
    let (original, dispatch) = journal::admit(
        &state.store.pool,
        user.id,
        &request,
        scope,
        digest,
        (state.clock)(),
    )
    .await?;
    if let Some(receipt) = receipt {
        receipt.record(response(state, original.clone()));
    }
    if !dispatch {
        return Ok(original);
    }
    let outcome = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        adapter.execute(state, user, &request),
    )
    .await
    .unwrap_or_else(|_| Outcome::unknown());
    let mut completed = original.clone();
    if outcome.receipt_refs.len() <= 100
        && outcome.affected_entities.len() <= 100
        && outcome
            .receipt_refs
            .iter()
            .chain(&outcome.affected_entities)
            .all(EntityRef::valid)
        && (outcome.disposition == Disposition::Confirmed
            || (outcome.receipt_refs.is_empty() && outcome.affected_entities.is_empty()))
    {
        completed.disposition = outcome.disposition;
        completed.receipt_refs = outcome.receipt_refs;
        completed.affected_entities = outcome.affected_entities;
    } else {
        completed.disposition = Disposition::Unknown;
    }
    completed.recovery = match completed.disposition {
        Disposition::Pending | Disposition::Unknown => Recovery::LookupOriginal,
        Disposition::Conflict | Disposition::Rejected => Recovery::RefreshAndReview,
        Disposition::Confirmed => Recovery::None,
    };
    let now = (state.clock)();
    completed.updated_at = now
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        .max(original.updated_at.clone());
    if serde_json::to_vec(&completed).map_or(true, |v| v.len() > 60000) {
        completed = original.clone();
        completed.disposition = Disposition::Unknown;
    }
    if journal::finish(&state.store.pool, user.id, &completed, now)
        .await
        .is_err()
    {
        // The receipt write itself may have committed. Re-read if possible;
        // otherwise return the Pending disposition whose admission is proven.
        return Ok(
            journal::lookup(&state.store.pool, user.id, request.operation_id)
                .await
                .unwrap_or(original),
        );
    }
    Ok(completed)
}
fn canonical_digest(request: &ActionRequest, scope: &Scope) -> Result<String, ApiError> {
    use sha2::{Digest, Sha256};
    let mut canonical = serde_json::json!({"request":request,"scope":scope});
    canonical.sort_all_objects();
    let bytes = serde_json::to_vec(&canonical).map_err(|_| ApiError::invalid())?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}
#[cfg(test)]
mod tests;
