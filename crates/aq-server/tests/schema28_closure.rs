//! Actual ordered schema28; no runtime activation or simulated old-version label.
mod common;
use aq_server::{
    store::Store,
    test_support::{TestDb, migrate_profile},
};
#[tokio::test]
async fn fresh_closure_schema_is_empty_and_default_router_has_no_owner() {
    use tower::ServiceExt;
    let db = TestDb::create().await;
    let store = db.store().await;
    let pool = db.pool().await;
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "28"
    );
    for table in [
        "profiled_native_registration_history",
        "profiled_launch_registrations",
        "profiled_launch_closures",
    ] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }
    let mut config:aq_server::config::Config=toml::from_str("format_version=1\nenvironment='development'\n[server]\nadmin_bind='127.0.0.1:0'\nworker_bind='127.0.0.1:0'\npublic_origin='http://127.0.0.1:8080'\n[auth]\nmode='dev'\ndev_email='nobody@example.invalid'\n[bootstrap]\nenabled=false\n").unwrap();
    config.profiled_execution = None;
    let state = aq_server::AppState {
        store: store.clone(),
        config: std::sync::Arc::new(config),
        identity: std::sync::Arc::new(aq_server::identity::DevIdentity::new(
            "nobody@example.invalid".into(),
        )),
        clock: std::sync::Arc::new(chrono::Utc::now),
        agent_access: None,
        source_retention: None,
    };
    let response = aq_server::build_worker_router(state)
        .oneshot(
            axum::http::Request::post("/v1/worker/profiled/launch-closure")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
}
#[tokio::test]
async fn actual27_upgrades_to28_and_collision_rolls_back_before_explicit_retry() {
    let db = TestDb::create().await;
    let pool = db.pool().await;
    migrate_profile(&pool, 27).await.unwrap();
    sqlx::query("CREATE TABLE profiled_launch_closures(fixture INTEGER)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(Store::connect_with(db.options()).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "27"
    );
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT to_regclass('public.profiled_native_registration_history') IS NOT NULL"
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        27
    );
    sqlx::query("DROP TABLE profiled_launch_closures")
        .execute(&pool)
        .await
        .unwrap();
    drop(Store::connect_with(db.options()).await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "28"
    );
    assert!(matches!(
        migrate_profile(&pool, 27).await,
        Err(aq_server::store::StoreError::IncompatibleState {
            found: 28,
            supported: 27
        })
    ));
}
