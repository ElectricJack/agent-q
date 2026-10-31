//! Assembled startup boundary: ordinary daemon construction never invents a
//! configured owner, and missing ordered review schema cannot start an engine.
mod common;
use aq_server::{
    playbooks::{
        driver::Driver,
        service::{Namespace, Service},
    },
    protected_storage::{KeyFile, Keyring, ProtectionConfig},
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::profiled::{fixture, insert};
use std::{
    os::unix::fs::PermissionsExt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tower::ServiceExt;
use uuid::Uuid;
#[tokio::test]
async fn ordinary_human_router_keeps_review_and_playbook_owners_unmounted() {
    let f = fixture().await;
    let app = aq_server::build_admin_router(f.state.clone());
    for path in [
        format!("/api/reviews/decisions/{}", Uuid::new_v4()),
        "/api/playbooks/status".into(),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get(path)
                    .header("x-aq-dev-email", "owner@localhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let response = app
        .oneshot(
            Request::get("/api/v1/bootstrap")
                .header("x-aq-dev-email", "owner@localhost")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let pool = f.db.pool().await;
    let version: String =
        sqlx::query_scalar("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(version, aq_server::STATE_FORMAT_VERSION.to_string());
    let migrations: Vec<i32> =
        sqlx::query_scalar("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        migrations,
        (1..=aq_server::STATE_FORMAT_VERSION as i32).collect::<Vec<_>>()
    );
    let review: bool =
        sqlx::query_scalar("SELECT to_regclass('public.review_owner_budget') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(review, aq_server::STATE_FORMAT_VERSION >= 26);
}
#[tokio::test]
async fn absent_review_schema_refuses_namespace_without_starting_engine_or_reseeding() {
    let f = fixture().await;
    insert(&f).await;
    let pool = f.db.pool().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("key");
    std::fs::write(&path, [9; 32]).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let keys = Keyring::load(&ProtectionConfig {
        active_key_id: "test".into(),
        keys: vec![KeyFile {
            id: "test".into(),
            file: path,
        }],
    })
    .unwrap();
    let namespace = Uuid::new_v4();
    let service = Service::new_profile2(
        &f.state,
        Namespace {
            namespace_id: namespace,
            generation: 1,
            name: format!("startup-{namespace}"),
            workspace_id: f.selection.lineage.workspace_id,
            project_id: f.selection.lineage.project_id,
            owner_user_id: f.actor,
        },
        Arc::new(keys),
    )
    .await
    .unwrap();
    if aq_server::STATE_FORMAT_VERSION >= 26 {
        service.initialize(f.actor).await.unwrap();
        return;
    }
    assert!(service.initialize(f.actor).await.is_err());
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM playbook_namespaces WHERE namespace_id=$1")
            .bind(namespace)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 0);
    let called = Arc::new(AtomicBool::new(false));
    let captured = called.clone();
    let result = Driver::start::<aq_playbook_host::postgres_storage::PostgresStorage, _>(
        service,
        aq_playbook_host::Limits::default(),
        move || {
            captured.store(true, Ordering::SeqCst);
            Err(aq_playbook_host::Error::Invalid)
        },
    )
    .await;
    assert!(result.is_err());
    assert!(!called.load(Ordering::SeqCst));
    let schema: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname='aq_playbooks')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!schema);
}
