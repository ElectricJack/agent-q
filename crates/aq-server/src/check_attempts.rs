//! D6 follow-up: the server->node check-attempt execution kind backing the
//! Host7 `CheckExecutor` port.
//!
//! `start` records one `check_attempts` row per check.request operation,
//! bound to the current profiled result that holds the subject task and to
//! the retained source the request names, and assigns it to the node that ran
//! that attempt. That node claims the attempt over the worker API, runs the
//! pinned validator in its sandbox, and reports one stop. The stop is final
//! and idempotent per operation id; `profiled_check_evidence` is written only
//! from it, in the same transaction, never from the host.
use crate::{
    AppState,
    error::AppError,
    playbooks::domain::{Error, Result as HostResult},
    playbooks::host_adapters::{
        ArtifactPin, CheckAttempt, CheckExecutor, CheckObservation, CheckRun, CheckStop,
        PinnedCheck, PinnedOutcome,
    },
    profiled_execution::results::{self, RecordedResult},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::HeaderMap,
    routing::post,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;

type Result<T> = std::result::Result<T, AppError>;
const MAX_CLAIM: u32 = 16;
const MAX_RESULT_ARTIFACTS: usize = 16;

fn db(_: sqlx::Error) -> AppError {
    AppError::Internal
}
fn bad(s: &str) -> AppError {
    AppError::BadRequest(s.into())
}
fn host(e: AppError) -> Error {
    match e {
        AppError::Conflict(_) | AppError::NotFound(_) => Error::Conflict,
        AppError::BadRequest(_) => Error::Invalid,
        _ => Error::Unknown,
    }
}
fn now(state: &AppState) -> i64 {
    (state.clock)().timestamp_millis()
}
fn sha256_text(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|h| {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The production check executor, bound only where profiled execution (and
/// therefore a node that can run the attempt) is configured.
pub fn production_executor(state: &AppState) -> Option<Arc<dyn CheckExecutor>> {
    state
        .config
        .profiled_execution
        .as_ref()
        .map(|_| Arc::new(NodeCheckExecutor::new(state)) as Arc<dyn CheckExecutor>)
}

/// Dispatches check attempts to the node that ran the pinned profiled result.
pub struct NodeCheckExecutor {
    state: AppState,
}
impl NodeCheckExecutor {
    pub fn new(state: &AppState) -> Self {
        Self {
            state: state.clone(),
        }
    }
}

/// The current recorded result of the attempt holding `subject_id` (a task
/// id) in this workspace and project, and the node that ran it.
async fn holding_result(
    state: &AppState,
    tx: &mut Transaction<'_, Postgres>,
    workspace: Uuid,
    project: Uuid,
    subject_id: &str,
) -> Result<Option<(RecordedResult, Uuid)>> {
    let Ok(task) = Uuid::parse_str(subject_id) else {
        return Ok(None);
    };
    let row = sqlx::query("SELECT h.report_id, a.node_id FROM execution_attempts a JOIN profiled_result_heads h ON h.attempt_id=a.attempt_id WHERE a.task_id=$1 AND a.holds_task FOR SHARE OF a")
        .bind(task)
        .fetch_optional(&mut **tx)
        .await
        .map_err(db)?;
    let Some(row) = row else { return Ok(None) };
    let recorded = results::load_locked(state, tx, row.get("report_id")).await?;
    if !recorded.is_current_report_head
        || recorded.spec.lifecycle.project_id != project
        || recorded.spec.task.workspace_id != workspace
        || recorded.spec.task.task_id != task
    {
        return Ok(None);
    }
    Ok(Some((recorded, row.get("node_id"))))
}

fn pinned_outcome(recorded: &RecordedResult) -> PinnedOutcome {
    PinnedOutcome {
        revision: recorded.outcome.task.version as i64,
        required_checks: recorded
            .outcome
            .required_checks
            .iter()
            .map(|c| PinnedCheck {
                id: c.id.as_str().into(),
                validator: ArtifactPin {
                    id: c.validator.id.as_str().into(),
                    version: c.validator.version.as_str().into(),
                    digest: format!("sha256:{}", c.validator.digest.as_str()),
                },
            })
            .collect(),
    }
}

fn request_digest(a: &CheckAttempt) -> HostResult<String> {
    let value = json!({"operation": a.operation, "workspace": a.workspace, "project": a.project,
        "subject": {"id": a.subject_id, "revision": a.subject_revision}, "check": {"id": a.check_id,
        "validator": a.validator}, "source": {"format": a.source_format, "hex": a.source_hex},
        "execution_profile": a.execution_profile});
    let bytes = aq_agent_api::canonical(&value).map_err(|_| Error::Invalid)?;
    Ok(aq_admission::Digest::of(&bytes).as_str().to_owned())
}

#[async_trait::async_trait]
impl CheckExecutor for NodeCheckExecutor {
    async fn pinned(
        &self,
        workspace: Uuid,
        project: Uuid,
        subject_id: &str,
    ) -> HostResult<Option<PinnedOutcome>> {
        let mut tx = self.state.store.pool.begin().await?;
        let found = holding_result(&self.state, &mut tx, workspace, project, subject_id)
            .await
            .map_err(host)?;
        tx.commit().await?;
        Ok(found.map(|(recorded, _)| pinned_outcome(&recorded)))
    }

    async fn start(&self, a: &CheckAttempt) -> HostResult<()> {
        let digest = request_digest(a)?;
        let mut tx = self.state.store.pool.begin().await?;
        let existing: Option<String> =
            sqlx::query_scalar("SELECT request_digest FROM check_attempts WHERE operation_id=$1")
                .bind(a.operation)
                .fetch_optional(&mut *tx)
                .await?;
        if let Some(existing) = existing {
            // Idempotent per operation id: one attempt, ever.
            return (existing == digest).then_some(()).ok_or(Error::Conflict);
        }
        let (recorded, node) =
            holding_result(&self.state, &mut tx, a.workspace, a.project, &a.subject_id)
                .await
                .map_err(host)?
                .ok_or(Error::Conflict)?;
        let pinned = pinned_outcome(&recorded);
        if pinned.revision != a.subject_revision
            || !pinned
                .required_checks
                .iter()
                .any(|c| c.id == a.check_id && c.validator == a.validator)
        {
            return Err(Error::Conflict);
        }
        // The named source must be a retained checkpoint pinned by this result.
        let mut source = None;
        for op in &recorded.report.source_receipt_ids {
            match crate::source_retention::load_confirmed_locked(&mut tx, *op, &recorded).await {
                Ok(c) if c.manifest.checkpoint.hex == a.source_hex => {
                    source = Some(*op);
                    break;
                }
                Ok(_) | Err(AppError::Conflict(_)) => {}
                Err(e) => return Err(host(e)),
            }
        }
        let source = source.ok_or(Error::Conflict)?;
        let time = now(&self.state);
        sqlx::query("INSERT INTO check_attempts(operation_id,workspace_id,project_id,node_id,attempt_id,report_id,source_operation_id,subject_id,subject_revision,check_id,validator,source_format,source_hex,execution_profile,request_digest,state,stop,created_at_ms,updated_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,'queued',NULL,$16,$16) ON CONFLICT(operation_id) DO NOTHING")
            .bind(a.operation)
            .bind(a.workspace)
            .bind(a.project)
            .bind(node)
            .bind(recorded.report.binding.attempt_id)
            .bind(recorded.report.report_id)
            .bind(source)
            .bind(&a.subject_id)
            .bind(a.subject_revision)
            .bind(&a.check_id)
            .bind(serde_json::to_value(&a.validator)?)
            .bind(&a.source_format)
            .bind(&a.source_hex)
            .bind(a.execution_profile)
            .bind(&digest)
            .bind(time)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn observe(&self, operation: Uuid) -> HostResult<CheckObservation> {
        let row = sqlx::query("SELECT state, stop FROM check_attempts WHERE operation_id=$1")
            .bind(operation)
            .fetch_optional(&self.state.store.pool)
            .await?;
        let Some(row) = row else {
            return Ok(CheckObservation::NotFound);
        };
        let stop: Option<Value> = row.get("stop");
        match (row.get::<String, _>("state").as_str(), stop) {
            ("stopped", Some(stop)) => {
                let report: StopBody = serde_json::from_value(stop).map_err(|_| Error::Unknown)?;
                Ok(CheckObservation::Finished(report.run()))
            }
            ("queued" | "running", None) => Ok(CheckObservation::Running),
            _ => Err(Error::Unknown),
        }
    }
}

// ----------------------------------------------------------------- node API

pub fn worker_routes() -> Router<AppState> {
    Router::new()
        .route("/v1/worker/checks/claim", post(claim))
        .route("/v1/worker/checks/stop", post(stop))
        .layer(DefaultBodyLimit::max(64 * 1024))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimRequest {
    pub limit: u32,
}

/// Returns this node's unstopped check attempts (oldest first) and marks the
/// queued ones running. Re-claiming a running attempt returns it again.
async fn claim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClaimRequest>,
) -> Result<Json<Value>> {
    if request.limit == 0 || request.limit > MAX_CLAIM {
        return Err(bad("invalid check claim limit"));
    }
    let auth = crate::worker_api::authenticate_context(&state, &headers).await?;
    let mut tx = state.store.pool.begin().await.map_err(db)?;
    let node = crate::worker_api::lock_current_node(&mut tx, &auth).await?;
    let rows = sqlx::query("SELECT operation_id,attempt_id,source_operation_id,subject_id,subject_revision,check_id,validator,source_format,source_hex,execution_profile,state FROM check_attempts WHERE node_id=$1 AND state<>'stopped' ORDER BY created_at_ms,operation_id LIMIT $2 FOR UPDATE")
        .bind(node)
        .bind(i64::from(request.limit))
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?;
    let queued: Vec<Uuid> = rows
        .iter()
        .filter(|r| r.get::<String, _>("state") == "queued")
        .map(|r| r.get("operation_id"))
        .collect();
    sqlx::query("UPDATE check_attempts SET state='running',updated_at_ms=$3 WHERE node_id=$1 AND operation_id=ANY($2) AND state='queued'")
        .bind(node)
        .bind(&queued)
        .bind(now(&state))
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    tx.commit().await.map_err(db)?;
    let checks: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "operation_id": r.get::<Uuid, _>("operation_id"),
                "attempt_id": r.get::<Uuid, _>("attempt_id"),
                "source_operation_id": r.get::<Uuid, _>("source_operation_id"),
                "subject": {"id": r.get::<String, _>("subject_id"), "revision": r.get::<i64, _>("subject_revision")},
                "check": {"id": r.get::<String, _>("check_id"), "validator": r.get::<Value, _>("validator")},
                "source": {"format": r.get::<String, _>("source_format"), "hex": r.get::<String, _>("source_hex")},
                "execution_profile": r.get::<String, _>("execution_profile"),
            })
        })
        .collect();
    Ok(Json(json!({"checks": checks})))
}

/// How the node's sandbox stopped the validator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StopKind {
    Exited { code: i32 },
    Signalled,
    TimedOut,
    Cancelled,
    Lost,
}

