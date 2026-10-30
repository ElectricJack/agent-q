use std::time::{Duration, Instant};

use async_trait::async_trait;
use axum::http::HeaderMap;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::Mutex;

use super::{IdentityError, IdentityVerifier, VerifiedIdentity};

const JWT_HEADER: &str = "cf-access-jwt-assertion";
const UNKNOWN_KID_REFETCH_INTERVAL: Duration = Duration::from_secs(60);
const LEEWAY_SECS: u64 = 60;
const JWKS_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct KeyCache {
    jwks: Option<JwkSet>,
    /// When the last refetch caused by an unknown `kid` happened. The first
    /// (lazy) fetch is not counted.
    last_unknown_kid_refetch: Option<Instant>,
}

/// Verifies the signed `Cf-Access-Jwt-Assertion` header. Never trusts the
/// plain email header.
pub struct CloudflareAccessVerifier {
    issuer: String,
    audience: String,
    jwks_url: String,
    http: reqwest::Client,
    fetch_timeout: Duration,
    cache: Mutex<KeyCache>,
}

#[derive(Deserialize)]
struct Claims {
    iss: String,
    sub: String,
    email: String,
}

impl CloudflareAccessVerifier {
    pub fn new(team_domain: String, audience: String, http: reqwest::Client) -> Self {
        let issuer = team_domain.trim_end_matches('/').to_string();
        let jwks_url = format!("{issuer}/cdn-cgi/access/certs");
        Self::with_jwks_url(issuer, audience, jwks_url, http)
    }

    /// Like `new`, but with an explicit JWKS URL (for tests).
    pub fn with_jwks_url(
        issuer: String,
        audience: String,
        jwks_url: String,
        http: reqwest::Client,
    ) -> Self {
        Self {
            issuer: issuer.trim_end_matches('/').to_string(),
            audience,
            jwks_url,
            http,
            fetch_timeout: JWKS_FETCH_TIMEOUT,
            cache: Mutex::new(KeyCache::default()),
        }
    }

    /// Overrides the JWKS fetch timeout (default 5 s).
    pub fn with_fetch_timeout(mut self, timeout: Duration) -> Self {
        self.fetch_timeout = timeout;
        self
    }

    async fn fetch_jwks(&self) -> Result<JwkSet, IdentityError> {
        let unavailable = |what: String| IdentityError::KeysUnavailable(what);
        let resp = self
            .http
            .get(&self.jwks_url)
            .timeout(self.fetch_timeout)
            .send()
            .await
            .map_err(|e| unavailable(format!("fetching signing keys failed: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(unavailable(format!(
                "signing key endpoint returned {status}"
            )));
        }
        resp.json::<JwkSet>()
            .await
            .map_err(|e| unavailable(format!("signing key response was not valid JWKS: {e}")))
    }

    /// Finds the decoding key for `kid`, fetching lazily and refetching at
    /// most once per interval for unknown key ids. The lock is held across the
    /// fetch so concurrent verifies cannot trigger duplicate refetches.
    async fn key_for(&self, kid: &str) -> Result<DecodingKey, IdentityError> {
        let mut cache = self.cache.lock().await;
        if cache.jwks.is_none() {
            cache.jwks = Some(self.fetch_jwks().await?);
        }
        if let Some(key) = Self::lookup(cache.jwks.as_ref(), kid)? {
            return Ok(key);
        }
        let unknown = || IdentityError::Invalid("unknown signing key".into());
        if let Some(last) = cache.last_unknown_kid_refetch
            && last.elapsed() < UNKNOWN_KID_REFETCH_INTERVAL
        {
            return Err(unknown());
        }
        cache.last_unknown_kid_refetch = Some(Instant::now());
        cache.jwks = Some(self.fetch_jwks().await?);
        Self::lookup(cache.jwks.as_ref(), kid)?.ok_or_else(unknown)
    }

    fn lookup(jwks: Option<&JwkSet>, kid: &str) -> Result<Option<DecodingKey>, IdentityError> {
        let Some(jwk) = jwks.and_then(|s| s.find(kid)) else {
            return Ok(None);
        };
        DecodingKey::from_jwk(jwk)
            .map(Some)
            .map_err(|_| IdentityError::Invalid("unusable signing key".into()))
    }
}

#[async_trait]
impl IdentityVerifier for CloudflareAccessVerifier {
    async fn verify(&self, headers: &HeaderMap) -> Result<VerifiedIdentity, IdentityError> {
        let token = headers
            .get(JWT_HEADER)
            .ok_or(IdentityError::Missing)?
            .to_str()
            .map_err(|_| IdentityError::Invalid("unreadable token header".into()))?
            .trim();
        if token.is_empty() {
            return Err(IdentityError::Missing);
        }

        let header =
            decode_header(token).map_err(|_| IdentityError::Invalid("malformed token".into()))?;
        if header.alg != Algorithm::RS256 {
            return Err(IdentityError::Invalid(
                "unsupported signing algorithm".into(),
            ));
        }
        let kid = header
            .kid
            .ok_or_else(|| IdentityError::Invalid("token has no key id".into()))?;
        let key = self.key_for(&kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = LEEWAY_SECS;
        validation.validate_nbf = true;

        let data = decode::<Claims>(token, &key, &validation).map_err(|e| {
            use jsonwebtoken::errors::ErrorKind::*;
            IdentityError::Invalid(
                match e.kind() {
                    ExpiredSignature => "token expired",
                    ImmatureSignature => "token not yet valid",
                    InvalidAudience => "wrong audience",
                    InvalidIssuer => "wrong issuer",
                    InvalidSignature => "bad signature",
                    MissingRequiredClaim(_) | Json(_) => "missing or malformed claims",
                    _ => "token rejected",
                }
                .into(),
            )
        })?;
        let c = data.claims;
        if c.email.is_empty() || c.sub.is_empty() {
            return Err(IdentityError::Invalid("missing or malformed claims".into()));
        }
        Ok(VerifiedIdentity {
            issuer: c.iss,
            subject: c.sub,
            email: c.email,
        })
    }
}
