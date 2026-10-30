mod common;

use aq_server::identity::{
    CloudflareAccessVerifier, DevIdentity, IdentityError, IdentityVerifier, VerifiedIdentity,
};
use axum::http::{HeaderMap, HeaderValue};
use common::{TestKeys, serve_jwks};
use serde_json::{Value, json};

const ISS: &str = "https://team.cloudflareaccess.com";
const AUD: &str = "aud-tag-123";

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn claims(f: impl FnOnce(&mut Value)) -> Value {
    let mut c = json!({
        "iss": ISS, "aud": [AUD], "sub": "user-1", "email": "a@example.com",
        "exp": now() + 300, "iat": now(),
    });
    f(&mut c);
    c
}

fn headers(token: &str) -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(
        "cf-access-jwt-assertion",
        HeaderValue::from_str(token).unwrap(),
    );
    h
}

async fn verifier(jwks: Value) -> (CloudflareAccessVerifier, common::JwksServer) {
    let server = serve_jwks(jwks).await;
    let v = CloudflareAccessVerifier::with_jwks_url(
        ISS.into(),
        AUD.into(),
        server.base_url.clone(),
        reqwest::Client::new(),
    );
    (v, server)
}

async fn std_verifier() -> (CloudflareAccessVerifier, common::JwksServer) {
    verifier(TestKeys::get().jwk_set_json()).await
}

#[tokio::test]
async fn accepts_valid_token() {
    let (v, _s) = std_verifier().await;
    let t = TestKeys::get().mint(claims(|_| {}));
    let id = v.verify(&headers(&t)).await.unwrap();
    assert_eq!(
        id,
        VerifiedIdentity {
            issuer: ISS.into(),
            subject: "user-1".into(),
            email: "a@example.com".into()
        }
    );
}

#[tokio::test]
async fn rejects_wrong_audience() {
    let (v, _s) = std_verifier().await;
    let t = TestKeys::get().mint(claims(|c| c["aud"] = json!(["other"])));
    assert!(matches!(
        v.verify(&headers(&t)).await,
        Err(IdentityError::Invalid(_))
    ));
}

#[tokio::test]
async fn rejects_wrong_issuer() {
    let (v, _s) = std_verifier().await;
    let t = TestKeys::get().mint(claims(|c| c["iss"] = json!("https://evil.example.com")));
    assert!(matches!(
        v.verify(&headers(&t)).await,
        Err(IdentityError::Invalid(_))
    ));
}

#[tokio::test]
async fn rejects_expired_beyond_leeway() {
    let (v, _s) = std_verifier().await;
    let t = TestKeys::get().mint(claims(|c| c["exp"] = json!(now() - 120)));
    assert!(matches!(
        v.verify(&headers(&t)).await,
        Err(IdentityError::Invalid(_))
    ));
}

#[tokio::test]
async fn accepts_within_clock_skew_leeway() {
    let (v, _s) = std_verifier().await;
    let t = TestKeys::get().mint(claims(|c| {
        c["exp"] = json!(now() - 30);
        c["nbf"] = json!(now() + 30);
    }));
    assert!(v.verify(&headers(&t)).await.is_ok());
}

#[tokio::test]
async fn rejects_missing_email_claim() {
    let (v, _s) = std_verifier().await;
    let t = TestKeys::get().mint(claims(|c| {
        c.as_object_mut().unwrap().remove("email");
    }));
    assert!(matches!(
        v.verify(&headers(&t)).await,
        Err(IdentityError::Invalid(_))
    ));
}

#[tokio::test]
async fn rejects_hs256_token() {
    let (v, _s) = std_verifier().await;
    let mut h = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256);
    h.kid = Some("test-1".into());
    let t = jsonwebtoken::encode(
        &h,
        &claims(|_| {}),
        &jsonwebtoken::EncodingKey::from_secret(b"secret"),
    )
    .unwrap();
    assert!(matches!(
        v.verify(&headers(&t)).await,
        Err(IdentityError::Invalid(_))
    ));
}

#[tokio::test]
async fn ignores_email_header_without_jwt() {
    let (v, _s) = std_verifier().await;
    let mut h = HeaderMap::new();
    h.insert(
        "cf-access-authenticated-user-email",
        HeaderValue::from_static("a@example.com"),
    );
    assert_eq!(v.verify(&h).await, Err(IdentityError::Missing));
}

#[tokio::test]
async fn unreachable_jwks_is_keys_unavailable() {
    let v = CloudflareAccessVerifier::with_jwks_url(
        ISS.into(),
        AUD.into(),
        "http://127.0.0.1:1/jwks".into(),
        reqwest::Client::new(),
    );
    let t = TestKeys::get().mint(claims(|_| {}));
    assert!(matches!(
        v.verify(&headers(&t)).await,
        Err(IdentityError::KeysUnavailable(_))
    ));
}

#[tokio::test]
async fn refetches_jwks_on_unknown_kid_once() {
    let keys = TestKeys::get();
    let (v, server) = verifier(keys.jwk_set_json_with_kid("old")).await;

    let t = keys.mint_with_kid("old", claims(|_| {}));
    assert!(v.verify(&headers(&t)).await.is_ok());
    assert_eq!(server.fetch_count(), 1);

    server.set_body(keys.jwk_set_json_with_kid("test-1"));
    let t = keys.mint_with_kid("test-1", claims(|_| {}));
    assert!(v.verify(&headers(&t)).await.is_ok());
    assert_eq!(server.fetch_count(), 2);

    let t = keys.mint_with_kid("test-2", claims(|_| {}));
    assert_eq!(
        v.verify(&headers(&t)).await,
        Err(IdentityError::Invalid("unknown signing key".into()))
    );
    assert_eq!(server.fetch_count(), 2);
}

#[tokio::test]
async fn dev_identity_uses_override_header() {
    let v = DevIdentity::new("default@example.com".into());
    let id = v.verify(&HeaderMap::new()).await.unwrap();
    assert_eq!(id.email, "default@example.com");
    assert_eq!(id.issuer, "dev");
    assert_eq!(id.subject, "default@example.com");

    let mut h = HeaderMap::new();
    h.insert(
        "x-aq-dev-email",
        HeaderValue::from_static("other@example.com"),
    );
    let id = v.verify(&h).await.unwrap();
    assert_eq!(id.email, "other@example.com");
    assert_eq!(id.subject, "other@example.com");
}
