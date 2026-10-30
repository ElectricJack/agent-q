//! API error type shared by all routes: a status code plus an `ApiError` body.

use aq_protocol::{
    ApiError, BAD_REQUEST, CONFLICT, CSRF_REJECTED, FORBIDDEN, IDENTITY_UNAVAILABLE, INTERNAL,
    NOT_FOUND, UNAUTHENTICATED,
};
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::identity::IdentityError;
use crate::store::StoreError;

#[derive(Debug)]
pub enum AppError {
    Unauthenticated(String),
    Forbidden(String),
    CsrfRejected(String),
    IdentityUnavailable(String),
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal,
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::Unauthenticated(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) | AppError::CsrfRejected(_) => StatusCode::FORBIDDEN,
            AppError::IdentityUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn to_api_error(&self) -> ApiError {
        let (code, message) = match self {
            AppError::Unauthenticated(m) => (UNAUTHENTICATED, m.clone()),
            AppError::Forbidden(m) => (FORBIDDEN, m.clone()),
            AppError::CsrfRejected(m) => (CSRF_REJECTED, m.clone()),
            AppError::IdentityUnavailable(m) => (IDENTITY_UNAVAILABLE, m.clone()),
            AppError::NotFound(m) => (NOT_FOUND, m.clone()),
            AppError::BadRequest(m) => (BAD_REQUEST, m.clone()),
            AppError::Conflict(m) => (CONFLICT, m.clone()),
            AppError::Internal => (INTERNAL, "internal server error".to_string()),
        };
        ApiError {
            error: code.to_string(),
            message,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status(), Json(self.to_api_error())).into_response()
    }
}

impl From<IdentityError> for AppError {
    fn from(e: IdentityError) -> Self {
        match e {
            IdentityError::Missing => {
                AppError::Unauthenticated("authentication required".to_string())
            }
            IdentityError::Invalid(reason) => {
                AppError::Unauthenticated(format!("invalid credential: {reason}"))
            }
            IdentityError::KeysUnavailable(reason) => {
                tracing::error!(%reason, "identity signing keys unavailable");
                AppError::IdentityUnavailable(
                    "identity provider signing keys are temporarily unavailable".to_string(),
                )
            }
        }
    }
}

impl From<StoreError> for AppError {
    fn from(e: StoreError) -> Self {
        if matches!(e, StoreError::EmailTaken) {
            return AppError::Conflict("a user with that email already exists".to_string());
        }
        tracing::error!(error = %e, "store error");
        AppError::Internal
    }
}
