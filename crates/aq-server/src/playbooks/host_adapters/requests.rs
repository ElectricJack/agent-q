//! D6 asynchronous host adapters for catalog profile 6 (AQ host catalog
//! 2.0.0): judgment.request, check.request and their operation.get
//! reconciliation.
//!
//! Both commands record one durable intent (`playbook_effects` row `accepted`
//! plus one `playbook_host_operations` row `running`) before any provider
//! request or sandbox start, return the operation handle, and complete exactly
//! once by a sealed trusted event (aq.judgment.completed / aq.check.receipted)
//! that commits with the retained completion. A re-dispatch or operation.get
//! only observes the original operation; nothing here ever starts a second
//! check writer or a second provider request for one intent.
//!
//! The sandbox executor and the judgment provider are injected ports. The
//! host never fabricates a receipt: every passed/failed disposition comes from
//! an executor-reported exit, and an absent port is a typed refusal.
use super::*;
use serde::{Deserialize, Serialize};

/// Check execution profile every D6 check attempt runs under.
pub const CHECK_EXECUTION_PROFILE: &str = "aq.check.sandbox/v1";
/// Executions retained per subject (id + revision) and check id.
pub const MAX_CHECK_EXECUTIONS: i64 = 8;
/// Judgment and check operations retained per subject lineage (subject id).
pub const MAX_LINEAGE_OPERATIONS: i64 = 256;
/// Judgment and check operations retained per namespace generation.
pub const MAX_RETAINED_OPERATIONS: i64 = 10_000;

/// Immutable artifact reference (id, version, digest).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPin {
    pub id: String,
    pub version: String,
    pub digest: String,
}

/// One required check of a pinned expected outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedCheck {
    pub id: String,
    pub validator: ArtifactPin,
}

/// The expected outcome pinned for a subject: its revision and its
/// required checks (at most 32).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinnedOutcome {
    pub revision: i64,
    pub required_checks: Vec<PinnedCheck>,
}

/// One sandbox check attempt. Carries no shell, URL or path: the executor
/// resolves the validator and source from their immutable identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckAttempt {
    /// Idempotency key: one attempt per operation, ever.
    pub operation: Uuid,
    pub workspace: Uuid,
    pub project: Uuid,
    pub subject_id: String,
    pub subject_revision: i64,
    pub check_id: String,
    pub validator: ArtifactPin,
    pub source_format: String,
    pub source_hex: String,
    pub execution_profile: &'static str,
}

/// How a sandbox check attempt stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckStop {
    Exited(i32),
    Signalled,
    TimedOut,
    Cancelled,
    Lost,
}

/// Executor-reported evidence for a stopped attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckRun {
    pub stop: CheckStop,
    /// False when any declared input was missing; such a run is never passed.
    pub inputs_complete: bool,
    pub executor_version: String,
    pub inputs_digest: String,
    pub result_artifacts: Vec<ArtifactPin>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckObservation {
    Running,
    Finished(CheckRun),
    /// The executor holds no attempt for this operation.
    NotFound,
}

/// Sandbox check executor port (C2/B5 backend in production).
#[async_trait::async_trait]
pub trait CheckExecutor: Send + Sync {
    /// The expected outcome pinned for `subject_id`, if any.
    async fn pinned(
        &self,
        workspace: Uuid,
        project: Uuid,
        subject_id: &str,
    ) -> Result<Option<PinnedOutcome>>;
    /// Starts the attempt; idempotent by `attempt.operation`. An error is
    /// ambiguous: the attempt may or may not exist.
    async fn start(&self, attempt: &CheckAttempt) -> Result<()>;
    async fn observe(&self, operation: Uuid) -> Result<CheckObservation>;
}

/// One bounded provider request. `credential_profile` is a reference chosen
/// by the host; the secret it names never crosses this boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgmentRequest {
    pub operation: Uuid,
    pub credential_profile: String,
    pub question: String,
    pub context_text: String,
    pub context_artifacts: Vec<ArtifactPin>,
    pub choices: Vec<String>,
    pub rationale_max_bytes: u64,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub max_cost_microunits: u64,
    pub deadline_ms: u64,
}

