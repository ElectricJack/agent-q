//! Bounded notification delivery plans. Persistence and authorization belong to
//! the adapter; a pure state transition is not durable dispatch permission.
pub mod discord;
use aq_lifecycle_api::{Digest, Scope, Validate};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("invalid notification contract")]
    Invalid,
    #[error("notification scope or route changed")]
    ScopeChanged,
    #[error("notification state conflicts")]
    Conflict,
    #[error("notification is not due or has expired")]
    NotDue,
    #[error("notification retry budget exhausted")]
    Exhausted,
}
fn require(ok: bool) -> Result<()> {
    if ok { Ok(()) } else { Err(Error::Invalid) }
}
fn positive(v: u64) -> bool {
    v > 0 && v <= i64::MAX as u64
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub message_id: Uuid,
    pub revision: u64,
    pub scope: Scope,
    pub recipient_user_id: Uuid,
    /// Deliberately supplied external summary, never a transcript export.
    pub summary: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}
impl Message {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate().map_err(|_| Error::Invalid)?;
        require(
            !self.message_id.is_nil()
                && !self.recipient_user_id.is_nil()
                && positive(self.revision)
                && positive(self.created_at_ms)
                && positive(self.expires_at_ms)
                && self.expires_at_ms > self.created_at_ms
                && self.expires_at_ms - self.created_at_ms <= 604_800_000
                && !self.summary.trim().is_empty()
                && self.summary.len() <= 4096
                && self.summary.encode_utf16().count() <= 1600
                && !self
                    .summary
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\t'),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub max_elapsed_ms: u64,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    /// New, explicitly authorized retry intents may accept duplicate delivery.
    pub allow_duplicate_risk: bool,
}
impl RetryPolicy {
    pub fn validate(&self) -> Result<()> {
        require(
            (1..=8).contains(&self.max_attempts)
                && (1000..=86_400_000).contains(&self.max_elapsed_ms)
                && (250..=60_000).contains(&self.initial_delay_ms)
                && self.max_delay_ms >= self.initial_delay_ms
                && self.max_delay_ms <= 3_600_000,
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub route_id: Uuid,
    pub revision: u64,
    pub scope: Scope,
    pub recipient_user_id: Uuid,
    /// Logical protected configuration key; no URL or token.
    pub endpoint_key: String,
    pub endpoint_revision: u64,
    pub enabled: bool,
    pub retry: RetryPolicy,
}
impl Route {
    pub fn validate(&self) -> Result<()> {
        self.scope.validate().map_err(|_| Error::Invalid)?;
        self.retry.validate()?;
        require(
            !self.route_id.is_nil()
                && !self.recipient_user_id.is_nil()
                && positive(self.revision)
                && positive(self.endpoint_revision)
                && !self.endpoint_key.is_empty()
                && self.endpoint_key.len() <= 128
                && self
                    .endpoint_key
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c)),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub content: String,
}
impl Payload {
    pub fn for_message(message: &Message, public_origin: &str) -> Result<Self> {
        message.validate()?;
        let u = reqwest::Url::parse(public_origin).map_err(|_| Error::Invalid)?;
        require(
            u.scheme() == "https"
                && u.host_str().is_some()
                && u.username().is_empty()
                && u.password().is_none()
                && u.query().is_none()
                && u.fragment().is_none()
                && u.path() == "/"
                && public_origin.len() <= 256,
        )?;
        let content = format!(
            "{}\n\n{}/api/notifications/{}",
            message.summary,
            public_origin.trim_end_matches('/'),
            message.message_id
        );
        let result = Self { content };
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        require(
            !self.content.is_empty()
                && self.content.len() <= 8192
                && self.content.encode_utf16().count() <= 2000,
        )
    }
    pub fn wire(&self) -> Result<serde_json::Value> {
        self.validate()?;
        Ok(
            serde_json::json!({"content":self.content,"allowed_mentions":{"parse":[],"users":[],"roles":[],"replied_user":false},"flags":4,"tts":false}),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Status {
    Queued,
    Dispatching {
        attempt_id: Uuid,
        started_at_ms: u64,
    },
    AcceptedByTransport {
        attempt_id: Uuid,
        message_id: String,
        accepted_at_ms: u64,
    },
    DeliveryUnknown {
        attempt_id: Uuid,
        since_ms: u64,
    },
    RetryScheduled {
        next_attempt_ms: u64,
    },
    PermanentlyFailed {
        reason: FailureReason,
    },
    Suppressed {
        reason: SuppressionReason,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureReason {
    InvalidPayload,
    DestinationUnavailable,
    RetryExhausted,
    ProviderDelayTooLong,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuppressionReason {
    ScopeRevoked,
    RouteChanged,
    MessageExpired,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub delivery_id: Uuid,
    pub message: Message,
    pub route: Route,
    pub payload: Payload,
    pub immutable_digest: Digest,
    pub attempts: u32,
    pub first_attempt_ms: Option<u64>,
    pub last_transition_ms: u64,
    pub duplicate_risk_retries: u32,
    pub status: Status,
}
/// Trusted current facts, acquired under the owning service's route/scope lock.
/// Neither a serialized request nor a transport response supplies these values.
pub struct DispatchAuthority<'a> {
    pub route: &'a Route,
    pub recipient_can_view: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportOutcome {
    Accepted { message_id: String },
    RateLimited { retry_after_ms: u64, global: bool },
    Permanent(FailureReason),
    Unknown,
}
pub fn snowflake(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 20
        && value.bytes().all(|b| b.is_ascii_digit())
        && value.parse::<u64>().is_ok_and(|v| v > 0)
}
impl Delivery {
    pub fn queued(
        delivery_id: Uuid,
        message: Message,
        route: Route,
        payload: Payload,
    ) -> Result<Self> {
        message.validate()?;
        route.validate()?;
        payload.validate()?;
        require(!delivery_id.is_nil())?;
        if message.scope != route.scope || message.recipient_user_id != route.recipient_user_id {
            return Err(Error::ScopeChanged);
        }
        let immutable_digest = immutable_digest(delivery_id, &message, &route, &payload)?;
        let result = Self {
            delivery_id,
            last_transition_ms: message.created_at_ms,
            message,
            route,
            payload,
            immutable_digest,
            attempts: 0,
            first_attempt_ms: None,
            duplicate_risk_retries: 0,
            status: Status::Queued,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn validate(&self) -> Result<()> {
        self.message.validate()?;
        self.route.validate()?;
        self.payload.validate()?;
        require(
            !self.delivery_id.is_nil()
                && self.message.scope == self.route.scope
                && self.message.recipient_user_id == self.route.recipient_user_id
                && self.immutable_digest
                    == immutable_digest(
                        self.delivery_id,
                        &self.message,
                        &self.route,
                        &self.payload,
                    )?
                && self.attempts <= self.route.retry.max_attempts
                && self.duplicate_risk_retries <= self.attempts
                && (self.attempts == 0) == self.first_attempt_ms.is_none()
                && self.last_transition_ms >= self.message.created_at_ms
                && positive(self.last_transition_ms),
        )?;
        if let Some(first) = self.first_attempt_ms {
            require(first >= self.message.created_at_ms && first <= self.last_transition_ms)?;
        }
        match &self.status {
            Status::Dispatching {
                attempt_id,
                started_at_ms,
            } => require(
                self.attempts > 0
                    && !attempt_id.is_nil()
                    && *started_at_ms == self.last_transition_ms,
            ),
            Status::DeliveryUnknown {
                attempt_id,
                since_ms,
            } => require(
                self.attempts > 0
                    && !attempt_id.is_nil()
                    && *since_ms <= self.last_transition_ms
                    && self
                        .first_attempt_ms
                        .is_some_and(|first| *since_ms >= first),
            ),
            Status::AcceptedByTransport {
                attempt_id,
                message_id,
                accepted_at_ms,
            } => require(
                self.attempts > 0
                    && !attempt_id.is_nil()
                    && snowflake(message_id)
                    && *accepted_at_ms == self.last_transition_ms,
            ),
            Status::RetryScheduled { next_attempt_ms } => require(
                self.attempts > 0
                    && positive(*next_attempt_ms)
                    && *next_attempt_ms >= self.last_transition_ms,
            ),
            Status::Queued => require(self.attempts == 0),
            _ => Ok(()),
        }
    }
    fn advance(&self, now: u64) -> Result<Self> {
        self.validate()?;
        require(positive(now) && now >= self.last_transition_ms)?;
        Ok(self.clone())
    }
    fn budget(&self, now: u64) -> Result<()> {
        if self.attempts >= self.route.retry.max_attempts
            || self
                .first_attempt_ms
                .is_some_and(|first| now.saturating_sub(first) >= self.route.retry.max_elapsed_ms)
        {
            return Err(Error::Exhausted);
        }
        Ok(())
    }
    /// Persist the returned state before sending. The caller must CAS the old
    /// revision and may dispatch only its exact committed attempt ID once.
    pub fn begin(
        &self,
        authority: DispatchAuthority<'_>,
        attempt_id: Uuid,
        now: u64,
    ) -> Result<Self> {
        let mut next = self.advance(now)?;
        authority.route.validate()?;
        require(!attempt_id.is_nil())?;
        if !authority.recipient_can_view
            || !authority.route.enabled
            || authority.route != &self.route
        {
            return Err(Error::ScopeChanged);
        }
        match self.status {
            Status::Queued => {}
            Status::RetryScheduled { next_attempt_ms } if now >= next_attempt_ms => {}
            _ => return Err(Error::NotDue),
        }
        if now >= self.message.expires_at_ms {
            return Err(Error::NotDue);
        }
        self.budget(now)?;
        next.attempts += 1;
        next.first_attempt_ms.get_or_insert(now);
        next.last_transition_ms = now;
        next.status = Status::Dispatching {
            attempt_id,
            started_at_ms: now,
        };
        next.validate()?;
        Ok(next)
    }
    pub fn observe(
        &self,
        attempt_id: Uuid,
        outcome: TransportOutcome,
        now: u64,
        jitter: u64,
    ) -> Result<Self> {
        let mut next = self.advance(now)?;
        if !matches!(self.status,Status::Dispatching{attempt_id:id,..} if id==attempt_id) {
            return Err(Error::Conflict);
        }
        next.last_transition_ms = now;
        next.status = match outcome {
            TransportOutcome::Accepted { message_id } => {
                require(snowflake(&message_id))?;
                Status::AcceptedByTransport {
                    attempt_id,
                    message_id,
                    accepted_at_ms: now,
                }
            }
            TransportOutcome::Unknown => Status::DeliveryUnknown {
                attempt_id,
                since_ms: now,
            },
            TransportOutcome::Permanent(reason) => Status::PermanentlyFailed { reason },
            TransportOutcome::RateLimited { retry_after_ms, .. } => {
                if retry_after_ms > 86_400_000 {
                    Status::PermanentlyFailed {
                        reason: FailureReason::ProviderDelayTooLong,
                    }
                } else if next.budget(now).is_err() {
                    Status::PermanentlyFailed {
                        reason: FailureReason::RetryExhausted,
                    }
                } else {
                    let ceiling = self
                        .route
                        .retry
                        .initial_delay_ms
                        .saturating_mul(1u64 << self.attempts.min(16))
                        .min(self.route.retry.max_delay_ms);
                    let delay =
                        (ceiling / 2 + jitter % (ceiling - ceiling / 2 + 1)).max(retry_after_ms);
                    let due = now.checked_add(delay).ok_or(Error::Invalid)?;
                    if due >= self.message.expires_at_ms
                        || due.saturating_sub(self.first_attempt_ms.unwrap())
                            >= self.route.retry.max_elapsed_ms
                    {
                        Status::PermanentlyFailed {
                            reason: FailureReason::RetryExhausted,
                        }
                    } else {
                        Status::RetryScheduled {
                            next_attempt_ms: due,
                        }
                    }
                }
            }
        };
        next.validate()?;
        Ok(next)
    }
    /// Recovery never repeats an in-flight HTTP send. Any stale dispatcher is
    /// fenced by the owner transaction before this transition is committed.
    pub fn recover(&self, now: u64) -> Result<Self> {
        if let Status::Dispatching { attempt_id, .. } = self.status {
            self.observe(attempt_id, TransportOutcome::Unknown, now, 0)
        } else {
            self.advance(now)
        }
    }
    /// Caller durably authorizes a NEW retry intent with its own idempotency key.
    /// This does not assert the previous attempt had no external effect.
    pub fn retry_unknown(&self, now: u64) -> Result<Self> {
        let mut next = self.advance(now)?;
        if !matches!(self.status, Status::DeliveryUnknown { .. })
            || !self.route.retry.allow_duplicate_risk
        {
            return Err(Error::Conflict);
        }
        self.budget(now)?;
        if now >= self.message.expires_at_ms {
            return Err(Error::NotDue);
        }
        next.duplicate_risk_retries += 1;
        next.last_transition_ms = now;
        next.status = Status::RetryScheduled {
            next_attempt_ms: now,
        };
        next.validate()?;
        Ok(next)
    }
}

fn immutable_digest(
    id: Uuid,
    message: &Message,
    route: &Route,
    payload: &Payload,
) -> Result<Digest> {
    let mut value =
        serde_json::to_value(("aq.notification-delivery/1", id, message, route, payload))
            .map_err(|_| Error::Invalid)?;
    value.sort_all_objects();
    Ok(Digest::of(
        &serde_json::to_vec(&value).map_err(|_| Error::Invalid)?,
    ))
}

// Central human API requests; worker credentials cannot use these endpoints.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteRequest {
    pub operation_id: Uuid,
    pub expected_revision: u64,
    pub route: Route,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendRequest {
    pub operation_id: Uuid,
    pub message_id: Uuid,
    pub revision: u64,
    pub scope: Scope,
    pub recipient_user_id: Uuid,
    pub summary: String,
    pub ttl_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryRequest {
    pub operation_id: Uuid,
    pub delivery_id: Uuid,
    pub expected_revision: u64,
    pub accept_duplicate_risk: bool,
}
