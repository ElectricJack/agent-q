//! Protected adapters only. Domain owners retain current actor, exact subject,
//! source/receipt and original-effect identity checks. No decision is admitted
//! here and a GET never dispatches a completion.
use super::*;
use crate::{admin_api::ApiJson, authz::RequireOwner, profiled_execution::settlement};
use axum::{
    Extension, Json, Router,
    extract::Path,
    routing::{get, post},
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveCombined {
    expected_subject: ArtifactRef,
    answer: ResolveRequest,
}
/// Merged only inside the existing configured reviews router, beneath its
/// Service extension and body limit and the daemon's human protection layers.
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/reviews/combined/{id}", get(inspect))
        .route("/api/reviews/combined/{id}/resolve", post(resolve))
        .route("/api/settlement/evaluations", post(evaluate))
        .route("/api/settlement/evaluations/{id}", get(lookup))
        .route(
            "/api/settlement/completions/{id}/reconcile",
            post(reconcile),
        )
}
fn owner(service: &Service) -> settlement::Service {
    settlement::Service::new(service.state.clone(), service.clone())
}
async fn inspect(
    RequireOwner(actor): RequireOwner,
    Extension(service): Extension<Arc<Service>>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    Ok(Json(service.inspect_combined_as(&actor, id).await?))
}
async fn resolve(
    RequireOwner(actor): RequireOwner,
    Extension(service): Extension<Arc<Service>>,
    Path(id): Path<Uuid>,
    ApiJson(request): ApiJson<ResolveCombined>,
) -> Result<Json<DecisionReceipt>> {
    if id != request.answer.decision_id {
        return Err(invalid());
    }
    Ok(Json(
        service
            .resolve_combined_as(&actor, &request.answer, &request.expected_subject)
            .await?,
    ))
}
async fn evaluate(
    RequireOwner(actor): RequireOwner,
    Extension(service): Extension<Arc<Service>>,
    ApiJson(request): ApiJson<settlement::EvaluateRequest>,
) -> Result<Json<settlement::EvaluationReceipt>> {
    Ok(Json(owner(&service).evaluate(&actor, &request).await?))
}
async fn lookup(
    RequireOwner(actor): RequireOwner,
    Extension(service): Extension<Arc<Service>>,
    Path(id): Path<Uuid>,
) -> Result<Json<settlement::EvaluationReceipt>> {
    Ok(Json(owner(&service).lookup(&actor, id).await?))
}
async fn reconcile(
    RequireOwner(actor): RequireOwner,
    Extension(service): Extension<Arc<Service>>,
    Path(id): Path<Uuid>,
    ApiJson(request): ApiJson<serde_json::Value>,
) -> Result<Json<settlement::CompletionStatus>> {
    // A derived empty struct also accepts an empty sequence in serde. Require
    // the literal empty-object shape, not []/null or an unbound extra option.
    if !request.as_object().is_some_and(|object| object.is_empty()) {
        return Err(AppError::BadRequest(
            "reconcile requires an empty JSON object".into(),
        ));
    }
    Ok(Json(owner(&service).complete(&actor, id).await?))
}
