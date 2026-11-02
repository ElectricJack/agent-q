//! Execution result38 only; no wait-owner schema or settlement authority.
use super::{Result, StoreError};
use sqlx::{Postgres, Transaction};
fn corrupt() -> StoreError {
    StoreError::Corrupt("result origin storage/guards or retained joins inconsistent".into())
}
pub(super) async fn validate(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    let actual: serde_json::Value = sqlx::query_scalar(include_str!("result38_catalog.sql"))
        .fetch_one(&mut **tx)
        .await?;
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("result38_catalog.json")).map_err(|_| corrupt())?;
    if actual != expected {
        return Err(corrupt());
    }
    let valid:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM profiled_result_reports r WHERE (r.result_origin_kind='legacy_prepare_v1' AND (r.origin_execution_receipt_digest IS NOT NULL OR r.origin_stopped_ack_bytes_digest IS NOT NULL OR EXISTS(SELECT 1 FROM profiled_v2_accepts a WHERE a.attempt_id=r.attempt_id) OR EXISTS(SELECT 1 FROM profiled_phase_execution_grants g WHERE g.attempt_id=r.attempt_id))) OR (r.result_origin_kind<>'legacy_prepare_v1' AND NOT profiled_result_origin_valid(r.attempt_id,r.node_id,r.result_origin_kind,r.origin_execution_receipt_digest,r.origin_stopped_ack_bytes_digest))))")
        .fetch_one(&mut **tx).await?;
    if !valid {
        return Err(corrupt());
    }
    Ok(())
}
