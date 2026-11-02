//! Actual isolated PostgreSQL upgrade and cancellation proof guard checks.
use aq_server::{
    store::{Store, StoreError},
    test_support::{self, TestDb},
};
use serde_json::Value;
#[tokio::test]
async fn cancellation42_upgrade_preserves_prior_records_and_old41_refuses() {
    let db = TestDb::create().await;
    let pool = db.pool().await;
    test_support::migrate_profile(&pool, 41).await.unwrap();
    let before: Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(t)) FROM global_supervisor_conversation t")
            .fetch_one(&pool)
            .await
            .unwrap();
    let _store = db.store().await;
    let after: Value =
        sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(t)) FROM global_supervisor_conversation t")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    assert!(matches!(
        test_support::migrate_profile(&pool, 41).await,
        Err(StoreError::IncompatibleState {
            found: 42,
            supported: 41
        })
    ));
    Store::connect_with(db.options()).await.unwrap();
}
#[tokio::test]
async fn cancellation42_startup_refuses_missing_or_changed_proof_owners() {
    for mutation in [
        "ALTER TABLE profiled_cancellations DISABLE TRIGGER profiled_cancellations_immutable",
        "ALTER TABLE profiled_cancelled_settlements DISABLE TRIGGER profiled_cancelled_settlement_guard",
        "ALTER TABLE profiled_never_granted_closures DISABLE TRIGGER profiled_cancelled_absence_guard",
        "CREATE OR REPLACE FUNCTION profiled_cancelled_settlement_guard() RETURNS trigger LANGUAGE plpgsql AS 'BEGIN RETURN NEW; END'",
        "CREATE OR REPLACE FUNCTION profiled_close_expired_ungranted(p_node UUID,p_attempt UUID,p_now BIGINT,p_closure UUID,p_operation UUID) RETURNS TABLE(body_bytes BYTEA,body_digest TEXT,changed BOOLEAN) LANGUAGE SQL AS 'SELECT NULL::BYTEA,NULL::TEXT,FALSE'",
    ] {
        let db = TestDb::create().await;
        let _store = db.store().await;
        let pool = db.pool().await;
        sqlx::raw_sql(mutation).execute(&pool).await.unwrap();
        assert!(
            Store::connect_with(db.options()).await.is_err(),
            "accepted drift: {mutation}"
        );
    }
}