/// The node-reported stop, retained verbatim as `check_attempts.stop`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopBody {
    pub stop: StopKind,
    pub executor_version: String,
    pub inputs_digest: String,
    pub inputs_complete: bool,
    pub result_artifacts: Vec<ArtifactPin>,
}
impl StopBody {
    fn valid(&self) -> bool {
        let text = |s: &str, max: usize| {
            !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_graphic())
        };
        text(&self.executor_version, 128)
            && sha256_text(&self.inputs_digest)
            && self.result_artifacts.len() <= MAX_RESULT_ARTIFACTS
            && self
                .result_artifacts
                .iter()
                .all(|a| text(&a.id, 256) && text(&a.version, 64) && sha256_text(&a.digest))
    }
    fn run(self) -> CheckRun {
        CheckRun {
            stop: match self.stop {
                StopKind::Exited { code } => CheckStop::Exited(code),
                StopKind::Signalled => CheckStop::Signalled,
                StopKind::TimedOut => CheckStop::TimedOut,
                StopKind::Cancelled => CheckStop::Cancelled,
                StopKind::Lost => CheckStop::Lost,
            },
            inputs_complete: self.inputs_complete,
            executor_version: self.executor_version,
            inputs_digest: self.inputs_digest,
            result_artifacts: self.result_artifacts,
        }
    }
}

