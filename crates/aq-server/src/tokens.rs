//! Random credential generation and SHA-256 verifiers.
//!
//! The server stores only the verifier of a credential. A plain hash is
//! sufficient because the secrets are high-entropy random values.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};

fn random_token(prefix: &str, bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buf);
    format!("{prefix}{}", URL_SAFE_NO_PAD.encode(buf))
}

pub fn new_enrollment_code() -> String {
    random_token("aqe_", 20)
}

pub fn new_node_credential() -> String {
    random_token("aqn_", 32)
}

/// Lowercase hex SHA-256 of `secret`.
pub fn verifier(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}
