use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Written by the server into `requests/`. `version: None` means the channel's current release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRequest {
    pub id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<Version>,
    pub requested_by: String,
    pub requested_at: DateTime<Utc>,
}

/// Persisted update steps, in order. The journal records a step before acting on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Requested,
    Validated,
    Pulled,
    Maintenance,
    BackedUp,
    Replacing,
    Probing,
    Active,
    RollingBack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSummary {
    pub version: Version,
    pub source_revision: String,
    pub server_image: String,
    pub state_format: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Succeeded,
    RolledBack,
    Refused,
    Rejected,
    Aborted,
    FailedNeedsOperator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    pub request_id: Option<Uuid>,
    pub from: Option<Version>,
    pub to: Option<Version>,
    pub outcome: Outcome,
    pub detail: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateState {
    Idle,
    InProgress {
        request_id: Uuid,
        target: Version,
        step: Step,
        started_at: DateTime<Utc>,
    },
    FailedNeedsOperator {
        detail: String,
    },
}

/// Written by the updater to `status/status.json`, read by the server.
/// `history` holds at most 20 entries, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateStatus {
    pub updater_version: String,
    pub state: UpdateState,
    pub active: Option<ReleaseSummary>,
    pub previous: Option<ReleaseSummary>,
    pub available: Option<ReleaseSummary>,
    pub history: Vec<HistoryEntry>,
    pub updated_at: DateTime<Utc>,
}

/// Body of the server's loopback `/readyz`, parsed by the updater's probe.
/// `database` is "ok" or "error".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessReport {
    pub version: Version,
    pub source_revision: String,
    pub state_format: u32,
    pub protocol_min: u32,
    pub protocol_max: u32,
    pub database: String,
    pub auth_mode: String,
}