/// Records the node-reported stop once (a repeat of the same stop is
/// accepted; a different one conflicts) and, for a complete exited run on a
/// sha1 checkpoint, the server-owned check evidence derived from it.
async fn stop(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Result<Json<Value>> {
    // `flatten` and `deny_unknown_fields` do not compose; split explicitly.
    let mut object = match request {
        Value::Object(o) => o,
        _ => return Err(bad("invalid check stop")),
    };
    let operation: Uuid = object
        .remove("operation_id")
        .and_then(|v| serde_json::from_value(v).ok())
        .ok_or_else(|| bad("invalid check stop"))?;
    let body: StopBody =
        serde_json::from_value(Value::Object(object)).map_err(|_| bad("invalid check stop"))?;
    if operation.is_nil() || !body.valid() {
        return Err(bad("invalid check stop"));
    }
    let auth = crate::worker_api::authenticate_context(&state, &headers).await?;
    let mut tx = state.store.pool.begin().await.map_err(db)?;
    let node = crate::worker_api::lock_current_node(&mut tx, &auth).await?;
    let row = sqlx::query("SELECT node_id,attempt_id,source_operation_id,check_id,validator,source_format,source_hex,execution_profile,state,stop FROM check_attempts WHERE operation_id=$1 FOR UPDATE")
        .bind(operation)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?
        .filter(|r| r.get::<Uuid, _>("node_id") == node)
        .ok_or_else(|| AppError::NotFound("check attempt not found".into()))?;
    let reported = serde_json::to_value(&body).map_err(|_| AppError::Internal)?;
    if row.get::<String, _>("state") == "stopped" {
        let recorded: Option<Value> = row.get("stop");
        return if recorded.as_ref() == Some(&reported) {
            Ok(Json(json!({"operation_id": operation, "recorded": true})))
        } else {
            Err(AppError::Conflict("check attempt already stopped".into()))
        };
    }
    let time = now(&state);
    sqlx::query(
        "UPDATE check_attempts SET state='stopped',stop=$2,updated_at_ms=$3 WHERE operation_id=$1",
    )
    .bind(operation)
    .bind(&reported)
    .bind(time)
    .execute(&mut *tx)
    .await
    .map_err(db)?;
    // Missing input or an uncertain stop is never evidence.
    if let (StopKind::Exited { code }, true, "sha1") = (
        &body.stop,
        body.inputs_complete,
        row.get::<String, _>("source_format").as_str(),
    ) {
        let validator: ArtifactPin =
            serde_json::from_value(row.get("validator")).map_err(|_| AppError::Internal)?;
        let validator_digest = validator
            .digest
            .strip_prefix("sha256:")
            .ok_or(AppError::Internal)?;
        let evidence = json!({"operation_id": operation, "node_id": node,
            "execution_profile": row.get::<String, _>("execution_profile"),
            "executor_version": body.executor_version, "inputs_digest": body.inputs_digest,
            "exit": {"code": code, "stop": "exited"}, "result_artifacts": body.result_artifacts});
        sqlx::query("INSERT INTO profiled_check_evidence(attempt_id,check_id,source_operation_id,checkpoint,validator_digest,verdict,evidence,recorded_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT(attempt_id,check_id,source_operation_id) DO UPDATE SET checkpoint=EXCLUDED.checkpoint,validator_digest=EXCLUDED.validator_digest,verdict=EXCLUDED.verdict,evidence=EXCLUDED.evidence,recorded_at_ms=EXCLUDED.recorded_at_ms")
            .bind(row.get::<Uuid, _>("attempt_id"))
            .bind(row.get::<String, _>("check_id"))
            .bind(row.get::<Uuid, _>("source_operation_id"))
            .bind(row.get::<String, _>("source_hex"))
            .bind(validator_digest)
            .bind(if *code == 0 { "passed" } else { "failed" })
            .bind(&evidence)
            .bind(time)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
    }
    tx.commit().await.map_err(db)?;
    Ok(Json(json!({"operation_id": operation, "recorded": true})))
}