/// A provider completion with its reported usage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgmentReply {
    pub provider: String,
    pub model: String,
    pub choice: String,
    pub rationale: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_microunits: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderFailure {
    Error,
    Unavailable,
}

/// Default pre-call input-token estimate: one token per four UTF-8 bytes of
/// the text the host sends (question, context text and choices), rounded up.
/// Artifact contents are resolved by the provider, so a provider that inlines
/// them should override [`JudgmentProvider::estimate_input_tokens`]. The
/// provider-reported usage check after the call remains authoritative.
pub fn estimate_input_tokens(request: &JudgmentRequest) -> u64 {
    let bytes = request.question.len()
        + request.context_text.len()
        + request.choices.iter().map(String::len).sum::<usize>();
    (bytes as u64).div_ceil(4)
}

/// Grace after a judgment's provider deadline before `operation.get` treats a
/// still-running operation as lost (indeterminate). The in-process provider
/// task settles at its own deadline; the grace keeps reconciliation from
/// racing it.
pub const JUDGMENT_RECONCILE_GRACE_MS: i64 = 5_000;

/// Judgment provider port: one completion per request, no tool loop.
#[async_trait::async_trait]
pub trait JudgmentProvider: Send + Sync {
    /// The credential profile reference the host permits for this scope.
    fn credential_profile(&self, workspace: Uuid, project: Uuid) -> Option<String>;
    /// Input tokens `request` is expected to consume. A request whose
    /// estimate exceeds `max_input_tokens` is refused `budget_exceeded`
    /// before anything is sent.
    fn estimate_input_tokens(&self, request: &JudgmentRequest) -> u64 {
        estimate_input_tokens(request)
    }
    async fn complete(
        &self,
        request: &JudgmentRequest,
    ) -> std::result::Result<JudgmentReply, ProviderFailure>;
}

