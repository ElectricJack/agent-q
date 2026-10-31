use super::*;
use aq_selection::{ArtifactRef, CapabilitySnapshot, CredentialBinding, ExecutionTarget};
use axum::http::HeaderMap;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledCredential {
    pub profile: ArtifactRef,
    pub credential_revision: u64,
    pub available: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryReport {
    pub operation_id: Uuid,
    pub boot_id: Uuid,
    pub sequence: u64,
    pub ttl_ms: u64,
    pub qualifications: Vec<ArtifactRef>,
    pub credentials: Vec<InstalledCredential>,
}
pub(super) async fn report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InventoryReport>,
) -> Result<Json<Value>> {
    let node = crate::worker_api::authenticate(&state, &headers).await?;
    if request.operation_id.is_nil()
        || request.boot_id.is_nil()
        || request.sequence == 0
        || request.sequence > i64::MAX as u64
        || !(1000..=120_000).contains(&request.ttl_ms)
        || request.qualifications.len() > 16
        || request.credentials.len() > 32
    {
        return Err(bad("invalid inventory identity/bounds"));
    }
    let payload = aq_selection::digest(&request).map_err(|_| bad("inventory digest"))?;
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
    let current: Value =
        sqlx::query_scalar("SELECT state FROM execution_nodes WHERE node_id=$1 FOR SHARE")
            .bind(node.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
            .ok_or_else(|| conflict("execution boot must be registered"))?;
    if current["identity"]["boot_id"] != request.boot_id.to_string() {
        return Err(conflict("inventory boot is stale"));
    }
    if let Some(row)=sqlx::query("SELECT kind,payload_digest,response FROM execution_receipts WHERE node_id=$1 AND operation_id=$2").bind(node.id).bind(request.operation_id).fetch_optional(&mut *tx).await.map_err(db)? {
        if row.get::<String,_>("kind")!="intelligence_inventory" || row.get::<String,_>("payload_digest")!=payload.as_str(){return Err(conflict("inventory operation ID conflict"));}
        return Ok(Json(row.get("response")));
    }
    let loaded = loaded(&state)?;
    registry::check_current(&state, &mut tx).await?;
    let old = sqlx::query(
        "SELECT boot_id,sequence,revision FROM intelligence_node_snapshots WHERE node_id=$1",
    )
    .bind(node.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(db)?;
    if old.as_ref().is_some_and(|row| {
        row.get::<Uuid, _>("boot_id") == request.boot_id
            && row.get::<i64, _>("sequence") as u64 >= request.sequence
    }) {
        return Err(conflict("inventory sequence must increase"));
    }
    let revision = old
        .as_ref()
        .map(|r| r.get::<i64, _>("revision") as u64)
        .unwrap_or(0)
        .checked_add(1)
        .filter(|v| *v <= i64::MAX as u64)
        .ok_or(AppError::Internal)?;
    let time = now(&state)?;
    let expires = time
        .checked_add(request.ttl_ms)
        .ok_or_else(|| bad("inventory expiry overflow"))?;
    let mut harnesses = Vec::new();
    for qualification in request.qualifications {
        if harnesses
            .iter()
            .any(|h: &aq_selection::InstalledHarness| h.qualification == qualification)
        {
            return Err(bad("duplicate harness qualification"));
        }
        let harness = loaded
            .config
            .qualified_harnesses
            .iter()
            .find(|h| h.qualification == qualification)
            .ok_or_else(|| bad("unrecognized qualified harness"))?;
        harnesses.push(harness.clone());
    }
    let mut credential_bindings = Vec::new();
    for presence in request.credentials {
        if credential_bindings
            .iter()
            .any(|c: &CredentialBinding| c.credential_profile == presence.profile)
        {
            return Err(bad("duplicate installed credential profile"));
        }
        let configured = loaded
            .config
            .credentials
            .iter()
            .find(|c| {
                c.credential.profile == presence.profile
                    && c.credential.revision == presence.credential_revision
            })
            .ok_or_else(|| bad("unknown exact credential revision"))?;
        if !configured.allowed_node_ids.contains(&node.id) {
            return Err(AppError::Forbidden(
                "credential not authorized for node".into(),
            ));
        }
        credential_bindings.push(CredentialBinding {
            credential_revision: presence.credential_revision,
            credential_profile: presence.profile,
            available: presence.available,
            observed_at_ms: time,
            expires_at_ms: expires,
        });
    }
    let snapshot = CapabilitySnapshot {
        target: ExecutionTarget::Worker {
            node_id: node.id,
            boot_id: request.boot_id,
        },
        revision,
        observed_at_ms: time,
        expires_at_ms: expires,
        harnesses,
        credential_bindings,
    };
    let value = serde_json::to_value(snapshot).map_err(|_| AppError::Internal)?;
    registry::machine_quota(&mut tx, node.id, &value).await?;
    sqlx::query("INSERT INTO intelligence_node_snapshots(node_id,boot_id,sequence,revision,snapshot) VALUES($1,$2,$3,$4,$5) ON CONFLICT(node_id) DO UPDATE SET boot_id=excluded.boot_id,sequence=excluded.sequence,revision=excluded.revision,snapshot=excluded.snapshot").bind(node.id).bind(request.boot_id).bind(request.sequence as i64).bind(revision as i64).bind(&value).execute(&mut *tx).await.map_err(db)?;
    sqlx::query("INSERT INTO execution_receipts(node_id,operation_id,kind,payload_digest,response) VALUES($1,$2,'intelligence_inventory',$3,$4)").bind(node.id).bind(request.operation_id).bind(payload.as_str()).bind(&value).execute(&mut *tx).await.map_err(db)?;
    tx.commit().await.map_err(db)?;
    Ok(Json(value))
}
