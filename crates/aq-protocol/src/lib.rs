//! Wire types shared by the Agent Q server and worker.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

/// Inclusive (min, max) worker protocol versions the server accepts.
pub const SUPPORTED_WORKER_PROTOCOLS: (u32, u32) = (1, 1);

pub const UNSUPPORTED_PROTOCOL: &str = "unsupported_protocol";
pub const INVALID_CREDENTIAL: &str = "invalid_credential";
pub const CREDENTIAL_REVOKED: &str = "credential_revoked";
pub const ENROLLMENT_INVALID: &str = "enrollment_invalid";
pub const FORBIDDEN: &str = "forbidden";
pub const UNAUTHENTICATED: &str = "unauthenticated";
pub const CSRF_REJECTED: &str = "csrf_rejected";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    pub server_version: String,
    pub source_revision: String,
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub state_format_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_round_trips() {
        let e = ApiError {
            error: FORBIDDEN.into(),
            message: "no".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, r#"{"error":"forbidden","message":"no"}"#);
        assert_eq!(serde_json::from_str::<ApiError>(&json).unwrap(), e);
    }
}
