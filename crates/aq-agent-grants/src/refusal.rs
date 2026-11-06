//! The structured refusal (§6.8). It describes only the caller's own
//! authority: the command, the grant it lacked and the grants of the same
//! family it holds. It never names or confirms any other record.
use super::Grant;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// The caller holds no grant for this command.
    GrantMissing,
    /// The caller holds the grant, but the target is outside its scope.
    ScopeDenied,
    /// The command can never be granted to an agent.
    OwnerOnly,
    /// A grant limit such as `max_created_tasks` was reached.
    QuotaExceeded,
    SessionRevoked,
    SessionExpired,
    /// The channel proxy refused the path (aq-worker).
    ChannelRefused,
    /// The command surface's own errors: no such command, no route for this
    /// principal, a malformed request, no credential, or a server failure.
    UnknownCommand,
    NotAvailable,
    InvalidRequest,
    Unauthorized,
    Internal,
}
/// Every code, in the published order.
pub const CODES: [RefusalCode; 12] = [
    RefusalCode::GrantMissing,
    RefusalCode::ScopeDenied,
    RefusalCode::OwnerOnly,
    RefusalCode::QuotaExceeded,
    RefusalCode::SessionRevoked,
    RefusalCode::SessionExpired,
    RefusalCode::ChannelRefused,
    RefusalCode::UnknownCommand,
    RefusalCode::NotAvailable,
    RefusalCode::InvalidRequest,
    RefusalCode::Unauthorized,
    RefusalCode::Internal,
];
impl RefusalCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GrantMissing => "grant_missing",
            Self::ScopeDenied => "scope_denied",
            Self::OwnerOnly => "owner_only",
            Self::QuotaExceeded => "quota_exceeded",
            Self::SessionRevoked => "session_revoked",
            Self::SessionExpired => "session_expired",
            Self::ChannelRefused => "channel_refused",
            Self::UnknownCommand => "unknown_command",
            Self::NotAvailable => "not_available",
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::Internal => "internal",
        }
    }
    /// The closed agent-access-1 `ErrorResponse` code that carries this
    /// refusal on the `/v1/agent/*` routes for clients that did not opt in
    /// to the structured shape.
    pub fn legacy(self) -> &'static str {
        match self {
            Self::GrantMissing | Self::OwnerOnly | Self::NotAvailable => "unsupported_capability",
            Self::UnknownCommand | Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::Internal => "internal",
            Self::ScopeDenied | Self::QuotaExceeded | Self::ChannelRefused => "scope_denied",
            Self::SessionRevoked => "session_revoked",
            Self::SessionExpired => "session_expired",
        }
    }
    pub fn status(self) -> u16 {
        match self {
            Self::SessionRevoked | Self::SessionExpired | Self::Unauthorized => 401,
            Self::UnknownCommand => 404,
            Self::InvalidRequest => 400,
            Self::Internal => 500,
            _ => 403,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    pub code: RefusalCode,
    pub command: String,
    /// The operation the command needs.
    pub grant: Option<String>,
    /// Grants of the same family the caller holds, with their scopes.
    pub held: Vec<Grant>,
    pub message: String,
    /// Always `not_applied`: a refusal is decided before any effect.
    pub effect: String,
    pub request_id: Option<Uuid>,
}
fn held_text(held: &[Grant]) -> String {
    if held.is_empty() {
        return "no related grant".into();
    }
    held.iter()
        .map(|g| format!("{} @ {}", g.operation, g.scope))
        .collect::<Vec<_>>()
        .join(", ")
}
const ASK: &str = "Use `aq question ask` to request it.";
impl Refusal {
    fn new(
        code: RefusalCode,
        command: &str,
        grant: &str,
        held: Vec<Grant>,
        message: String,
    ) -> Self {
        Self {
            code,
            command: command.into(),
            grant: Some(grant.into()),
            held,
            message,
            effect: "not_applied".into(),
            request_id: None,
        }
    }
    pub fn grant_missing(command: &str, grant: &str, held: Vec<Grant>) -> Self {
        let message = format!(
            "This agent does not hold the `{grant}` grant that `{command}` needs (holds: {}). {ASK}",
            held_text(&held)
        );
        Self::new(RefusalCode::GrantMissing, command, grant, held, message)
    }
    pub fn scope_denied(command: &str, grant: &str, held: Vec<Grant>) -> Self {
        let message = format!(
            "This agent holds `{grant}` only as {}; the target of `{command}` is outside that scope. {ASK}",
            held_text(&held)
        );
        Self::new(RefusalCode::ScopeDenied, command, grant, held, message)
    }
    pub fn owner_only(command: &str, grant: &str) -> Self {
        let message =
            format!("`{command}` is an owner-only command and can never be granted to an agent.");
        Self::new(RefusalCode::OwnerOnly, command, grant, Vec::new(), message)
    }
    pub fn quota_exceeded(command: &str, grant: &str, held: Vec<Grant>, limit: &str) -> Self {
        let message = format!(
            "This agent reached the `{limit}` limit of its `{grant}` grant for `{command}`. {ASK}"
        );
        Self::new(RefusalCode::QuotaExceeded, command, grant, held, message)
    }
    /// A command-surface error that names no grant.
    pub fn error(code: RefusalCode, command: &str, message: &str) -> Self {
        Self {
            code,
            command: command.into(),
            grant: None,
            held: Vec::new(),
            message: message.into(),
            effect: "not_applied".into(),
            request_id: None,
        }
    }
    pub fn with_request(mut self, request_id: Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }
    /// The §6.8 body.
    pub fn body(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("refusal encodes")
    }
}
