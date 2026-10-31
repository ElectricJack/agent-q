//! AQ-owned future API values and bounded fake peers, not installed routes,
//! authentication, a durable wait registry, a source verifier or a notification sender.
pub use aq_lifecycle_protocol as worker_lifecycle;
pub use aq_lifecycle_protocol::{
    ArtifactRef, Binding, Capability, Checkpoint, ContinuationAttachment, ContinuationRef,
    ContractError, Cost, Counter, Counters, Digest, EventRef, OutcomeRef, SessionIdentity,
    SummaryText, Text, Validate, WaitRef, artifact, canonical_bytes, canonical_digest, ensure,
    unique_ids, uuid,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;
mod inspection;
mod peer;
mod waits;
pub use inspection::*;
pub use peer::*;
pub use waits::*;
pub const API_VERSION: u32 = 1;
pub const MAX_REQUEST_BYTES: usize = 65536;
pub const MAX_VIEW_BYTES: usize = 262144;
pub const MAX_MEMBERS: usize = 32;
pub const MAX_PAGE: usize = 64;
pub const WAIT_CAPABILITY: &str = "aq.lifecycle-api.wait";
pub const INSPECT_CAPABILITY: &str = "aq.lifecycle-api.inspect";
pub const NOTIFICATION_CAPABILITY: &str = "aq.lifecycle-api.notification-inspect";
pub const OPERATOR_CAPABILITY: &str = "aq.lifecycle-api.operator";
pub const ACK_CAPABILITY: &str = "aq.lifecycle-api.continuation-ack";
macro_rules! record {($name:ident {$($field:ident:$ty:ty),* $(,)?})=>{#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize,JsonSchema)] #[serde(deny_unknown_fields)] pub struct $name {$(pub $field:$ty),*}}}
pub(crate) use record;
record!(Scope {
    project_id: Uuid,
    workspace_id: Uuid
});
impl Validate for Scope {
    fn validate(&self) -> Result<(), ContractError> {
        uuid(self.project_id)?;
        uuid(self.workspace_id)
    }
}
record!(AckRequest {
    admission_id: Uuid,
    continuation: ContinuationRef,
    successor: Binding,
    attachment_digest: Digest,
    outcome: OutcomeRef
});
impl Validate for AckRequest {
    fn validate(&self) -> Result<(), ContractError> {
        uuid(self.admission_id)?;
        self.continuation.validate()?;
        self.successor.validate()?;
        self.outcome.validate()?;
        ensure(
            self.outcome.wait == self.continuation.wait,
            "ack outcome binding",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperatorAction {
    CancelWait { wait: WaitRef },
    ManualResume { wait: WaitRef, reason: Text },
    HoldWait { wait: WaitRef, reason: Text },
    StopSession { binding: Binding },
    QuarantineSession { binding: Binding, reason: Text },
}
record!(OperatorRequest {
    scope: Scope,
    expected_revision: u64,
    action: OperatorAction
});
impl Validate for OperatorRequest {
    fn validate(&self) -> Result<(), ContractError> {
        self.scope.validate()?;
        ensure(self.expected_revision > 0, "operator expected revision")?;
        match &self.action {
            OperatorAction::CancelWait { wait } => wait.validate(),
            OperatorAction::ManualResume { wait, reason }
            | OperatorAction::HoldWait { wait, reason } => {
                wait.validate()?;
                reason.validate()
            }
            OperatorAction::StopSession { binding } => {
                binding.validate()?;
                ensure(
                    binding.project_id == self.scope.project_id,
                    "operator project",
                )
            }
            OperatorAction::QuarantineSession { binding, reason } => {
                binding.validate()?;
                reason.validate()?;
                ensure(
                    binding.project_id == self.scope.project_id,
                    "operator project",
                )
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "body",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Command {
    RegisterWait(WaitRegistration),
    Checkpoint(Checkpoint),
    ContinuationAck(AckRequest),
    Inspect(InspectRequest),
    Operator(OperatorRequest),
}
impl Validate for Command {
    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::RegisterWait(v) => v.validate(),
            Self::Checkpoint(v) => v.validate(),
            Self::ContinuationAck(v) => v.validate(),
            Self::Inspect(v) => v.validate(),
            Self::Operator(v) => v.validate(),
        }
    }
}
impl Command {
    pub fn capability(&self) -> &'static str {
        match self {
            Self::RegisterWait(_) | Self::Checkpoint(_) => WAIT_CAPABILITY,
            Self::ContinuationAck(_) => ACK_CAPABILITY,
            Self::Inspect(InspectRequest {
                target: InspectTarget::Notification { .. },
                ..
            }) => NOTIFICATION_CAPABILITY,
            Self::Inspect(_) => INSPECT_CAPABILITY,
            Self::Operator(_) => OPERATOR_CAPABILITY,
        }
    }
}
record!(RequestBody {
    api_version: u32,
    operation_id: Uuid,
    required_capability: Capability,
    command: Command
});
impl Validate for RequestBody {
    fn validate(&self) -> Result<(), ContractError> {
        ensure(self.api_version == API_VERSION, "API version")?;
        uuid(self.operation_id)?;
        self.command.validate()?;
        self.required_capability.validate()?;
        if self.required_capability.id.as_str() != self.command.capability()
            || self.required_capability.version != 1
        {
            return Err(ContractError::Unsupported);
        }
        if let Command::RegisterWait(wait) = &self.command {
            wait.reject_self(Some(self.operation_id), None)?;
        }
        Ok(())
    }
}
record!(Request {
    body: RequestBody,
    body_digest: Digest
});
impl Request {
    pub fn seal(operation_id: Uuid, command: Command) -> Result<Self, ContractError> {
        let body = RequestBody {
            api_version: API_VERSION,
            operation_id,
            required_capability: Capability {
                id: Text::new(command.capability())?,
                version: 1,
            },
            command,
        };
        let value = Self {
            body_digest: canonical_digest(&body)?,
            body,
        };
        value.validate()?;
        Ok(value)
    }
}
impl Validate for Request {
    fn validate(&self) -> Result<(), ContractError> {
        self.body.validate()?;
        ensure(
            canonical_digest(&self.body)? == self.body_digest,
            "request digest",
        )?;
        ensure(
            serde_json::to_vec(self)
                .map_err(|e| ContractError::Json(e.to_string()))?
                .len()
                <= MAX_REQUEST_BYTES,
            "request wire bound",
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    InvalidRequest,
    UnsupportedCapability,
    ScopeDenied,
    SelfWait,
    KnownCycle,
    ChangedPayload,
    StaleGeneration,
    SourceUnavailable,
    Overloaded,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    NotApplied,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "body",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ResponseBody {
    WaitRegistered(Box<RegisteredWait>),
    WaitOutcome(WaitOutcome),
    CheckpointRecorded {
        continuation: ContinuationRef,
        claim_digest: Digest,
    },
    ContinuationAcknowledged {
        admission_id: Uuid,
        continuation: ContinuationRef,
        acknowledgment_receipt: ArtifactRef,
    },
    Inspection(InspectionPage),
    OperatorRequested {
        receipt: ArtifactRef,
    },
    Refused {
        code: RefusalCode,
        effect: Effect,
    },
}
impl Validate for ResponseBody {
    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::WaitRegistered(v) => v.validate(),
            Self::WaitOutcome(v) => v.validate(),
            Self::CheckpointRecorded { continuation, .. } => continuation.validate(),
            Self::ContinuationAcknowledged {
                admission_id,
                continuation,
                acknowledgment_receipt,
            } => {
                uuid(*admission_id)?;
                continuation.validate()?;
                artifact(acknowledgment_receipt)
            }
            Self::Inspection(v) => v.validate(),
            Self::OperatorRequested { receipt } => artifact(receipt),
            Self::Refused { .. } => Ok(()),
        }
    }
}
record!(Response {
    api_version: u32,
    operation_id: Uuid,
    request_digest: Digest,
    body: ResponseBody
});
impl Response {
    pub fn validate_against(&self, request: &Request) -> Result<(), ContractError> {
        self.validate()?;
        request.validate()?;
        if self.operation_id != request.body.operation_id
            || self.request_digest != request.body_digest
        {
            return Err(ContractError::Conflict);
        }
        match (&request.body.command, &self.body) {
            (Command::RegisterWait(wanted), ResponseBody::WaitRegistered(got)) => {
                ensure(
                    got.predicate == wanted.predicate
                        && got.scope == wanted.scope
                        && got.binding == wanted.binding
                        && got.wait == wanted.wait_ref()
                        && got.continuation.generation == wanted.continuation_generation,
                    "registration response binding",
                )?;
                let members = wanted.predicate.members();
                ensure(
                    got.boundaries.len() == members.len(),
                    "source boundary completeness",
                )?;
                for (member, boundary) in members.iter().zip(&got.boundaries) {
                    ensure(
                        boundary.member_id == member.member_id
                            && boundary.owner == member.condition.owner()
                            && boundary.condition_digest == canonical_digest(&member.condition)?,
                        "source boundary identity",
                    )?;
                }
            }
            (Command::RegisterWait(wanted), ResponseBody::WaitOutcome(got)) => {
                ensure(
                    got.outcome.wait == wanted.wait_ref(),
                    "already resolved response binding",
                )?;
                got.validate_for(&wanted.predicate)?;
            }
            (
                Command::ContinuationAck(wanted),
                ResponseBody::ContinuationAcknowledged {
                    admission_id,
                    continuation,
                    ..
                },
            ) => ensure(
                *admission_id == wanted.admission_id && *continuation == wanted.continuation,
                "ack response binding",
            )?,
            (
                Command::Checkpoint(wanted),
                ResponseBody::CheckpointRecorded {
                    continuation,
                    claim_digest,
                },
            ) => ensure(
                *continuation == wanted.continuation
                    && *claim_digest == canonical_digest(&wanted.claim)?,
                "checkpoint response binding",
            )?,
            (Command::Inspect(wanted), ResponseBody::Inspection(got)) => {
                ensure(
                    got.scope == wanted.scope && got.items.len() <= wanted.page.limit as usize,
                    "inspection response scope/page",
                )?;
                for item in &got.items {
                    match (&wanted.target, item) {
                        (InspectTarget::Session { binding }, InspectionItem::Session(view)) => {
                            ensure(view.binding == *binding, "session projection binding")?
                        }
                        (InspectTarget::Wait { wait }, InspectionItem::Wait(view)) => {
                            ensure(view.registration.wait == *wait, "wait projection binding")?
                        }
                        (
                            InspectTarget::Notification { message_id },
                            InspectionItem::Notification(view),
                        ) => ensure(
                            view.message.message_id == *message_id,
                            "notification projection binding",
                        )?,
                        _ => return Err(ContractError::Invalid("projection type mismatch")),
                    }
                }
            }
            (Command::Operator(_), ResponseBody::OperatorRequested { .. })
            | (_, ResponseBody::Refused { .. }) => {}
            _ => return Err(ContractError::Invalid("response class mismatch")),
        }
        Ok(())
    }
}
impl Validate for Response {
    fn validate(&self) -> Result<(), ContractError> {
        ensure(self.api_version == API_VERSION, "API version")?;
        uuid(self.operation_id)?;
        self.body.validate()?;
        ensure(
            serde_json::to_vec(self)
                .map_err(|e| ContractError::Json(e.to_string()))?
                .len()
                <= MAX_VIEW_BYTES,
            "response wire bound",
        )
    }
}
pub fn decode_request(bytes: &[u8]) -> Result<Request, ContractError> {
    ensure(bytes.len() <= MAX_REQUEST_BYTES, "request byte bound")?;
    let value: Request =
        serde_json::from_slice(bytes).map_err(|e| ContractError::Json(e.to_string()))?;
    value.validate()?;
    Ok(value)
}
pub fn decode_response(bytes: &[u8]) -> Result<Response, ContractError> {
    ensure(bytes.len() <= MAX_VIEW_BYTES, "response byte bound")?;
    let value: Response =
        serde_json::from_slice(bytes).map_err(|e| ContractError::Json(e.to_string()))?;
    value.validate()?;
    Ok(value)
}
pub fn negotiate_api(
    required: &Capability,
    advertised: &[Capability],
) -> Result<(), ContractError> {
    ensure(advertised.len() <= 16, "API capability count")?;
    let mut names = BTreeSet::new();
    for c in advertised {
        c.validate()?;
        ensure(names.insert(c.id.clone()), "duplicate API capability")?;
    }
    if required.version != 1
        || !matches!(
            required.id.as_str(),
            WAIT_CAPABILITY
                | INSPECT_CAPABILITY
                | NOTIFICATION_CAPABILITY
                | OPERATOR_CAPABILITY
                | ACK_CAPABILITY
        )
        || !advertised.contains(required)
    {
        return Err(ContractError::Unsupported);
    }
    Ok(())
}
fn bounded_schema<T: JsonSchema>(maximum: usize) -> serde_json::Value {
    fn annotate(value: &mut serde_json::Value) {
        if let Some(object) = value.as_object_mut() {
            match object.get("format").and_then(|v| v.as_str()) {
                Some("uint64") => {
                    object.insert("maximum".into(), serde_json::json!(u64::MAX));
                }
                Some("uint32") => {
                    object.insert("maximum".into(), serde_json::json!(u32::MAX));
                }
                _ => {}
            }
            if let Some(props) = object.get_mut("properties").and_then(|v| v.as_object_mut()) {
                for (name, shape) in props {
                    if name == "api_version" {
                        shape["const"] = serde_json::json!(API_VERSION);
                    }
                    if matches!(
                        name.as_str(),
                        "members" | "outcomes" | "boundaries" | "pending_operation_ids" | "reasons"
                    ) {
                        shape["maxItems"] = serde_json::json!(MAX_MEMBERS);
                    }
                    if name == "items" {
                        shape["maxItems"] = serde_json::json!(MAX_PAGE);
                    }
                    if name == "limit" {
                        shape["minimum"] = serde_json::json!(1);
                        shape["maximum"] = serde_json::json!(MAX_PAGE);
                    }
                    if matches!(
                        name.as_str(),
                        "generation"
                            | "continuation_generation"
                            | "wait_generation"
                            | "expected_revision"
                            | "revision"
                            | "message_revision"
                            | "question_revision"
                    ) {
                        shape["minimum"] = serde_json::json!(1);
                    }
                }
            }
            for child in object.values_mut() {
                annotate(child);
            }
        } else if let Some(array) = value.as_array_mut() {
            for child in array {
                annotate(child);
            }
        }
    }
    let mut schema = serde_json::to_value(schemars::schema_for!(T)).expect("schema serialization");
    annotate(&mut schema);
    schema["x-max-wire-bytes"] = serde_json::json!(maximum);
    schema
}
pub fn request_schema() -> serde_json::Value {
    bounded_schema::<Request>(MAX_REQUEST_BYTES)
}
pub fn response_schema() -> serde_json::Value {
    bounded_schema::<Response>(MAX_VIEW_BYTES)
}
