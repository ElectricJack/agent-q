//! Pure bounded fake service. Its caller/source contexts are fixture inputs, not
//! runtime authentication, source verification or a lost-wakeup transaction.
use crate::*;
use std::collections::BTreeMap;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActorRole {
    SessionAgent,
    Operator,
    NodeRecovery,
}
#[derive(Clone, Debug)]
pub struct CallerContext {
    pub role: ActorRole,
    pub scope: Scope,
    pub binding: Option<Binding>,
    pub current_operation_id: Option<Uuid>,
    pub current_run_id: Option<Uuid>,
}
struct WaitRecord {
    request: WaitRegistration,
    registration: RegisteredWait,
    outcome: Option<WaitOutcome>,
}
pub struct ConformanceApi {
    context: CallerContext,
    advertised: Vec<Capability>,
    seen: BTreeMap<Uuid, (Digest, Response)>,
    waits: BTreeMap<Uuid, WaitRecord>,
    attachment: Option<ContinuationAttachment>,
    acknowledgment: Option<ArtifactRef>,
}
fn reference(id: &str) -> ArtifactRef {
    ArtifactRef {
        id: Text::new(id).expect("fixture ref"),
        version: Text::new("1").expect("fixture version"),
        digest: Digest::of(id.as_bytes()),
    }
}
impl ConformanceApi {
    pub fn new(context: CallerContext, advertised: Vec<Capability>) -> Result<Self, ContractError> {
        context.scope.validate()?;
        ensure(advertised.len() <= 16, "API advertisement bound")?;
        if let Some(binding) = &context.binding {
            binding.validate()?;
            ensure(
                binding.project_id == context.scope.project_id,
                "context project",
            )?;
        }
        ensure(
            context.role != ActorRole::SessionAgent || context.binding.is_some(),
            "agent context binding",
        )?;
        Ok(Self {
            context,
            advertised,
            seen: BTreeMap::new(),
            waits: BTreeMap::new(),
            attachment: None,
            acknowledgment: None,
        })
    }
    pub fn install_attachment(
        &mut self,
        value: ContinuationAttachment,
    ) -> Result<(), ContractError> {
        value.validate()?;
        ensure(
            self.context.binding.as_ref() == Some(&value.successor),
            "attachment caller binding",
        )?;
        ensure(self.attachment.is_none(), "one admitted continuation")?;
        self.attachment = Some(value);
        Ok(())
    }
    pub fn bootstrap_acknowledged(&self) -> bool {
        self.acknowledgment.is_some()
    }
    fn own(&self, binding: &Binding) -> Result<(), ContractError> {
        if self.context.role != ActorRole::SessionAgent {
            return Err(ContractError::Unsupported);
        }
        if self.context.binding.as_ref() != Some(binding) {
            return Err(ContractError::Stale);
        }
        Ok(())
    }
    pub fn receive(&mut self, request: &Request, now_ms: u64) -> Result<Response, ContractError> {
        request.validate()?;
        negotiate_api(&request.body.required_capability, &self.advertised)?;
        if self.context.role == ActorRole::NodeRecovery {
            return Err(ContractError::Unsupported);
        }
        if let Some((digest, response)) = self.seen.get(&request.body.operation_id) {
            return if digest == &request.body_digest {
                Ok(response.clone())
            } else {
                Err(ContractError::Conflict)
            };
        }
        ensure(self.seen.len() < 1024, "API receipt capacity")?;
        let body = match &request.body.command {
            Command::RegisterWait(value) => {
                self.own(&value.binding)?;
                if value.scope != self.context.scope {
                    return Err(ContractError::Stale);
                }
                value.reject_self(
                    self.context.current_operation_id,
                    self.context.current_run_id,
                )?;
                if let Some(old) = self.waits.get(&value.wait_id) {
                    if old.request != *value {
                        return Err(ContractError::Conflict);
                    }
                    ResponseBody::WaitRegistered(Box::new(old.registration.clone()))
                } else {
                    ensure(self.waits.len() < 256, "fake wait registry capacity")?;
                    ensure(
                        !self.waits.values().any(|old| {
                            old.request.binding.attempt_id == value.binding.attempt_id
                                && old.outcome.is_none()
                        }),
                        "one active suspension wait per attempt",
                    )?;
                    let registration = RegisteredWait {
                        scope: value.scope.clone(),
                        binding: value.binding.clone(),
                        wait: value.wait_ref(),
                        predicate: value.predicate.clone(),
                        continuation: ContinuationRef {
                            continuation_id: request.body.operation_id,
                            generation: value.continuation_generation,
                            wait: value.wait_ref(),
                        },
                        original_created_at_ms: now_ms,
                        central_hold_receipt: reference("fake-central-hold-only"),
                        boundaries: value
                            .predicate
                            .members()
                            .iter()
                            .map(|member| SourceBoundary {
                                member_id: member.member_id,
                                owner: member.condition.owner(),
                                condition_digest: canonical_digest(&member.condition).unwrap(),
                                committed_revision: Text::new("fixture-committed-revision")
                                    .unwrap(),
                                committed_replay_cursor: Text::new(
                                    "fixture-committed-replay-boundary",
                                )
                                .unwrap(),
                                subscription_receipt: reference(
                                    "fake-subscription-not-runtime-proof",
                                ),
                                snapshot_digest: Digest::of(b"fixture-source-snapshot"),
                            })
                            .collect(),
                        cycle_assessment: CycleAssessment::ExternalUnknown {
                            reason: Text::new(
                                "runtime graph/source cycle verification remains required",
                            )
                            .unwrap(),
                        },
                        execution: WaitExecutionState::PolicyPending,
                    };
                    registration.validate()?;
                    self.waits.insert(
                        value.wait_id,
                        WaitRecord {
                            request: value.clone(),
                            registration: registration.clone(),
                            outcome: None,
                        },
                    );
                    ResponseBody::WaitRegistered(Box::new(registration))
                }
            }
            Command::Checkpoint(value) => {
                self.own(&value.binding)?;
                let wait = self
                    .waits
                    .get(&value.continuation.wait.wait_id)
                    .ok_or(ContractError::Stale)?;
                ensure(
                    wait.registration.continuation == value.continuation,
                    "checkpoint continuation binding",
                )?;
                ResponseBody::CheckpointRecorded {
                    continuation: value.continuation.clone(),
                    claim_digest: canonical_digest(&value.claim)?,
                }
            }
            Command::ContinuationAck(value) => {
                self.own(&value.successor)?;
                let attachment = self.attachment.as_ref().ok_or(ContractError::Stale)?;
                if value.admission_id != attachment.admission_id
                    || value.continuation != attachment.continuation
                    || value.attachment_digest != canonical_digest(attachment)?
                    || value.outcome != attachment.outcome
                {
                    return Err(ContractError::Stale);
                }
                ensure(
                    self.acknowledgment.is_none(),
                    "continuation ack already recorded",
                )?;
                let receipt = reference("fake-authenticated-api-receipt-not-runtime-proof");
                self.acknowledgment = Some(receipt.clone());
                ResponseBody::ContinuationAcknowledged {
                    admission_id: value.admission_id,
                    continuation: value.continuation.clone(),
                    acknowledgment_receipt: receipt,
                }
            }
            Command::Inspect(value) => {
                if value.scope != self.context.scope {
                    return Err(ContractError::Stale);
                }
                if self.context.role == ActorRole::SessionAgent {
                    match &value.target {
                        InspectTarget::Session { binding } => self.own(binding)?,
                        InspectTarget::Wait { wait } => {
                            let row = self.waits.get(&wait.wait_id).ok_or(ContractError::Stale)?;
                            self.own(&row.registration.binding)?;
                            ensure(row.registration.wait == *wait, "inspection wait revision")?;
                        }
                        InspectTarget::Notification { .. } => {
                            return Err(ContractError::Unsupported);
                        }
                    }
                }
                let items = if let InspectTarget::Wait { wait } = &value.target {
                    let row = self.waits.get(&wait.wait_id).ok_or(ContractError::Stale)?;
                    vec![InspectionItem::Wait(Box::new(WaitView {
                        registration: row.registration.clone(),
                        outcome: row.outcome.clone(),
                        continuation: ContinuationState::PendingPolicy,
                        parking_report: None,
                    }))]
                } else {
                    Vec::new()
                };
                ResponseBody::Inspection(InspectionPage {
                    scope: value.scope.clone(),
                    items,
                    next_cursor: None,
                    complete_for_authorized_scope: false,
                    reasons: vec![
                        Text::new("fake peer has no runtime projection/source scan").unwrap(),
                    ],
                })
            }
            Command::Operator(value) => {
                if self.context.role != ActorRole::Operator {
                    return Err(ContractError::Unsupported);
                }
                if value.scope != self.context.scope {
                    return Err(ContractError::Stale);
                }
                ResponseBody::OperatorRequested {
                    receipt: reference("fake-operator-request-not-executed"),
                }
            }
        };
        let response = Response {
            api_version: API_VERSION,
            operation_id: request.body.operation_id,
            request_digest: request.body_digest.clone(),
            body,
        };
        response.validate_against(request)?;
        self.seen.insert(
            request.body.operation_id,
            (request.body_digest.clone(), response.clone()),
        );
        Ok(response)
    }
    /// Supply already verified owner observations in a fake test. There is no wire
    /// command that lets an agent submit these verdicts or a transport ack as truth.
    pub fn resolve_from_fixture(
        &mut self,
        wait_id: Uuid,
        outcomes: Vec<MemberOutcome>,
        outcome_id: Uuid,
        now_ms: u64,
    ) -> Result<WaitOutcome, ContractError> {
        let row = self.waits.get_mut(&wait_id).ok_or(ContractError::Stale)?;
        ensure(row.outcome.is_none(), "immutable outcome")?;
        let members = row.request.predicate.members();
        ensure(
            outcomes.len() == members.len(),
            "all source members reobserved",
        )?;
        for (member, result) in members.iter().zip(&outcomes) {
            result.validate()?;
            ensure(result.member_id == member.member_id, "member outcome order")?;
            if let Some(evidence) = &result.evidence {
                ensure(
                    evidence.owner == member.condition.owner()
                        && evidence.condition_digest == canonical_digest(&member.condition)?,
                    "source owner/condition mismatch",
                )?;
            }
        }
        let satisfied: Vec<Uuid> = outcomes
            .iter()
            .filter(|v| v.state == MemberState::Satisfied)
            .map(|v| v.member_id)
            .collect();
        let any = matches!(row.request.predicate, Predicate::Any { .. });
        let kind = if (any && !satisfied.is_empty()) || (!any && satisfied.len() == members.len()) {
            WaitOutcomeKind::Satisfied
        } else if any && outcomes.iter().any(|v| v.state == MemberState::Pending) {
            return Err(ContractError::Invalid("wait remains pending"));
        } else if outcomes
            .iter()
            .any(|v| v.state == MemberState::SourceFailed)
        {
            WaitOutcomeKind::SourceFailed
        } else if outcomes
            .iter()
            .any(|v| v.state == MemberState::SourceUnavailable)
        {
            WaitOutcomeKind::SourceUnavailable
        } else {
            return Err(ContractError::Invalid("wait remains pending"));
        };
        let outcome = WaitOutcome {
            outcome: OutcomeRef {
                outcome_id,
                wait: row.registration.wait.clone(),
                revision: 1,
                digest: Digest::of(
                    &serde_json::to_vec(&outcomes)
                        .map_err(|e| ContractError::Json(e.to_string()))?,
                ),
            },
            kind,
            selected_member_id: if any {
                satisfied.first().copied()
            } else {
                None
            },
            members: outcomes,
            recorded_at_ms: now_ms,
            unresolved_predicate: None,
        };
        outcome.validate_for(&row.request.predicate)?;
        row.outcome = Some(outcome.clone());
        Ok(outcome)
    }
}
