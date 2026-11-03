//! Credential metadata and backend restrictions shared by selection and material owners.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(JsonSchema, Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialKind {
    #[default]
    ApiKey,
    SubscriptionLogin,
}

#[derive(
    JsonSchema, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord,
)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    DockerLocal,
    Boat,
    CloudflareContainers,
}

/// Unrestricted policy: eligibility of the backend itself is owned by placement.
pub fn default_allowed_backends() -> Vec<BackendKind> {
    vec![
        BackendKind::DockerLocal,
        BackendKind::Boat,
        BackendKind::CloudflareContainers,
    ]
}

pub fn valid_allowed_backends(backends: &[BackendKind]) -> bool {
    backends.len() <= 3 && backends.iter().collect::<BTreeSet<_>>().len() == backends.len()
}

pub fn unrestricted_backends(backends: &[BackendKind]) -> bool {
    backends.len() == 3 && valid_allowed_backends(backends)
}

/// Missing backend identity is compatible only with unrestricted credentials.
/// Callers supply trusted node facts, never a caller-provided placement hint.
pub fn credential_backend_policy(
    allowed: &[BackendKind],
    backend: Option<BackendKind>,
) -> Result<(), &'static str> {
    if !valid_allowed_backends(allowed) {
        return Err("credential_backend_policy_invalid");
    }
    match backend {
        Some(backend) if allowed.contains(&backend) => Ok(()),
        Some(_) => Err("credential_backend_not_allowed"),
        None if unrestricted_backends(allowed) => Ok(()),
        None => Err("credential_backend_unknown"),
    }
}
