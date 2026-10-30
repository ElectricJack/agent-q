use async_trait::async_trait;
use axum::http::HeaderMap;

use super::{IdentityError, IdentityVerifier, VerifiedIdentity};

pub const DEV_EMAIL_HEADER: &str = "x-aq-dev-email";

/// Local-development identity. Only constructed for `AuthConfig::Dev`, which
/// configuration validation forbids in production.
pub struct DevIdentity {
    default_email: String,
}

impl DevIdentity {
    pub fn new(default_email: String) -> Self {
        Self { default_email }
    }
}

#[async_trait]
impl IdentityVerifier for DevIdentity {
    async fn verify(&self, headers: &HeaderMap) -> Result<VerifiedIdentity, IdentityError> {
        let email = match headers.get(DEV_EMAIL_HEADER) {
            Some(v) => v
                .to_str()
                .map_err(|_| IdentityError::Invalid("unreadable dev email header".into()))?
                .trim()
                .to_string(),
            None => self.default_email.clone(),
        };
        if email.is_empty() {
            return Err(IdentityError::Invalid("empty dev email".into()));
        }
        Ok(VerifiedIdentity {
            issuer: "dev".into(),
            subject: email.clone(),
            email,
        })
    }
}
