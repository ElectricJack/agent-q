//! Owner-only deterministic inspection of explicit catalog/context snapshots.
//! These endpoints neither resolve current project bindings nor admit launches.
use super::{Result, invalid};
use crate::{AppState, authz::RequireOwner, error::AppError};
use aq_profiles::{Compilation, CompileRequest, ValidatedCatalog};
use axum::{Json, Router, extract::DefaultBodyLimit, routing::post};
use serde::{Deserialize, Serialize};

const MAX_ARTIFACT: usize = 2_097_152;
const MAX_COMPILE: usize = 2_097_152;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InspectionRequest {
    artifact_json: String,
    composition: Option<CompileRequest>,
    graph_access_profile_digest: Option<String>,
}
#[derive(Serialize)]
struct Inspection {
    catalog_valid: bool,
    catalog_digest: String,
    selection_pending: bool,
    /// Caller-supplied snapshots are inspected, never promoted to current authority.
    authority_admission: bool,
    compilation: Option<Compilation>,
}
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/profiles/validate", post(validate))
        .route("/api/profiles/explain", post(explain))
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
}
async fn inspect(
    request: InspectionRequest,
    require_composition: bool,
) -> Result<Json<Inspection>> {
    if request.artifact_json.len() > MAX_ARTIFACT {
        return Err(AppError::PayloadTooLarge(
            "catalog artifact exceeds 2 MiB".into(),
        ));
    }
    if require_composition && request.composition.is_none() {
        return Err(AppError::BadRequest(
            "explain requires a composition snapshot".into(),
        ));
    }
    if request.composition.is_some() != request.graph_access_profile_digest.is_some() {
        return Err(AppError::BadRequest(
            "composition and access-profile digest must be supplied together".into(),
        ));
    }
    if request
        .composition
        .as_ref()
        .is_some_and(|r| serde_json::to_vec(r).map_or(true, |v| v.len() > MAX_COMPILE))
    {
        return Err(AppError::PayloadTooLarge(
            "composition snapshot exceeds 2 MiB".into(),
        ));
    }
    tokio::task::spawn_blocking(move || {
        let catalog = ValidatedCatalog::from_artifact(request.artifact_json.as_bytes())
            .map_err(|_| invalid())?;
        let compilation = request
            .composition
            .as_ref()
            .map(|r| {
                aq_profiles::compile(
                    &catalog,
                    r,
                    request.graph_access_profile_digest.as_deref().unwrap(),
                )
            })
            .transpose()
            .map_err(|_| AppError::BadRequest("profile composition refused".into()))?;
        Ok(Json(Inspection {
            catalog_valid: true,
            catalog_digest: catalog.digest().into(),
            selection_pending: true,
            authority_admission: false,
            compilation,
        }))
    })
    .await
    .map_err(|_| AppError::Internal)?
}
async fn validate(
    RequireOwner(_): RequireOwner,
    Json(request): Json<InspectionRequest>,
) -> Result<Json<Inspection>> {
    inspect(request, false).await
}
async fn explain(
    RequireOwner(_): RequireOwner,
    Json(request): Json<InspectionRequest>,
) -> Result<Json<Inspection>> {
    inspect(request, true).await
}
