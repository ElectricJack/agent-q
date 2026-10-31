use super::*;
use aq_selection::{ResolvedSelection, StructuredFailure, derive_worker_observation};
use axum::http::HeaderMap;
pub(super) async fn report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<StructuredFailure>,
) -> Result<Json<Value>> {
    let node = crate::worker_api::authenticate(&state, &headers).await?;
    if request.schema_version != 1
        || request.node_id != node.id
        || request.operation_id.is_nil()
        || request.attempt_id.is_nil()
        || request.sequence == 0
        || request.sequence > i64::MAX as u64
    {
        return Err(bad("failure identity/sequence"));
    }
    let payload = aq_selection::digest(&request).map_err(|_| bad("failure digest"))?;
    let mut tx = state.store.pool.begin().await.map_err(db)?;
    let revoked: Option<bool> =
        sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM nodes WHERE id=$1 FOR UPDATE")
            .bind(node.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
    if revoked != Some(false) {
        return Err(AppError::CredentialRevoked);
    }
    let registration: Value =
        sqlx::query_scalar("SELECT state FROM execution_nodes WHERE node_id=$1 FOR SHARE")
            .bind(node.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
            .ok_or_else(|| conflict("execution boot absent"))?;
    if registration["identity"]["boot_id"] != request.boot_id.to_string() {
        return Err(conflict("failure boot stale"));
    }
    if let Some(row)=sqlx::query("SELECT kind,payload_digest,response FROM execution_receipts WHERE node_id=$1 AND operation_id=$2").bind(node.id).bind(request.operation_id).fetch_optional(&mut *tx).await.map_err(db)? {
        if row.get::<String,_>("kind")!="intelligence_failure" || row.get::<String,_>("payload_digest")!=payload.as_str(){return Err(conflict("failure operation ID conflict"));}
        return Ok(Json(row.get("response")));
    }
    let attempt = crate::execution::authority::load_attempt_locked(
        &state,
        &mut tx,
        node.id,
        request.attempt_id,
        crate::execution::authority::Policy::GraphActive,
    )
    .await
    .map_err(|_| {
        conflict(
            "attempt has no actual sealed profiled authority; legacy fixture feedback is refused",
        )
    })?;
    if attempt.kind != crate::execution::authority::Kind::ProfiledV1 {
        return Err(conflict(
            "attempt has no actual sealed profiled selection; legacy fixture feedback is refused",
        ));
    }
    let sealed: Value = sqlx::query_scalar(
        "SELECT selection FROM intelligence_attempt_selections WHERE attempt_id=$1 FOR SHARE",
    )
    .bind(request.attempt_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db)?
    .ok_or_else(|| {
        conflict(
            "attempt has no actual sealed profiled selection; legacy fixture feedback is refused",
        )
    })?;
    let sealed: ResolvedSelection =
        serde_json::from_value(sealed).map_err(|_| AppError::Internal)?;
    let revision = registry::check_current(&state, &mut tx)
        .await?
        .checked_add(1)
        .filter(|r| *r <= i64::MAX as u64)
        .ok_or(AppError::Internal)?;
    let time = now(&state)?;
    if attempt.status != crate::execution::profiled::Status::Granted
        || time >= attempt.grant_expires_at_ms
        || attempt.identity.boot_id != request.boot_id
    {
        return Err(conflict(
            "failure attempt is not currently granted to this boot",
        ));
    }
    let observation = derive_worker_observation(
        &request,
        &aq_selection::FailureContext {
            observation_id: Uuid::new_v4(),
            authenticated_node: node.id,
            current_boot: request.boot_id,
            sealed_attempt: request.attempt_id,
            sealed: &sealed,
            revision,
            now_ms: time,
            ttl_ms: 30_000,
            cooldown_ms: 30_000,
        },
    )
    .map_err(|_| bad("failure lacks exact sealed classifier/selection qualification"))?;
    let current = loaded(&state)?.credentials.iter().find(|c| {
        c.profile == observation.scope.credential_profile
            && c.revision == observation.scope.credential_revision
            && c.provider == observation.scope.provider
            && c.account_reference == observation.scope.account_reference
            && c.endpoint_reference == observation.scope.endpoint_reference
            && c.enabled
    });
    if current.is_none() {
        return Err(conflict("failure credential revision no longer current"));
    }
    let key = aq_selection::digest(&(&observation.scope, &observation.source))
        .map_err(|_| AppError::Internal)?;
    let previous: Option<i64> = sqlx::query_scalar(
        "SELECT sequence FROM intelligence_observations WHERE scope_source_digest=$1",
    )
    .bind(key.as_str())
    .fetch_optional(&mut *tx)
    .await
    .map_err(db)?;
    if previous.is_some_and(|v| request.sequence <= v as u64) {
        return Err(conflict("failure sequence must increase"));
    }
    let value = serde_json::to_value(observation).map_err(|_| AppError::Internal)?;
    registry::quota(&mut tx, previous.is_none(), &value).await?;
    registry::machine_quota(&mut tx, node.id, &value).await?;
    sqlx::query("INSERT INTO intelligence_observations(scope_source_digest,sequence,observation) VALUES($1,$2,$3) ON CONFLICT(scope_source_digest) DO UPDATE SET sequence=excluded.sequence,observation=excluded.observation").bind(key.as_str()).bind(request.sequence as i64).bind(&value).execute(&mut *tx).await.map_err(db)?;
    sqlx::query("UPDATE intelligence_registry SET revision=$1 WHERE singleton=TRUE")
        .bind(revision as i64)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    sqlx::query("INSERT INTO execution_receipts(node_id,operation_id,kind,payload_digest,response) VALUES($1,$2,'intelligence_failure',$3,$4)").bind(node.id).bind(request.operation_id).bind(payload.as_str()).bind(&value).execute(&mut *tx).await.map_err(db)?;
    if now(&state)? >= attempt.grant_expires_at_ms {
        return Err(conflict("failure grant expired during lock wait"));
    }
    tx.commit().await.map_err(db)?;
    Ok(Json(value))
}
