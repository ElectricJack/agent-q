//! Spec 0006 phase 0: the question, comment and wait routes check the grant
//! the attempt holds, record the decision and refuse with the reason. The
//! decision itself is `aq-agent-grants`.
use super::*;
use aq_agent_grants::{AttemptIds, AuditEntry, Decision, GrantSet, Holder, Refusal};

/// Seal `grants` beside a new launch binding, in its transaction.
pub(super) async fn seal(
    tx: &mut Transaction<'_, Postgres>,
    attempt: Uuid,
    g: &GrantSet,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO agent_launch_grants(attempt_id,grants,grants_digest) VALUES($1,$2,$3)",
    )
    .bind(attempt)
    .bind(serde_json::to_value(g)?)
    .bind(g.digest())
    .execute(&mut **tx)
    .await?;
    Ok(())
}
/// The sealed grants. No row (sealed before grants existed) holds none.
pub(super) fn sealed(column: Option<Value>) -> Result<GrantSet> {
    column.map_or(Ok(GrantSet::empty()), |v| {
        GrantSet::new(serde_json::from_value(v)?).map_err(|_| Error("internal"))
    })
}
fn ids(p: &Principal) -> Result<AttemptIds> {
    Ok(AttemptIds {
        attempt_id: id(&p.binding, "attempt_id")?,
        task_id: id(&p.binding, "task_id")?,
        project_id: id(&p.binding, "project_id")?,
    })
}
/// Check the grant `path` needs for `p`; Err is the refusal response.
pub(super) async fn require(
    state: &AppState,
    headers: &HeaderMap,
    p: &Principal,
    path: &str,
) -> std::result::Result<(), Response> {
    let request_id = Uuid::new_v4();
    let ids = ids(p).map_err(|e| e.respond(request_id))?;
    let holder = Holder {
        project: ids.project_id,
    };
    let (command, grant, target) = aq_agent_grants::route_grant(path, holder)
        .ok_or_else(|| Error("internal").respond(request_id))?;
    let decision = aq_agent_grants::check(&p.grants, holder, command, grant, target);
    let entry = AuditEntry::agent(ids, command, grant, target, &decision, request_id);
    audit(state, &entry).await;
    match decision {
        Decision::Allow(_) => Ok(()),
        Decision::Deny(refusal) => Err(refused(headers, refusal, request_id)),
    }
}
/// One `command_audit` row. Best effort: a failed audit write is logged and
/// never turns a refusal into an allow or an allow into an error.
pub(crate) async fn audit(state: &AppState, e: &AuditEntry) {
    let r = sqlx::query("INSERT INTO command_audit(audit_id,recorded_at_ms,caller_kind,caller_id,attempt_id,task_id,project_id,command,grant_operation,target,decision,reason_code,operation_id,request_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
        .bind(Uuid::new_v4()).bind(now(state) as i64).bind(e.caller_kind).bind(e.caller_id)
        .bind(e.attempt_id).bind(e.task_id).bind(e.project_id).bind(&e.command).bind(&e.grant)
        .bind(&e.target).bind(if e.allowed { "allow" } else { "deny" }).bind(e.reason_code)
        .bind(e.operation_id).bind(e.request_id)
        .execute(&state.store.pool).await;
    if let Err(error) = r {
        tracing::warn!(%error, command = %e.command, "command audit row not recorded");
    }
}
/// The §6.8 body for a client that sends `X-AQ-Refusal-Version: 1`; otherwise
/// the closed agent-access-1 `ErrorResponse`, whose message names the grant.
pub(crate) fn refused(headers: &HeaderMap, r: Refusal, request_id: Uuid) -> Response {
    let status = StatusCode::from_u16(r.code.status()).unwrap_or(StatusCode::FORBIDDEN);
    if headers
        .get("X-AQ-Refusal-Version")
        .is_some_and(|v| v == "1")
    {
        return response(status, r.with_request(request_id).body());
    }
    response(
        status,
        json!({"api_version":1,"request_id":request_id,"operation_id":null,"code":r.code.legacy(),"message":r.message,"effect":"not_applied","retryable":false,"recovery":null}),
    )
}
