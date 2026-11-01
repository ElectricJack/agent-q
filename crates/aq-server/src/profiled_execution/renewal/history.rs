//! Immutable API-owned renewal receipts; transport stamps are not retained as
//! authority. Same-privilege direct SQL remains the trusted operator boundary.
use super::health::Evidence;
use crate::error::AppError;
use aq_admission::{
    Digest, LaunchBinding,
    control::{DeliveryStamp, RenewedAuthority, Versions},
    transport::{DeliveryClock, Renew},
};
use aq_session_protocol::Validate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
pub const MAX_PER_ATTEMPT: usize = 256;
pub const MAX_PER_NODE: i64 = 4096;
pub const MAX_GLOBAL: i64 = 65536;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Body {
    pub operation_id: Uuid,
    pub binding: LaunchBinding,
    pub grant_id: Uuid,
    pub reservation_id: Uuid,
    pub guardian_id: Uuid,
    pub launch_id: Uuid,
    pub journal_revision: u64,
    pub execution_deadline_ms: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub versions: Versions,
    pub health: Evidence,
    pub registry_revision: u64,
    pub inventory_revision: u64,
    pub binding_observation_digest: Digest,
    pub task_observation_digest: Digest,
    pub source_observation_digest: Digest,
    pub material_observed_at_ms: u64,
    pub task_observed_at_ms: u64,
    pub task_received_at_ms: u64,
    pub task_expires_at_ms: u64,
    pub source_observed_at_ms: u64,
    pub source_expires_at_ms: u64,
    pub binding_observed_at_ms: u64,
    pub binding_expires_at_ms: u64,
    pub inventory_expires_at_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stored {
    schema_version: u32,
    kind: Kind,
    pub body: Body,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Renewed,
}
fn invalid() -> AppError {
    AppError::Conflict("renewal receipt identity or history changed".into())
}
impl Stored {
    pub fn new(body: Body) -> Result<Self, AppError> {
        let result = Self {
            schema_version: 1,
            kind: Kind::Renewed,
            body,
        };
        result.validate()?;
        Ok(result)
    }
    fn validate(&self) -> Result<(), AppError> {
        let b = &self.body;
        b.binding.validate().map_err(|_| invalid())?;
        b.versions.validate().map_err(|_| invalid())?;
        if self.schema_version != 1
            || [
                b.operation_id,
                b.grant_id,
                b.reservation_id,
                b.guardian_id,
                b.launch_id,
            ]
            .iter()
            .any(Uuid::is_nil)
            || b.journal_revision == 0
            || b.journal_revision > i64::MAX as u64
            || b.versions.grant_revision == 0
            || b.issued_at_ms == 0
            || b.expires_at_ms <= b.issued_at_ms
            || b.expires_at_ms > b.execution_deadline_ms
            || b.registry_revision == 0
            || b.inventory_revision == 0
            || b.material_observed_at_ms == 0
            || b.material_observed_at_ms > b.issued_at_ms
            || b.task_observed_at_ms == 0
            || b.task_observed_at_ms > b.task_received_at_ms
            || b.task_received_at_ms > b.issued_at_ms
            || b.source_observed_at_ms == 0
            || b.source_observed_at_ms > b.issued_at_ms
            || b.binding_observed_at_ms == 0
            || b.binding_observed_at_ms > b.issued_at_ms
            || b.expires_at_ms > b.task_expires_at_ms
            || b.expires_at_ms > b.source_expires_at_ms
            || b.expires_at_ms > b.binding_expires_at_ms
            || b.expires_at_ms > b.inventory_expires_at_ms
            || b.health.matching_sources > 1024
            || b.health.evaluated_at_ms == 0
            || b.health.evaluated_at_ms > b.issued_at_ms
            || b.health.state == super::health::State::Denied
        {
            return Err(invalid());
        }
        if serde_json::to_vec(self)
            .map_err(|_| AppError::Internal)?
            .len()
            > 16384
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub fn decode(value: Value) -> Result<Self, AppError> {
        let result: Self = serde_json::from_value(value).map_err(|_| invalid())?;
        result.validate()?;
        Ok(result)
    }
    pub fn matches(&self, request: &Renew) -> bool {
        let b = &self.body;
        b.operation_id == request.operation_id
            && b.binding == request.binding
            && b.grant_id == request.grant_id
            && b.reservation_id == request.reservation_id
            && b.guardian_id == request.guardian_id
            && b.launch_id == request.launch_id
            && b.journal_revision == request.journal_revision
    }
    pub fn deliver(&self, nonce: Uuid, now: u64) -> Result<RenewedAuthority, AppError> {
        self.validate()?;
        let b = &self.body;
        if now < b.issued_at_ms {
            return Err(invalid());
        }
        let reply = RenewedAuthority {
            schema_version: 1,
            operation_id: b.operation_id,
            binding: b.binding.clone(),
            grant_id: b.grant_id,
            reservation_id: b.reservation_id,
            guardian_id: b.guardian_id,
            launch_id: b.launch_id,
            journal_revision: b.journal_revision,
            execution_deadline_ms: b.execution_deadline_ms,
            versions: b.versions.clone(),
            delivery: DeliveryStamp {
                nonce,
                clock: DeliveryClock::new(now, b.expires_at_ms).map_err(|_| invalid())?,
            },
        };
        aq_admission::control::ControlReply::Renewed(reply.clone())
            .validate_shape()
            .map_err(|_| invalid())?;
        Ok(reply)
    }
}
pub async fn lookup(
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    request: &Renew,
) -> Result<Option<Stored>, AppError> {
    let row=sqlx::query("SELECT kind,request_digest,response FROM profiled_execution_receipts WHERE node_id=$1 AND operation_id=$2 FOR SHARE").bind(node).bind(request.operation_id).fetch_optional(&mut **tx).await.map_err(|_|AppError::Internal)?;
    let Some(row) = row else { return Ok(None) };
    if row.get::<String, _>("kind") != "renew"
        || row.get::<String, _>("request_digest")
            != aq_selection::digest(request)
                .map_err(|_| AppError::Internal)?
                .as_str()
    {
        return Err(invalid());
    }
    let stored = Stored::decode(row.get("response"))?;
    if !stored.matches(request) {
        return Err(invalid());
    }
    Ok(Some(stored))
}
/// The counter is acknowledged by the server, not a claimed worker journal
/// digest. Closure callers combine it with real physical checkpoint counters.
/// No unknown table/format is interpreted as an empty history.
pub(crate) async fn latest_renewal_revision_locked(
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    binding: &LaunchBinding,
) -> Result<Option<u64>, AppError> {
    binding.validate().map_err(|_| invalid())?;
    if !matches!(binding.session,aq_admission::SessionIdentity::Remote{node_id,..}if node_id==node)
    {
        return Err(invalid());
    }
    // The node PK prefix is bounded by this API's finite node quota. LIMIT257
    // additionally bounds decoding/rows for one attempt even on corrupt data.
    let rows=sqlx::query("SELECT response FROM profiled_execution_receipts WHERE node_id=$1 AND kind='renew' AND (COALESCE(response#>>'{body,binding,attempt_id}',response#>>'{body,grant,binding,attempt_id}')=$2 OR COALESCE(response#>>'{body,binding,attempt_id}',response#>>'{body,grant,binding,attempt_id}') IS NULL) ORDER BY operation_id LIMIT 257 FOR SHARE").bind(node).bind(binding.attempt_id.to_string()).fetch_all(&mut **tx).await.map_err(|_|AppError::Internal)?;
    if rows.len() > MAX_PER_ATTEMPT {
        return Err(invalid());
    }
    let mut latest = None;
    for row in rows {
        let stored = Stored::decode(row.get("response"))?;
        if stored.body.binding != *binding {
            return Err(invalid());
        }
        latest = Some(latest.unwrap_or(0_u64).max(stored.body.journal_revision));
    }
    Ok(latest)
}
pub async fn reserve_quota(
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    binding: &LaunchBinding,
    stored: &Stored,
) -> Result<Value, AppError> {
    stored.validate()?;
    let value = serde_json::to_value(stored).map_err(|_| AppError::Internal)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('aq-native-renewal-history-v1',0))")
        .execute(&mut **tx)
        .await
        .map_err(|_| AppError::Internal)?;
    let incoming: i64 = sqlx::query_scalar("SELECT (octet_length($1::JSONB::TEXT)+512)::BIGINT")
        .bind(&value)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| AppError::Internal)?;
    let (total,local,attempt,total_bytes,local_bytes):(i64,i64,i64,i64,i64)=sqlx::query_as("SELECT count(*),count(*) FILTER(WHERE node_id=$1),count(*) FILTER(WHERE node_id=$1 AND response#>>'{body,binding,attempt_id}'=$2),COALESCE(sum(octet_length(response::TEXT)+512),0)::BIGINT,COALESCE(sum(octet_length(response::TEXT)+512) FILTER(WHERE node_id=$1),0)::BIGINT FROM profiled_execution_receipts WHERE kind='renew'").bind(node).bind(binding.attempt_id.to_string()).fetch_one(&mut **tx).await.map_err(|_|AppError::Internal)?;
    if incoming > 32768
        || total >= MAX_GLOBAL
        || local >= MAX_PER_NODE
        || attempt >= MAX_PER_ATTEMPT as i64
        || total_bytes
            .checked_add(incoming)
            .is_none_or(|n| n > 67_108_864)
        || local_bytes
            .checked_add(incoming)
            .is_none_or(|n| n > 8_388_608)
    {
        return Err(AppError::UpdatesUnavailable(
            "retained renewal history capacity reached".into(),
        ));
    }
    Ok(value)
}
pub async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    node: Uuid,
    request: &Renew,
    response: Value,
) -> Result<(), AppError> {
    sqlx::query("INSERT INTO profiled_execution_receipts(node_id,operation_id,kind,request_digest,response) VALUES($1,$2,'renew',$3,$4)").bind(node).bind(request.operation_id).bind(aq_selection::digest(request).map_err(|_|AppError::Internal)?.as_str()).bind(response).execute(&mut **tx).await.map_err(|_|AppError::Internal)?;
    Ok(())
}