fn source_valid(source: &Value) -> bool {
    let want = match source["format"].as_str() {
        Some("sha1") => 40,
        Some("sha256") => 64,
        _ => return false,
    };
    source["hex"].as_str().is_some_and(|h| {
        h.len() == want
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn pin(value: &Value) -> Result<ArtifactPin> {
    serde_json::from_value(value.clone()).map_err(|_| Error::Invalid)
}

fn revision_of(subject: &Value) -> Result<i64> {
    subject["revision"]
        .as_i64()
        .filter(|r| *r >= 0)
        .ok_or(Error::Invalid)
}

/// One accepted judgment whose provider request runs off the dispatch path.
struct JudgmentJob {
    operation: Uuid,
    command: String,
    intent_id: String,
    reference: String,
    subject: Value,
    credential_profile: String,
    prompt_digest: String,
    context_digest: String,
    request: JudgmentRequest,
}

/// Completed outcome and output of one retained host operation.
struct Operation {
    command: String,
    intent_id: String,
    request: Value,
    status: String,
    output: Option<Value>,
}

impl Service {
    fn completion_contract(&self, command: &str) -> Result<(Value, &'static str)> {
        Ok(match command {
            "judgment.request" => (json!("judgment"), "aq.judgment.completed"),
            "check.request" => (json!("check"), "aq.check.receipted"),
            _ => return Err(Error::Invalid),
        })
    }

    /// Counts against the retention bounds; returns false when a bound is hit.
    async fn operation_room(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        subject: &Value,
        check: Option<&str>,
    ) -> Result<bool> {
        let id = text(subject, "id")?;
        let row = sqlx::query("SELECT count(*) AS total, count(*) FILTER (WHERE subject_id=$3) AS lineage, count(*) FILTER (WHERE subject_id=$3 AND subject_revision=$4 AND check_id=$5) AS executions FROM playbook_host_operations WHERE namespace_id=$1 AND generation=$2")
            .bind(self.namespace.namespace_id)
            .bind(self.namespace.generation)
            .bind(id)
            .bind(revision_of(subject)?)
            .bind(check)
            .fetch_one(&mut **tx)
            .await?;
        Ok(row.get::<i64, _>("total") < MAX_RETAINED_OPERATIONS
            && row.get::<i64, _>("lineage") < MAX_LINEAGE_OPERATIONS
            && (check.is_none() || row.get::<i64, _>("executions") < MAX_CHECK_EXECUTIONS))
    }

    /// Records the durable intent (accepted effect row and running operation
    /// row) and commits it before any external effect.
    #[allow(clippy::too_many_arguments)]
    async fn record_operation(
        &self,
        mut tx: Transaction<'_, Postgres>,
        id: Uuid,
        revision: i64,
        intent: &Intent,
        identity: &str,
        check: Option<&str>,
        request: &Value,
    ) -> Result<HostResult> {
        let operation = self.operation(intent)?;
        let accepted = HostResult {
            outcome: "accepted".into(),
            output: json!({"handle": operation.to_string()}),
        };
        self.record(
            &mut tx,
            id,
            revision,
            intent,
            identity,
            &accepted.outcome,
            &accepted.output,
        )
        .await?;
        let subject = &intent.input["subject"];
        sqlx::query("INSERT INTO playbook_host_operations(operation_id,namespace_id,generation,intent_id,command_name,subject_id,subject_revision,check_id,request,status,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'running',$10)")
            .bind(operation)
            .bind(self.namespace.namespace_id)
            .bind(self.namespace.generation)
            .bind(&intent.id)
            .bind(&intent.command.name)
            .bind(text(subject, "id")?)
            .bind(revision_of(subject)?)
            .bind(check)
            .bind(request)
            .bind(self.now()?)
            .execute(&mut *tx)
            .await?;
        self.commit_fresh(tx, id, intent).await?;
        Ok(accepted)
    }

    async fn load_operation(&self, operation: Uuid) -> Result<Option<Operation>> {
        let row = sqlx::query("SELECT command_name,intent_id,request,status,output FROM playbook_host_operations WHERE operation_id=$1 AND namespace_id=$2 AND generation=$3")
            .bind(operation)
            .bind(self.namespace.namespace_id)
            .bind(self.namespace.generation)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| Operation {
            command: row.get("command_name"),
            intent_id: row.get("intent_id"),
            request: row.get("request"),
            status: row.get("status"),
            output: row.get("output"),
        }))
    }

    /// Completes a running operation exactly once: the retained completion and
    /// its sealed completion event commit together. A completion that lost the
    /// race returns the retained one.
    async fn complete_operation(
        &self,
        operation: Uuid,
        command: &str,
        intent_id: &str,
        result: HostResult,
    ) -> Result<HostResult> {
        let (_, contract) = self.completion_contract(command)?;
        let payload = json!({"outcome": result.outcome, "output": result.output});
        self.profile
            .registry()
            .commands
            .get(command)
            .and_then(|c| c.asynchronous.as_ref())
            .ok_or(Error::Invalid)?
            .completion
            .validate(&payload)
            .map_err(|_| Error::Unknown)?;
        let mut tx = self.tx().await?;
        self.namespace_locked(&mut tx).await?;
        let updated = sqlx::query("UPDATE playbook_host_operations SET status=$4,output=$5,completed_at_ms=$6 WHERE operation_id=$1 AND namespace_id=$2 AND generation=$3 AND status='running'")
            .bind(operation)
            .bind(self.namespace.namespace_id)
            .bind(self.namespace.generation)
            .bind(&result.outcome)
            .bind(&result.output)
            .bind(self.now()?)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if updated == 0 {
            let row = sqlx::query("SELECT status,output FROM playbook_host_operations WHERE operation_id=$1 AND namespace_id=$2 AND generation=$3")
                .bind(operation)
                .bind(self.namespace.namespace_id)
                .bind(self.namespace.generation)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(Error::Unknown)?;
            tx.commit().await?;
            return Ok(HostResult {
                outcome: row.get("status"),
                output: row
                    .get::<Option<Value>, _>("output")
                    .ok_or(Error::Unknown)?,
            });
        }
        let event_id = stable_id(
            &format!("aq.playbook.{command}.completion"),
            self.namespace.namespace_id,
            self.namespace.generation,
            intent_id,
        )?;
        self.seal_event(
            &mut tx,
            contract,
            event_id,
            operation,
            intent_id,
            &result.output["subject"],
            payload,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    // ------------------------------------------------------------- check.request

    pub(super) async fn check_request(
        &self,
        owner: &RuntimeOwner,
        id: Uuid,
        revision: i64,
        intent: &Intent,
    ) -> Result<HostResult> {
        let ports = self.ports()?;
        let (mut tx, stage, identity) = self.begin(Some(owner), id, revision, intent).await?;
        let operation = self.operation(intent)?;
        match stage.ok_or(Error::Unknown)? {
            Stage::Retained(result) => {
                tx.commit().await?;
                // Lost response: observe the original attempt, never restart.
                // An observation failure leaves the retained handle intact.
                if result.outcome == "accepted" {
                    let _ = self.observe_check(operation).await;
                }
                return Ok(result);
            }
            Stage::Pending(_) => return Err(Error::Conflict),
            Stage::Fresh => {}
        }
        let input = &intent.input;
        let subject = &input["subject"];
        let check = &input["check"];
        let check_id = text(check, "id")?;
        let refusal = |disposition: Value, reason: &str| HostResult {
            outcome: "refused".into(),
            output: json!({"check": check, "disposition": disposition, "reason": reason,
                "reference": format!("check:{operation}"), "source": input["source"], "subject": subject}),
        };
        let workspace = self.namespace.workspace_id;
        let project = self.namespace.project_id;
        let refused = if check_id.is_empty() || !source_valid(&input["source"]) {
            Some(refusal(Value::Null, "invalid_request"))
        } else if let Some(checks) = &ports.checks {
            let validator = pin(&check["validator"])?;
            match checks
                .pinned(workspace, project, text(subject, "id")?)
                .await?
            {
                None => Some(refusal(Value::Null, "not_required")),
                Some(p) if p.revision != revision_of(subject)? => {
                    Some(refusal(json!("stale"), "stale_subject"))
                }
                Some(p)
                    if p.required_checks.len() > 32
                        || !p
                            .required_checks
                            .iter()
                            .any(|c| c.id == check_id && c.validator == validator) =>
                {
                    Some(refusal(Value::Null, "not_required"))
                }
                Some(_)
                    if !self
                        .operation_room(&mut tx, subject, Some(check_id))
                        .await? =>
                {
                    Some(refusal(Value::Null, "execution_limit"))
                }
                Some(_) => None,
            }
        } else {
            Some(refusal(Value::Null, "unavailable"))
        };
        if let Some(result) = refused {
            return self
                .settle_fresh(tx, id, revision, intent, &identity, result)
                .await;
        }
        let checks = ports.checks.as_ref().ok_or(Error::Unknown)?;
        let request = json!({"check": check, "source": input["source"], "subject": subject,
            "deadline_at_ms": intent.deadline_at_ms, "execution_profile": CHECK_EXECUTION_PROFILE});
        let accepted = self
            .record_operation(
                tx,
                id,
                revision,
                intent,
                &identity,
                Some(check_id),
                &request,
            )
            .await?;
        let attempt = CheckAttempt {
            operation,
            workspace,
            project,
            subject_id: text(subject, "id")?.into(),
            subject_revision: revision_of(subject)?,
            check_id: check_id.into(),
            validator: pin(&check["validator"])?,
            source_format: text(&input["source"], "format")?.into(),
            source_hex: text(&input["source"], "hex")?.into(),
            execution_profile: CHECK_EXECUTION_PROFILE,
        };
        // An ambiguous start leaves the operation running; reconciliation
        // observes it and never issues a second start.
        // The handle is durable from here: an observation failure leaves the
        // operation running for operation.get.
        if checks.start(&attempt).await.is_ok() {
            let _ = self.observe_check(operation).await;
        }
        Ok(accepted)
    }

    /// Observes a running check once and completes it when the executor has
    /// stopped it. Returns the completion if there is one.
    async fn observe_check(&self, operation: Uuid) -> Result<Option<HostResult>> {
        let Some(op) = self.load_operation(operation).await? else {
            return Ok(None);
        };
        if op.status != "running" {
            return Ok(Some(HostResult {
                outcome: op.status,
                output: op.output.ok_or(Error::Unknown)?,
            }));
        }
        let checks = self.ports()?.checks.as_ref().ok_or(Error::Unknown)?;
        let r = &op.request;
        let base = |outcome: &str, disposition: &str, receipt: Value| HostResult {
            outcome: outcome.into(),
            output: json!({"check": r["check"], "disposition": disposition, "receipt": receipt,
                "reference": format!("check-receipt:{operation}"), "source": r["source"], "subject": r["subject"]}),
        };
        let result = match checks.observe(operation).await? {
            CheckObservation::Running => return Ok(None),
            CheckObservation::NotFound => {
                if self.now()? < r["deadline_at_ms"].as_i64().ok_or(Error::Unknown)? {
                    return Ok(None);
                }
                base("infrastructure_failure", "unknown", Value::Null)
            }
            CheckObservation::Finished(run) => {
                let (code, stop) = match run.stop {
                    CheckStop::Exited(code) => (json!(code), "exited"),
                    CheckStop::Signalled => (Value::Null, "signalled"),
                    CheckStop::TimedOut => (Value::Null, "timed_out"),
                    CheckStop::Cancelled => (Value::Null, "cancelled"),
                    CheckStop::Lost => (Value::Null, "lost"),
                };
                let exit = json!({"code": code, "stop": stop});
                let artifacts = serde_json::to_value(&run.result_artifacts)?;
                let receipt_digest = digest(&(
                    "aq.check.receipt",
                    &r["subject"],
                    &r["check"],
                    &r["source"],
                    CHECK_EXECUTION_PROFILE,
                    &run.executor_version,
                    &run.inputs_digest,
                    &artifacts,
                    &exit,
                ))?;
                let receipt = json!({"digest": receipt_digest, "executor_version": run.executor_version,
                    "inputs_digest": run.inputs_digest, "result_artifacts": artifacts, "exit": exit});
                // Missing input or an uncertain stop is never passed.
                match (&run.stop, run.inputs_complete) {
                    (CheckStop::Exited(0), true) => base("passed", "passed", receipt),
                    (CheckStop::Exited(_), true) => base("failed", "failed", receipt),
                    (CheckStop::TimedOut, _) => {
                        base("infrastructure_failure", "timed_out", receipt)
                    }
                    (CheckStop::Cancelled, _) => {
                        base("infrastructure_failure", "cancelled", receipt)
                    }
                    _ => base("infrastructure_failure", "unknown", receipt),
                }
            }
        };
        self.complete_operation(operation, &op.command, &op.intent_id, result)
            .await
            .map(Some)
    }

    // ---------------------------------------------------------- judgment.request

    pub(super) async fn judgment_request(
        &self,
        owner: &RuntimeOwner,
        id: Uuid,
        revision: i64,
        intent: &Intent,
    ) -> Result<HostResult> {
        let ports = self.ports()?;
        let (mut tx, stage, identity) = self.begin(Some(owner), id, revision, intent).await?;
        let operation = self.operation(intent)?;
        match stage.ok_or(Error::Unknown)? {
            Stage::Retained(result) => {
                tx.commit().await?;
                // Lost response: the original request is never re-sent.
                return Ok(result);
            }
            Stage::Pending(_) => return Err(Error::Conflict),
            Stage::Fresh => {}
        }
        let input = &intent.input;
        let subject = &input["subject"];
        let reference = format!("judgment:{operation}");
        let refusal = |provenance: Value, reason: &str| HostResult {
            outcome: "refused".into(),
            output: json!({"provenance": provenance, "reason": reason, "reference": reference, "subject": subject}),
        };
        let workspace = self.namespace.workspace_id;
        let project = self.namespace.project_id;
        let choices: Vec<String> = serde_json::from_value(input["output"]["choices"].clone())
            .map_err(|_| Error::Invalid)?;
        let stale = match Uuid::parse_str(text(subject, "id")?) {
            Ok(node) => match ports.graph.node(workspace, node).await? {
                Some(n) => node_version(&n)? != revision_of(subject)?,
                None => false,
            },
            Err(_) => false,
        };
        let profile = ports
            .judgments
            .as_ref()
            .and_then(|j| j.credential_profile(workspace, project).map(|p| (j, p)));
        let refused = if choices.is_empty() || text(input, "question")?.is_empty() {
            Some(refusal(Value::Null, "invalid_request"))
        } else if profile.is_none() {
            Some(refusal(Value::Null, "no_permitted_profile"))
        } else if stale {
            Some(refusal(Value::Null, "stale_subject"))
        } else if !self.operation_room(&mut tx, subject, None).await? {
            Some(refusal(Value::Null, "budget_exceeded"))
        } else {
            None
        };
        if let Some(result) = refused {
            return self
                .settle_fresh(tx, id, revision, intent, &identity, result)
                .await;
        }
        let (provider, credential_profile) = profile.ok_or(Error::Unknown)?;
        let provider = provider.clone();
        let budget = &input["budget"];
        let field = |name: &str| budget[name].as_u64().ok_or(Error::Invalid);
        // The provider runs off the dispatch path, so its deadline is the
        // declared budget, not what is left of the dispatch deadline.
        let request = JudgmentRequest {
            operation,
            credential_profile: credential_profile.clone(),
            question: text(input, "question")?.into(),
            context_text: text(&input["context"], "text")?.into(),
            context_artifacts: serde_json::from_value(input["context"]["artifacts"].clone())
                .map_err(|_| Error::Invalid)?,
            choices: choices.clone(),
            rationale_max_bytes: input["output"]["rationale_max_bytes"]
                .as_u64()
                .ok_or(Error::Invalid)?,
            max_input_tokens: field("max_input_tokens")?,
            max_output_tokens: field("max_output_tokens")?,
            max_cost_microunits: field("max_cost_microunits")?,
            deadline_ms: field("deadline_ms")?,
        };
        // Pre-call estimate: a request expected to exceed the input budget is
        // refused before any operation is recorded or anything is sent.
        let estimated_input_tokens = provider.estimate_input_tokens(&request);
        if estimated_input_tokens > request.max_input_tokens {
            let result = refusal(Value::Null, "budget_exceeded");
            return self
                .settle_fresh(tx, id, revision, intent, &identity, result)
                .await;
        }
        let prompt_digest = digest(&(
            "aq.judgment.prompt",
            &request.question,
            &request.choices,
            request.rationale_max_bytes,
        ))?;
        let context_digest = digest(&("aq.judgment.context", &input["context"]))?;
        let record = json!({"subject": subject, "credential_profile": credential_profile,
            "prompt_digest": prompt_digest, "context_digest": context_digest,
            "estimated_input_tokens": estimated_input_tokens,
            "deadline_at_ms": self.now()? + request.deadline_ms as i64});
        let accepted = self
            .record_operation(tx, id, revision, intent, &identity, None, &record)
            .await?;
        // The intent is durable: the provider request runs in the background
        // and completes the operation by its sealed aq.judgment.completed
        // event. A completion that fails to commit leaves the operation
        // running; it reconciles to indeterminate and is never re-sent.
        let job = JudgmentJob {
            operation,
            command: intent.command.name.clone(),
            intent_id: intent.id.clone(),
            reference: reference.clone(),
            subject: subject.clone(),
            credential_profile,
            prompt_digest,
            context_digest,
            request,
        };
        let service = self.clone();
        tokio::spawn(async move {
            let _ = service.run_judgment(provider, job).await;
        });
        Ok(accepted)
    }

    /// Sends one accepted judgment's provider request, bounded by its budget
    /// deadline, and completes the operation exactly once.
    async fn run_judgment(
        &self,
        provider: Arc<dyn JudgmentProvider>,
        job: JudgmentJob,
    ) -> Result<HostResult> {
        let JudgmentJob {
            operation,
            command,
            intent_id,
            reference,
            subject,
            credential_profile,
            prompt_digest,
            context_digest,
            request,
        } = job;
        let reply = tokio::time::timeout(
            Duration::from_millis(request.deadline_ms),
            provider.complete(&request),
        )
        .await;
        let refusal = |provenance: Value, reason: &str| HostResult {
            outcome: "refused".into(),
            output: json!({"provenance": provenance, "reason": reason, "reference": reference, "subject": subject}),
        };
        let provenance = |reply: &JudgmentReply, output: Value| {
            json!({"provider": reply.provider, "model": reply.model,
                "credential_profile": credential_profile, "prompt_digest": prompt_digest,
                "context_digest": context_digest, "output_digest": output,
                "usage": {"input_tokens": reply.input_tokens, "output_tokens": reply.output_tokens,
                    "cost_microunits": reply.cost_microunits}})
        };
        let failed = |reason: &str| HostResult {
            outcome: "failed".into(),
            output: json!({"provenance": null, "reason": reason, "reference": reference, "subject": subject}),
        };
        let result = match reply {
            Err(_) => refusal(Value::Null, "deadline_exceeded"),
            Ok(Err(ProviderFailure::Error)) => failed("provider_error"),
            Ok(Err(ProviderFailure::Unavailable)) => failed("provider_unavailable"),
            Ok(Ok(reply)) => {
                if reply.input_tokens > request.max_input_tokens
                    || reply.output_tokens > request.max_output_tokens
                    || reply.cost_microunits > request.max_cost_microunits
                {
                    refusal(provenance(&reply, Value::Null), "budget_exceeded")
                } else if !request.choices.contains(&reply.choice)
                    || reply.rationale.len() as u64 > request.rationale_max_bytes
                    || reply.provider.is_empty()
                    || reply.provider.len() > 128
                    || reply.model.is_empty()
                    || reply.model.len() > 256
                {
                    refusal(Value::Null, "invalid_output")
                } else {
                    let output_digest =
                        digest(&("aq.judgment.output", &reply.choice, &reply.rationale))?;
                    HostResult {
                        outcome: "assessed".into(),
                        output: json!({"choice": reply.choice, "rationale": reply.rationale,
                            "provenance": provenance(&reply, json!(output_digest)),
                            "reference": reference, "subject": subject}),
                    }
                }
            }
        };
        self.complete_operation(operation, &command, &intent_id, result)
            .await
    }

    // ------------------------------------------------------------ operation.get

    /// Observation only: reads (and, for a stopped check, completes) the
    /// operation named by the handle in this namespace generation.
    pub(super) async fn host_operation_get(
        &self,
        owner: &RuntimeOwner,
        id: Uuid,
        revision: i64,
        intent: &Intent,
    ) -> Result<HostResult> {
        let (tx, stage, _) = self.begin(Some(owner), id, revision, intent).await?;
        tx.commit().await?;
        if !matches!(stage, Some(Stage::Fresh)) {
            return Err(Error::Conflict);
        }
        let handle = text(&intent.input, "handle")?;
        let missing = || HostResult {
            outcome: "refused".into(),
            output: json!({"reference": handle, "subject": subject(handle, 0)}),
        };
        let Ok(operation) = Uuid::parse_str(handle) else {
            return Ok(missing());
        };
        let Some(op) = self.load_operation(operation).await? else {
            return Ok(missing());
        };
        let completion = if op.status != "running" {
            Some(HostResult {
                outcome: op.status.clone(),
                output: op.output.clone().ok_or(Error::Unknown)?,
            })
        } else if op.command == "check.request" {
            self.observe_check(operation).await?
        } else if self.now()?
            >= op.request["deadline_at_ms"]
                .as_i64()
                .ok_or(Error::Unknown)?
                .saturating_add(JUDGMENT_RECONCILE_GRACE_MS)
        {
            // The provider task outlived its deadline and grace without a
            // completion (process loss): indeterminate, never re-sent.
            let output = json!({"provenance": null, "reason": "indeterminate",
                "reference": format!("judgment:{operation}"), "subject": op.request["subject"]});
            Some(
                self.complete_operation(
                    operation,
                    &op.command,
                    &op.intent_id,
                    HostResult {
                        outcome: "failed".into(),
                        output,
                    },
                )
                .await?,
            )
        } else {
            None
        };
        let result = match completion {
            Some(done) => HostResult {
                outcome: "completed".into(),
                output: json!({"outcome": done.outcome, "output": done.output}),
            },
            None => {
                let (kind, _) = self.completion_contract(&op.command)?;
                HostResult {
                    outcome: "running".into(),
                    output: json!({"reference": format!("{}:{operation}", kind.as_str().unwrap_or("operation")),
                        "subject": op.request["subject"]}),
                }
            }
        };
        self.validated(intent, &result.outcome, &result.output)?;
        Ok(result)
    }
}
