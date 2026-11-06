//! Spec 0006 phase 2: an agent creates a task (`task.create`, agent route
//! `/v1/agent/tasks`). The grant, scope and depth are checked as on the
//! command surface; the per-attempt quota is reserved in one transaction;
//! the create itself is the supervisor's single-task Graph action under the
//! attempt's own journal identity, so idempotency and receipts are the same.
use super::grant_check::{AgentCaller, DISPATCHED, audit};
use super::*;
use crate::commands::refuse;
use aq_agent_grants::{AuditEntry, Decision, Refusal, RefusalCode, Relation, Target};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    api_version: u32,
    operation_id: Uuid,
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    acceptance_criteria: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    parent_id: Option<Value>,
    priority: Option<u16>,
    task_type: Option<String>,
    task_state: Option<String>,
    expected_project_revision: Option<String>,
    profile_hint: Option<Value>,
    intelligence_hint: Option<Value>,
}
pub(super) fn machine_routes() -> Router<AppState> {
    Router::new().route("/v1/agent/tasks", post(create))
}
fn internal<E>(_: E) -> Refusal {
    Refusal::error(
        RefusalCode::Internal,
        "task.create",
        "Task creation is unavailable.",
    )
}
async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    bytes: axum::body::Bytes,
) -> Response {
    let request_id = Uuid::new_v4();
    let a = match super::grant_check::agent_caller(&state, &headers).await {
        Ok(a) => a,
        Err(code) => {
            return refuse(
                Refusal::error(code, "task.create", "The caller is not authenticated."),
                request_id,
            );
        }
    };
    let Ok(raw) = serde_json::from_slice::<serde_json::Map<String, Value>>(&bytes) else {
        return Error("invalid_request").respond(request_id);
    };
    let Ok(b) = serde_json::from_value::<Body>(Value::Object(raw.clone())) else {
        return Error("invalid_request").respond(request_id);
    };
    if b.api_version != 1 {
        return Error("unsupported_api_version").respond(request_id);
    }
    match admit(&state, &a, &b, raw, request_id).await {
        Ok(response) => response,
        Err(r) => refuse(r, request_id),
    }
}
async fn admit(
    state: &AppState,
    a: &AgentCaller,
    b: &Body,
    mut params: serde_json::Map<String, Value>,
    request_id: Uuid,
) -> std::result::Result<Response, Refusal> {
    let spec = crate::commands::registry()
        .get("task.create")
        .expect("registered");
    let (decision, target) = a.authorize(state, spec, &mut params).await;
    let grant = aq_command_registry::required_grant(spec, &target).unwrap_or_default();
    let entry = |d: &Decision| AuditEntry::agent(a.ids, spec.name, grant, target, d, request_id);
    if DISPATCHED.try_with(|_| ()).is_err() {
        audit(state, &entry(&decision)).await;
    }
    let Decision::Allow(held) = decision else {
        let Decision::Deny(r) = decision else {
            unreachable!()
        };
        return Err(r);
    };
    // §8: an agent cannot pick the profile or intelligence of a task it
    // creates, so it cannot launch a child with more authority than its own.
    if b.profile_hint.as_ref().is_some_and(|v| !v.is_null())
        || b.intelligence_hint.as_ref().is_some_and(|v| !v.is_null())
    {
        let r = Refusal::scope_denied(spec.name, grant, a.grants.related(grant));
        let r = Refusal {
            message: "An agent cannot set profile_hint or intelligence_hint on a task it creates."
                .into(),
            ..r
        };
        audit(state, &entry(&Decision::Deny(r.clone()))).await;
        return Err(r);
    }
    let (project, own) = (a.ids.project_id, a.ids.task_id);
    let parent = match &b.parent_id {
        Some(Value::String(s)) if s == "self" => Some(own),
        Some(v) if !v.is_null() => v.as_str().and_then(|s| s.parse().ok()),
        _ => None,
    };
    // Reserve the create against the quota; a replay of the same operation
    // finds its own row and is not counted again.
    let depth = match target {
        Target::Task {
            relation: Relation::Descendant(d),
            ..
        } => Some(d as i32),
        _ => None,
    };
    {
        let mut tx = state.store.pool.begin().await.map_err(internal)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text,0))")
            .bind(a.ids.attempt_id)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        let (count, mine): (i64, bool) = sqlx::query_as("SELECT count(*),COALESCE(bool_or(task_id=$2),FALSE) FROM agent_created_tasks WHERE attempt_id=$1")
            .bind(a.ids.attempt_id).bind(b.operation_id).fetch_one(&mut *tx).await.map_err(internal)?;
        let max = held.limits.max_created_tasks.map_or(i64::MAX, i64::from);
        if !mine && count >= max {
            let r = Refusal::quota_exceeded(
                spec.name,
                grant,
                a.grants.related(grant),
                "max_created_tasks",
            );
            audit(state, &entry(&Decision::Deny(r.clone()))).await;
            return Err(r);
        }
        sqlx::query("INSERT INTO agent_created_tasks(task_id,attempt_id,project_id,parent_id,root_task_id,depth,operation_id,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$1,$7) ON CONFLICT DO NOTHING")
            .bind(b.operation_id).bind(a.ids.attempt_id).bind(project).bind(parent).bind(own).bind(depth).bind(now(state) as i64)
            .execute(&mut *tx).await.map_err(internal)?;
        tx.commit().await.map_err(internal)?;
    }
    let revision = match &b.expected_project_revision {
        Some(r) => r.clone(),
        None => crate::project_bindings::current(state, project)
            .await
            .map_err(internal)?
            .revision
            .to_string(),
    };
    let payload = json!({"operation_id":b.operation_id,"expected_project_revision":revision,"title":b.title,"body":b.body,
        "acceptance_criteria":b.acceptance_criteria,"labels":b.labels,"task_state":b.task_state.as_deref().unwrap_or("active"),
        "parent_id":parent,"priority":b.priority,"task_type":b.task_type});
    let request = json!({"operation_id":b.operation_id,"kind":"task.create","subject":{"type":"project","id":project,"revision":revision},
        "expected_versions":{"project":revision},"payload":payload});
    let request = serde_json::from_value(request).map_err(|_| {
        Refusal::error(
            RefusalCode::InvalidRequest,
            "task.create",
            "The task is malformed.",
        )
    })?;
    Ok(super::human::enqueue::supervisor::agent_create(state, a.ids, request).await)
}
