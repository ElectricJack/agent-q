//! Identity adapters: map an incoming request's headers to a verified identity.

mod cloudflare;
mod dev;

use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderMap;

use crate::config::AuthConfig;

pub use cloudflare::CloudflareAccessVerifier;
pub use dev::DevIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedIdentity {
    pub issuer: String,
    pub subject: String,
    pub email: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdentityError {
    /// No credential was presented (maps to 401).
    #[error("missing credential")]
    Missing,
    /// A credential was presented but rejected (maps to 401).
    #[error("invalid credential: {0}")]
    Invalid(String),
    /// The signing keys could not be fetched (maps to 503).
    #[error("signing keys unavailable: {0}")]
    KeysUnavailable(String),
}

#[async_trait]
pub trait IdentityVerifier: Send + Sync {
    async fn verify(&self, headers: &HeaderMap) -> Result<VerifiedIdentity, IdentityError>;
}

pub fn verifier_from_config(auth: &AuthConfig, http: reqwest::Client) -> Arc<dyn IdentityVerifier> {
    match auth {
        AuthConfig::Dev { dev_email } => Arc::new(DevIdentity::new(dev_email.clone())),
        AuthConfig::CloudflareAccess {
            team_domain,
            audience,
        } => Arc::new(CloudflareAccessVerifier::new(
            team_domain.clone(),
            audience.clone(),
            http,
        )),
    }
}
