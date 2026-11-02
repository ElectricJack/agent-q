//! Internal wait startup refuses catalog drift and inconsistent retained indexes.
//! Encryption/source evidence is separately authenticated by configured Owner::open.
use super::{Result, StoreError};
use sqlx::{Postgres, Transaction};
const CATALOG: &str = include_str!("wait38_catalog.sql");
fn corrupt() -> StoreError {
    StoreError::Corrupt("verified wait owner storage or projections inconsistent".into())
}
pub(crate) async fn validate(tx: &mut Transaction<'_, Postgres>) -> Result<()> {
    // A frozen reference is generated only from the exact reviewed migration on
    // fresh disposable PostgreSQL. Runtime never learns guards from its own DB.
    let actual: serde_json::Value = sqlx::query_scalar(CATALOG).fetch_one(&mut **tx).await?;
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("wait38_catalog.json")).map_err(|_| corrupt())?;
    if actual != expected {
        return Err(corrupt());
    }
    let consistent:bool=sqlx::query_scalar("SELECT (SELECT count(*)<=2048 FROM wait_owner_records) AND NOT EXISTS(SELECT 1 FROM wait_owner_budgets b WHERE b.envelopes<>(SELECT count(*) FROM wait_owner_records w WHERE b.scope_key='installation' OR b.scope_key='project/'||w.workspace_id::text||'/'||w.project_id::text) OR b.reserved_bytes<>b.envelopes*1048576 OR (b.scope_key<>'installation' AND b.envelopes=0)) AND EXISTS(SELECT 1 FROM wait_owner_budgets WHERE scope_key='installation') AND NOT EXISTS(SELECT 1 FROM wait_owner_projects p WHERE NOT EXISTS(SELECT 1 FROM wait_owner_records w WHERE w.workspace_id=p.workspace_id AND w.project_id=p.project_id)) AND NOT EXISTS(SELECT 1 FROM wait_owner_tasks t WHERE NOT EXISTS(SELECT 1 FROM wait_owner_records w WHERE w.workspace_id=t.workspace_id AND w.project_id=t.project_id AND w.task_id=t.task_id AND w.lineage_id=t.lineage_id)) AND NOT EXISTS(SELECT 1 FROM wait_owner_tasks t WHERE t.active_wait_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM wait_owner_records w WHERE w.wait_id=t.active_wait_id AND w.workspace_id=t.workspace_id AND w.project_id=t.project_id AND w.task_id=t.task_id AND w.lineage_id=t.lineage_id AND NOT w.retired))")
        .fetch_one(&mut **tx).await?;
    if !consistent {
        return Err(corrupt());
    }
    // This checks SQL child projections, not encrypted source authenticity.
    sqlx::query("SELECT wait38_check_wait(wait_id) FROM wait_owner_records ORDER BY wait_id")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
