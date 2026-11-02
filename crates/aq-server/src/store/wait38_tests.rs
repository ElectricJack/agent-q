//! Actual fresh/prefix SQL and same-name structural drift, disposable only.
use super::{MIGRATIONS, StoreError, migrate_known};
use crate::test_support::TestDb;
use sqlx::PgPool;
async fn database(version: u32) -> (TestDb, PgPool) {
    let db = TestDb::create().await;
    let pool = db.pool().await;
    migrate_known(&pool, &MIGRATIONS[..version as usize], version)
        .await
        .unwrap();
    (db, pool)
}
async fn table_rows(pool: &PgPool, table: &str) -> serde_json::Value {
    assert!(
        table
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    );
    sqlx::query_scalar(&format!("SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'[]'::jsonb) FROM {table} t")).fetch_one(pool).await.unwrap()
}
#[tokio::test]
async fn reviewed_ddl_catalog_export() {
    let (_db, pool) = database(37).await;
    sqlx::raw_sql(include_str!(
        "../../migrations/0038_verified_wait_owner.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    let actual: serde_json::Value = sqlx::query_scalar(include_str!("wait38_catalog.sql"))
        .fetch_one(&pool)
        .await
        .unwrap();
    if let Ok(path) = std::env::var("AQ_TEST_EXPORT_WAIT38_CATALOG") {
        // Explicit disposable generator only. Never learns runtime guards from
        // an arbitrary existing database and never silently updates the golden.
        std::fs::write(path, serde_json::to_vec_pretty(&actual).unwrap()).unwrap();
    } else {
        assert_eq!(
            actual,
            serde_json::from_str::<serde_json::Value>(include_str!("wait38_catalog.json")).unwrap()
        );
    }
}
#[tokio::test]
async fn fresh38_and_actual37_upgrade_preserve_rows_and_reject_old_reader() {
    let (_db, pool) = database(37).await;
    // Representative retained rows; no delivery or source-proof claim. Preserve
    // every preexisting data table, including empty ones and all old history.
    sqlx::raw_sql("INSERT INTO users(id,email,role,issuer,subject,created_at,bound_at) VALUES('1a3400a0-3196-469c-b9fd-d5244bbb249f','wait38-retained@localhost','owner','dev','wait38-retained@localhost',now(),now()); INSERT INTO mailbox_heads(mailbox_key,next_cursor) VALUES('wait38-retained-legacy',2); INSERT INTO mailbox_messages(message_id,mailbox_key,cursor,thread_id,sender,recipient,payload_digest,body,created_at_ms) VALUES('5dbb77ca-7d10-46f2-8c07-4ab3892b72aa','wait38-retained-legacy',1,'3b228baa-bca0-462d-947d-66a4b0b0a177','{\"kind\":\"owner\"}','{\"kind\":\"task\"}','legacy-retained-digest','Retained legacy fixture',1000)").execute(&pool).await.unwrap();
    let tables:Vec<String>=sqlx::query_scalar("SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename NOT IN('meta','schema_migrations') ORDER BY tablename").fetch_all(&pool).await.unwrap();
    let mut before = Vec::new();
    for table in &tables {
        before.push(table_rows(&pool, table).await);
    }
    migrate_known(&pool, &MIGRATIONS[..38], 38).await.unwrap();
    for (table, expected) in tables.iter().zip(&before) {
        assert_eq!(
            table_rows(&pool, table).await,
            *expected,
            "changed retained table {table}"
        );
    }
    let budget: (i64, i64) = sqlx::query_as(
        "SELECT envelopes,reserved_bytes FROM wait_owner_budgets WHERE scope_key='installation'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(budget, (0, 0));
    assert!(matches!(
        migrate_known(&pool, &MIGRATIONS[..37], 37).await,
        Err(StoreError::IncompatibleState {
            found: 38,
            supported: 37
        })
    ));
    migrate_known(&pool, &MIGRATIONS[..38], 38).await.unwrap();
}
#[tokio::test]
async fn named_guard_replacements_and_required_identity_drift_refuse_startup() {
    let (_db, pool) = database(38).await;
    for mutation in [
        "ALTER TABLE wait_owner_budgets DROP CONSTRAINT wait_owner_budget_capacity; ALTER TABLE wait_owner_budgets ADD CONSTRAINT wait_owner_budget_capacity CHECK(TRUE)",
        "DROP INDEX wait_owner_due_work; CREATE INDEX wait_owner_due_work ON wait_owner_work(wait_id)",
        "ALTER TABLE wait_owner_records DROP CONSTRAINT wait_owner_records_operation_id_key",
        "ALTER TABLE wait_owner_records ALTER COLUMN operation_id DROP NOT NULL",
        "ALTER TABLE wait_owner_records DISABLE TRIGGER wait_owner_record_guard",
        "ALTER TABLE wait_owner_members DISABLE TRIGGER ALL",
        "CREATE OR REPLACE FUNCTION wait38_budget_guard() RETURNS trigger LANGUAGE plpgsql AS 'BEGIN RETURN NEW; END'",
        "ALTER TABLE wait_owner_tasks DROP CONSTRAINT wait_owner_active_wait; ALTER TABLE wait_owner_tasks ADD CONSTRAINT wait_owner_active_wait FOREIGN KEY(active_wait_id) REFERENCES wait_owner_records(wait_id)",
    ] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::raw_sql(mutation).execute(&mut *tx).await.unwrap();
        assert!(
            super::wait38::validate(&mut tx).await.is_err(),
            "accepted {mutation}"
        );
        tx.rollback().await.unwrap();
    }
    migrate_known(&pool, &MIGRATIONS[..38], 38).await.unwrap();
}
