//! Actual ordered state29 and rollback from a real state28 database.
use aq_server::{
    store::Store,
    test_support::{TestDb, migrate_profile},
};
#[tokio::test]
async fn fresh_scoped_storage_is_empty_and_quotas_start_at_zero() {
    let db = TestDb::create().await;
    let _store = db.store().await;
    let pool = db.pool().await;
    let version: String =
        sqlx::query_scalar("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, aq_server::STATE_FORMAT_VERSION.to_string());
    for table in [
        "profiled_presence_operations",
        "profiled_inventory_semantic_heads",
        "profiled_material_presence_heads",
        "profiled_presence_node_storage",
    ] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT operations,logical_bytes,presence_heads FROM profiled_presence_storage WHERE singleton").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 0, 0));
}
#[tokio::test]
async fn actual28_upgrade_collision_rolls_back_all29_then_explicit_retry_succeeds() {
    let db = TestDb::create().await;
    let pool = db.pool().await;
    migrate_profile(&pool, 28).await.unwrap();
    sqlx::query("CREATE TABLE profiled_presence_operations(fixture INTEGER)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(Store::connect_with(db.options()).await.is_err());
    let version: String =
        sqlx::query_scalar("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, "28");
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.profiled_presence_storage') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!exists);
    sqlx::query("DROP TABLE profiled_presence_operations")
        .execute(&pool)
        .await
        .unwrap();
    drop(Store::connect_with(db.options()).await.unwrap());
    let version: String =
        sqlx::query_scalar("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, aq_server::STATE_FORMAT_VERSION.to_string());
    assert!(
        matches!(migrate_profile(&pool,28).await,Err(aq_server::store::StoreError::IncompatibleState{found,supported:28})if found==aq_server::STATE_FORMAT_VERSION)
    );
}
