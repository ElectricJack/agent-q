//! Actual disposable SQL owners; no physical-stop or runnable-agent evidence.
mod common;
use aq_server::{
    execution::{
        authority::hot::{HotState, LaunchOwner},
        profiled::Status,
    },
    profiled_execution::expiry::{self, Sweep},
};
use common::profiled::{Fixture, NOW, fixture, insert};
use serde_json::json;
use std::{sync::atomic::Ordering, time::Duration};
use uuid::Uuid;

async fn setup() -> (Fixture, Uuid, sqlx::PgPool) {
    let f = fixture().await;
    let attempt = insert(&f).await;
    let pool = f.db.pool().await;
    sqlx::query("INSERT INTO profiled_authority_hot(attempt_id,state,record_revision,authority_revision,grant_revision) VALUES($1,$2,1,1,0)")
        .bind(attempt).bind(json!(HotState::from_compat(&f.profiled).unwrap())).execute(&pool).await.unwrap();
    (f, attempt, pool)
}
async fn held(pool: &sqlx::PgPool, attempt: Uuid) -> bool {
    sqlx::query_scalar("SELECT holds_task FROM execution_attempts WHERE attempt_id=$1")
        .bind(attempt)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn fresh_offer_remains_held_then_cursor_wrap_closes_exact_expired_owner_once() {
    let (f, attempt, pool) = setup().await;
    let mut sweep = Sweep::default();
    let first = sweep.tick(&f.state).await.unwrap();
    assert_eq!((first.inspected, first.closed, first.refused), (1, 0, 0));
    assert!(held(&pool, attempt).await);
    f.time.store((NOW + 30000) as i64, Ordering::SeqCst);
    assert_eq!(sweep.tick(&f.state).await.unwrap().inspected, 0);
    assert_eq!(sweep.tick(&f.state).await.unwrap().closed, 1);
    assert!(!held(&pool, attempt).await);
    for _ in 0..3 {
        assert_eq!(sweep.tick(&f.state).await.unwrap().closed, 0);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM profiled_never_granted_closures")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn node_lock_refusal_advances_cursor_and_does_not_drop_claim_or_poison_pool() {
    let (f, attempt, pool) = setup().await;
    f.time.store((NOW + 30000) as i64, Ordering::SeqCst);
    let mut owner = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE")
        .bind(f.node.node_id)
        .fetch_one(&mut *owner)
        .await
        .unwrap();
    let mut sweep = Sweep::default();
    let result = tokio::time::timeout(Duration::from_secs(3), sweep.tick(&f.state))
        .await
        .unwrap()
        .unwrap();
    assert_eq!((result.inspected, result.closed, result.refused), (1, 0, 1));
    assert!(held(&pool, attempt).await);
    owner.rollback().await.unwrap();
    assert_eq!(sweep.tick(&f.state).await.unwrap().inspected, 0);
    assert_eq!(sweep.tick(&f.state).await.unwrap().closed, 1);
}

#[tokio::test]
async fn granted_without_started_remains_held_even_after_grant_and_wall_expiry() {
    let (f, attempt, pool) = setup().await;
    let mut hot = HotState::from_compat(&f.profiled).unwrap();
    hot.status = Status::Preparing;
    hot.owner = Some(LaunchOwner {
        guardian_id: Uuid::new_v4(),
        launch_id: Uuid::new_v4(),
        reservation_id: Uuid::new_v4(),
    });
    sqlx::query("UPDATE profiled_authority_hot SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(hot))
        .execute(&pool)
        .await
        .unwrap();
    hot.status = Status::Granted;
    hot.execution_deadline_ms = Some(NOW + 60000);
    hot.grant = Some(aq_session_protocol::ProfiledGrant {
        binding: f.profiled.offer.binding.clone(),
        grant_id: Uuid::new_v4(),
        expires_at_ms: NOW + 1,
    });
    sqlx::query("UPDATE profiled_authority_hot SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(hot))
        .execute(&pool)
        .await
        .unwrap();
    f.time.store((NOW + 120000) as i64, Ordering::SeqCst);
    for _ in 0..3 {
        assert_eq!(Sweep::default().tick(&f.state).await.unwrap().inspected, 0);
    }
    assert!(held(&pool, attempt).await);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM profiled_never_granted_closures")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn configured_runtime_closes_offline_offer_and_obeys_shutdown() {
    let case = common::native_preparation::setup().await;
    let attempt = case.request.acceptance.binding.attempt_id;
    let pool = case.f.db.pool().await;
    case.f.time.fetch_add(30000, Ordering::SeqCst);
    let (stop, receiver) = tokio::sync::watch::channel(false);
    let task = tokio::spawn(expiry::run(case.f.state.clone(), receiver));
    tokio::time::timeout(Duration::from_secs(5), async {
        while held(&pool, attempt).await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    stop.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn unconfigured_runtime_returns_without_closing_fixture_claims() {
    let (f, attempt, pool) = setup().await;
    assert!(f.state.config.profiled_execution.is_none());
    f.time.store((NOW + 30000) as i64, Ordering::SeqCst);
    let (_stop, receiver) = tokio::sync::watch::channel(false);
    tokio::time::timeout(Duration::from_secs(1), expiry::run(f.state, receiver))
        .await
        .unwrap();
    assert!(held(&pool, attempt).await);
}

#[tokio::test]
async fn published_offer_actual_accept_and_expiry_preserve_source_while_revoking_material() {
    let (case, offer) = common::native_offer::published().await;
    let request = common::native_offer::acceptance(&case, &offer);
    let preparation = common::native_offer::accept(&case, &request).await;
    let pool = &case.env.env.pool;
    let attempt = offer.offered.offer.binding.attempt_id;
    // Publication uses a real clock for actual Graph HTTP read stamps. Switch
    // only the subsequent cleanup owner to the original plan's exact expiry.
    let mut state = case.env.env.state.clone();
    let expires = preparation.original.clock.expires_at_ms as i64;
    state.clock =
        std::sync::Arc::new(move || chrono::DateTime::from_timestamp_millis(expires).unwrap());
    let closed = Sweep::default().tick(&state).await.unwrap();
    assert_eq!((closed.inspected, closed.closed, closed.refused), (1, 1, 0));
    assert!(!held(pool, attempt).await);
    let material: String =
        sqlx::query_scalar("SELECT state FROM admission_material_leases WHERE lease_id=$1")
            .bind(preparation.original.materials[0].lease_id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(material, "revoked");
    let linked: i64 =
        sqlx::query_scalar("SELECT count(*) FROM profiled_launch_artifacts WHERE attempt_id=$1")
            .bind(attempt)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(linked > 0);
    let mut tx = pool.begin().await.unwrap();
    let source =
        aq_server::profiled_execution::source_exports::load_linked_locked(&mut tx, attempt)
            .await
            .unwrap();
    assert_eq!(source.manifest().attempt.attempt_id, attempt);
    assert!(!source.fact().canonical_bytes().unwrap().is_empty());
    tx.rollback().await.unwrap();
    let proof: serde_json::Value =
        sqlx::query_scalar("SELECT state FROM profiled_authority_hot WHERE attempt_id=$1")
            .bind(attempt)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(proof["status"], "never_granted");
    assert!(proof["grant"].is_null());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM agent_launch_bindings WHERE credential_verifier IS NOT NULL"
        )
        .fetch_one(pool)
        .await
        .unwrap(),
        0
    );
}
