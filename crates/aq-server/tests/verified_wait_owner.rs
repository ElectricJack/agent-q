//! Actual AQ08+SQL owner qualification with synthetic launch/source fixtures.
//! No daemon route, execution hold, provider, Graph or physical worker is used.
mod common;
use aq_server::{
    protected_storage::{KeyFile, Keyring, ProtectionConfig},
    waits::{Error, Owner},
};
use aq_wait_engine::{self as engine, wire};
use common::{
    native_started::{self, context},
    profiled::{Fixture, NOW},
};
use sqlx::Row;
use std::{
    os::unix::fs::PermissionsExt,
    sync::{Arc, Condvar, Mutex, atomic::Ordering},
};
use uuid::Uuid;
struct Test {
    f: Fixture,
    owner: Owner,
    keys: Arc<Keyring>,
    _directory: tempfile::TempDir,
    raw: Vec<u8>,
    request: wire::Request,
}
fn policy() -> engine::ResolvedPolicy {
    let body = engine::PolicyBody {
        maximum_age_ms: 500,
        check_interval_ms: 1000,
        retry_base_ms: 1000,
        retry_cap_ms: 2000,
        max_retries: 4,
        max_work_attempts: 16,
    };
    engine::ResolvedPolicy {
        reference: wire::ArtifactRef {
            id: wire::Text::new("internal-test-policy").unwrap(),
            version: wire::Text::new("1").unwrap(),
            digest: wire::canonical_digest(&body).unwrap(),
        },
        body,
    }
}
async fn setup() -> Test {
    let (f, _) = native_started::setup(true).await;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("key");
    std::fs::write(&path, [42u8; 32]).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let keys = Arc::new(
        Keyring::load(&ProtectionConfig {
            active_key_id: "wait-test".into(),
            keys: vec![KeyFile {
                id: "wait-test".into(),
                file: path,
            }],
        })
        .unwrap(),
    );
    let owner = Owner::open_fixture(f.state.clone(), keys.clone(), policy())
        .await
        .unwrap();
    let binding = aq_admission::transport::lifecycle_binding(&f.profiled.offer).unwrap();
    let predicate = wire::Predicate::Single {
        member: wire::Member {
            member_id: Uuid::new_v4(),
            condition: wire::Condition::Timer {
                due_at_ms: NOW + 100,
            },
        },
    };
    let registration = wire::WaitRegistration {
        scope: wire::Scope {
            project_id: binding.project_id,
            workspace_id: f.profiled.offer.manifest.bindings.graph_id,
        },
        binding,
        wait_id: Uuid::new_v4(),
        wait_generation: 1,
        continuation_generation: 1,
        policy: policy().reference,
        predicate_digest: wire::canonical_digest(&predicate).unwrap(),
        predicate,
        preference: wire::WaitPreference::Hibernate,
    };
    let request =
        wire::Request::seal(Uuid::new_v4(), wire::Command::RegisterWait(registration)).unwrap();
    let raw = serde_json::to_vec(&request).unwrap();
    Test {
        f,
        owner,
        keys,
        _directory: directory,
        raw,
        request,
    }
}
fn reg(t: &Test) -> &wire::WaitRegistration {
    let wire::Command::RegisterWait(r) = &t.request.body.command else {
        panic!()
    };
    r
}
async fn register(t: &Test) -> aq_server::waits::Observation {
    t.owner
        .register(
            &context(&t.f).await,
            t.f.profiled.offer.binding.attempt_id,
            &t.raw,
        )
        .await
        .unwrap()
}
async fn cancel(t: &Test) -> Uuid {
    let pool = t.f.db.pool().await;
    let mut hot: aq_server::execution::authority::hot::HotState = serde_json::from_value(
        sqlx::query_scalar("SELECT state FROM profiled_authority_hot WHERE attempt_id=$1")
            .bind(reg(t).binding.attempt_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
    )
    .unwrap();
    let id = Uuid::new_v4();
    hot.status = aq_server::execution::profiled::Status::Cancelling;
    hot.cancellation_id = Some(id);
    sqlx::query("UPDATE profiled_authority_hot SET state=$2 WHERE attempt_id=$1")
        .bind(reg(t).binding.attempt_id)
        .bind(serde_json::json!(hot))
        .execute(&pool)
        .await
        .unwrap();
    id
}
#[tokio::test]
async fn immutable_registration_replay_restart_and_exact_actual_authority() {
    let t = setup().await;
    let first = register(&t).await;
    assert_eq!(first.snapshot.phase(), &engine::Phase::Installing);
    let replay = register(&t).await;
    assert_eq!(first.registration, replay.registration);
    assert_eq!(
        first.snapshot.encode().unwrap(),
        replay.snapshot.encode().unwrap()
    );
    let restored = Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
    assert_eq!(
        restored
            .observe(
                &context(&t.f).await,
                reg(&t).binding.attempt_id,
                reg(&t).wait_id
            )
            .await
            .unwrap()
            .registration,
        first.registration
    );
    let mut whitespace = t.raw.clone();
    whitespace.push(b' ');
    assert!(matches!(
        t.owner
            .register(
                &context(&t.f).await,
                reg(&t).binding.attempt_id,
                &whitespace
            )
            .await,
        Err(Error::Conflict)
    ));
    let mut wrong = t.request.clone();
    if let wire::Command::RegisterWait(r) = &mut wrong.body.command {
        r.binding.task_id = Uuid::new_v4();
    }
    wrong.body_digest = wire::canonical_digest(&wrong.body).unwrap();
    assert!(matches!(
        t.owner
            .register(
                &context(&t.f).await,
                reg(&t).binding.attempt_id,
                &serde_json::to_vec(&wrong).unwrap()
            )
            .await,
        Err(Error::Authority)
    ));
    let rows: (i64, i64) = sqlx::query_as(
        "SELECT envelopes,reserved_bytes FROM wait_owner_budgets WHERE scope_key='installation'",
    )
    .fetch_one(&t.f.db.pool().await)
    .await
    .unwrap();
    assert_eq!(rows, (1, 1048576));
}
#[tokio::test]
async fn actual_clock_install_then_unconditional_reobserve_and_immutable_terminal() {
    let t = setup().await;
    let initial = register(&t).await;
    let r = reg(&t);
    let auth = context(&t.f).await;
    let installed = t
        .owner
        .drive_clock(&auth, r.binding.attempt_id, r.wait_id)
        .await
        .unwrap();
    assert!(installed.snapshot.outcome().is_none());
    assert_eq!(installed.snapshot.phase(), &engine::Phase::Pending);
    t.f.time.store((NOW + 200) as i64, Ordering::SeqCst);
    let terminal = t
        .owner
        .drive_clock(&auth, r.binding.attempt_id, r.wait_id)
        .await
        .unwrap();
    let outcome = terminal.snapshot.outcome().unwrap();
    assert_eq!(outcome.kind, wire::WaitOutcomeKind::Satisfied);
    assert!(terminal.snapshot.resume_intent().is_some());
    assert_eq!(terminal.registration, initial.registration);
    let before = serde_json::to_vec(outcome).unwrap();
    let late = t
        .owner
        .hint(
            &auth,
            r.binding.attempt_id,
            r.wait_id,
            r.predicate.members()[0].member_id,
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_vec(late.snapshot.outcome().unwrap()).unwrap(),
        before
    );
    let gate: Option<Uuid> = sqlx::query_scalar("SELECT active_wait_id FROM wait_owner_tasks")
        .fetch_one(&t.f.db.pool().await)
        .await
        .unwrap();
    assert_eq!(gate, Some(r.wait_id), "outcome is not scheduling release");
    Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
}
#[tokio::test]
async fn full_capacity_cancellation_uses_reserved_slots_without_budget_lock() {
    let t = setup().await;
    register(&t).await;
    let r = reg(&t);
    let pool = t.f.db.pool().await;
    sqlx::query("UPDATE wait_owner_budgets SET envelope_limit=1")
        .execute(&pool)
        .await
        .unwrap();
    let cancelled = cancel(&t).await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT scope_key FROM wait_owner_budgets FOR UPDATE")
        .execute(&mut *held)
        .await
        .unwrap();
    let observed = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        t.owner
            .observe(&context(&t.f).await, r.binding.attempt_id, r.wait_id),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(observed.retired);
    assert_eq!(
        observed.snapshot.outcome().unwrap().kind,
        wire::WaitOutcomeKind::Cancelled
    );
    assert!(observed.snapshot.resume_intent().is_none());
    held.rollback().await.unwrap();
    let row = sqlx::query("SELECT cancel_operation_id,retired FROM wait_owner_records")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.get::<Option<Uuid>, _>("cancel_operation_id"),
        Some(cancelled)
    );
    assert!(row.get::<bool, _>("retired"));
    let filled:i64=sqlx::query_scalar("SELECT count(*) FROM wait_owner_slots WHERE kind IN('cancellation','outcome','completion') AND plaintext_bytes>0").fetch_one(&pool).await.unwrap();
    assert_eq!(filled, 3);
    let bytes: i64 = sqlx::query_scalar(
        "SELECT reserved_bytes FROM wait_owner_budgets WHERE scope_key='installation'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bytes, 1048576);
    Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
}
#[tokio::test]
async fn cancellation_after_satisfaction_retires_resume_without_rewriting_outcome() {
    let t = setup().await;
    register(&t).await;
    let r = reg(&t);
    let auth = context(&t.f).await;
    t.owner
        .drive_clock(&auth, r.binding.attempt_id, r.wait_id)
        .await
        .unwrap();
    t.f.time.store((NOW + 200) as i64, Ordering::SeqCst);
    let before = t
        .owner
        .drive_clock(&auth, r.binding.attempt_id, r.wait_id)
        .await
        .unwrap();
    let bytes = serde_json::to_vec(before.snapshot.outcome().unwrap()).unwrap();
    cancel(&t).await;
    let after = t
        .owner
        .observe(&auth, r.binding.attempt_id, r.wait_id)
        .await
        .unwrap();
    assert!(after.retired);
    assert_eq!(
        serde_json::to_vec(after.snapshot.outcome().unwrap()).unwrap(),
        bytes
    );
    assert!(after.snapshot.resume_intent().is_none());
    Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
}
struct Held {
    entered: Option<tokio::sync::oneshot::Sender<()>>,
    release: Arc<(Mutex<bool>, Condvar)>,
}
impl engine::SourceOwnerPort for Held {
    fn install(&mut self, r: &engine::SourceRequest) -> engine::Result<engine::SourceReply> {
        if let Some(entered) = self.entered.take() {
            let _ = entered.send(());
        }
        let (lock, wake) = &*self.release;
        let mut ready = lock.lock().unwrap();
        while !*ready {
            ready = wake.wait(ready).unwrap();
        }
        Ok(engine::SourceReply::RetryableUnavailable {
            member_id: r.member.member_id,
            reason: wire::Text::new("held synthetic owner unavailable").unwrap(),
        })
    }
    fn reobserve(&mut self, r: &engine::SourceRequest) -> engine::Result<engine::SourceReply> {
        self.install(r)
    }
}
fn release(gate: &Arc<(Mutex<bool>, Condvar)>) {
    let (lock, wake) = &**gate;
    *lock.lock().unwrap() = true;
    wake.notify_all();
}
async fn wait_for_lock(pool: &sqlx::PgPool, pattern: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(1),async {
        loop {
            let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND application_name='aq-wait-owner38' AND wait_event_type='Lock' AND query LIKE $1)")
                .bind(pattern).fetch_one(pool).await.unwrap();
            if blocked {break;}
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.expect("owner reached expected SQL lock barrier");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn held_source_has_no_sql_locks_and_postio_waits_gate_before_node_and_registry() {
    let t = setup().await;
    register(&t).await;
    let r = reg(&t);
    let owner = t.owner.clone();
    let auth = context(&t.f).await;
    let attempt = r.binding.attempt_id;
    let wait = r.wait_id;
    let (entered, seen) = tokio::sync::oneshot::channel();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let callback = Held {
        entered: Some(entered),
        release: gate.clone(),
    };
    let job = tokio::spawn(async move {
        owner
            .drive_fixture(&auth, attempt, wait, Box::new(callback))
            .await
    });
    seen.await.unwrap();
    let pool = t.f.db.pool().await;
    let mut inspect = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(t.f.node.node_id)
        .execute(&mut *inspect)
        .await
        .unwrap();
    sqlx::query("SELECT wait_id FROM wait_owner_records WHERE wait_id=$1 FOR UPDATE NOWAIT")
        .bind(wait)
        .execute(&mut *inspect)
        .await
        .unwrap();
    inspect.rollback().await.unwrap();
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT revision FROM wait_owner_projects FOR UPDATE NOWAIT")
        .execute(&mut *held)
        .await
        .unwrap();
    release(&gate);
    wait_for_lock(&pool, "SELECT revision FROM wait_owner_projects%").await;
    // The actual post-IO transaction is now waiting at the project gate.
    // It must not already own either later authority or registry lock.
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(t.f.node.node_id)
        .execute(&mut *held)
        .await
        .unwrap();
    sqlx::query("SELECT wait_id FROM wait_owner_records WHERE wait_id=$1 FOR UPDATE NOWAIT")
        .bind(wait)
        .execute(&mut *held)
        .await
        .unwrap();
    held.commit().await.unwrap();
    let result = job.await.unwrap().unwrap();
    assert!(result.snapshot.outcome().is_none());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn abort_does_not_release_physical_callback_or_allow_duplicate_work() {
    let t = setup().await;
    register(&t).await;
    let r = reg(&t);
    let owner = t.owner.clone();
    let auth = context(&t.f).await;
    let attempt = r.binding.attempt_id;
    let wait = r.wait_id;
    let (entered, seen) = tokio::sync::oneshot::channel();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let callback = Held {
        entered: Some(entered),
        release: gate.clone(),
    };
    let job = tokio::spawn(async move {
        owner
            .drive_fixture(&auth, attempt, wait, Box::new(callback))
            .await
    });
    seen.await.unwrap();
    job.abort();
    assert!(job.await.unwrap_err().is_cancelled());
    assert!(matches!(
        t.owner
            .drive_clock(&context(&t.f).await, attempt, wait)
            .await,
        Err(Error::Capacity)
    ));
    let installed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM wait_owner_records WHERE phase='installing'")
            .fetch_one(&t.f.db.pool().await)
            .await
            .unwrap();
    assert_eq!(installed, 1);
    release(&gate);
    // No detached callback may later apply source status or an outcome.
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    let current = t
        .owner
        .observe(&context(&t.f).await, attempt, wait)
        .await
        .unwrap();
    assert_eq!(current.snapshot.revision(), 1);
    assert!(current.snapshot.outcome().is_none());
}
#[tokio::test]
async fn encrypted_slot_tampering_refuses_configured_owner_without_deleting_history() {
    let t = setup().await;
    register(&t).await;
    let pool = t.f.db.pool().await;
    let original: i64 = sqlx::query_scalar("SELECT count(*) FROM wait_owner_records")
        .fetch_one(&pool)
        .await
        .unwrap();
    // Simulate privileged disk/DB corruption with self-consistent structural hash.
    sqlx::raw_sql("ALTER TABLE wait_owner_slots DISABLE TRIGGER ALL; UPDATE wait_owner_slots SET ciphertext=set_byte(ciphertext,0,get_byte(ciphertext,0)#1),ciphertext_digest=encode(sha256(set_byte(ciphertext,0,get_byte(ciphertext,0)#1))),'hex') WHERE kind='snapshot'; ALTER TABLE wait_owner_slots ENABLE TRIGGER ALL").execute(&pool).await.unwrap();
    assert!(
        Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM wait_owner_records")
            .fetch_one(&pool)
            .await
            .unwrap(),
        original
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn observed_status_waits_project_gate_before_node_and_registry() {
    let t = setup().await;
    register(&t).await;
    let r = reg(&t);
    let wait = r.wait_id;
    let attempt = r.binding.attempt_id;
    let pool = t.f.db.pool().await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT revision FROM wait_owner_projects FOR UPDATE")
        .execute(&mut *held)
        .await
        .unwrap();
    let owner = t.owner.clone();
    let auth = context(&t.f).await;
    let job = tokio::spawn(async move { owner.observe(&auth, attempt, wait).await });
    wait_for_lock(&pool, "SELECT revision FROM wait_owner_projects%").await;
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE NOWAIT")
        .bind(t.f.node.node_id)
        .execute(&mut *held)
        .await
        .unwrap();
    sqlx::query("SELECT wait_id FROM wait_owner_records WHERE wait_id=$1 FOR UPDATE NOWAIT")
        .bind(wait)
        .execute(&mut *held)
        .await
        .unwrap();
    held.commit().await.unwrap();
    assert!(job.await.unwrap().unwrap().snapshot.outcome().is_none());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mailbox_head_precedes_registry_and_held_head_cannot_revive_expired_grant() {
    let t = setup().await;
    register(&t).await;
    let r = reg(&t);
    let wait = r.wait_id;
    let attempt = r.binding.attempt_id;
    // Exact existing messaging owner's canonical task-recipient key.
    let recipient = serde_json::json!({"kind":"task","workspace_id":r.scope.workspace_id,"project_id":r.scope.project_id,"task_id":r.binding.task_id});
    let key = wire::Digest::of(&aq_agent_api::canonical(&recipient).unwrap())
        .as_str()
        .to_owned();
    let pool = t.f.db.pool().await;
    sqlx::query("INSERT INTO mailbox_heads(mailbox_key) VALUES($1)")
        .bind(&key)
        .execute(&pool)
        .await
        .unwrap();
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT next_cursor FROM mailbox_heads WHERE mailbox_key=$1 FOR UPDATE")
        .bind(key)
        .execute(&mut *held)
        .await
        .unwrap();
    let owner = t.owner.clone();
    let auth = context(&t.f).await;
    let job = tokio::spawn(async move { owner.observe(&auth, attempt, wait).await });
    wait_for_lock(&pool, "SELECT next_cursor FROM mailbox_heads%").await;
    sqlx::query("SELECT wait_id FROM wait_owner_records WHERE wait_id=$1 FOR UPDATE NOWAIT")
        .bind(wait)
        .execute(&mut *held)
        .await
        .unwrap();
    t.f.time.store((NOW + 1001) as i64, Ordering::SeqCst);
    held.commit().await.unwrap();
    assert!(matches!(job.await.unwrap(), Err(Error::Authority)));
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM wait_owner_records")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(revision, 1);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actual_cancellation_during_source_io_wins_before_any_source_outcome() {
    let t = setup().await;
    register(&t).await;
    let r = reg(&t);
    let owner = t.owner.clone();
    let auth = context(&t.f).await;
    let attempt = r.binding.attempt_id;
    let wait = r.wait_id;
    let (entered, seen) = tokio::sync::oneshot::channel();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let callback = Held {
        entered: Some(entered),
        release: gate.clone(),
    };
    let job = tokio::spawn(async move {
        owner
            .drive_fixture(&auth, attempt, wait, Box::new(callback))
            .await
    });
    seen.await.unwrap();
    cancel(&t).await;
    release(&gate);
    let result = job.await.unwrap().unwrap();
    assert!(result.retired);
    assert_eq!(
        result.snapshot.outcome().unwrap().kind,
        wire::WaitOutcomeKind::Cancelled
    );
    assert!(result.snapshot.resume_intent().is_none());
    Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
}
#[tokio::test]
async fn fixed_deadline_survives_rollback_and_full_capacity() {
    let t = setup().await;
    let first = register(&t).await;
    let r = reg(&t);
    let auth = context(&t.f).await;
    let pool = t.f.db.pool().await;
    sqlx::query("UPDATE wait_owner_budgets SET envelope_limit=1")
        .execute(&pool)
        .await
        .unwrap();
    t.f.time.store((NOW + 500) as i64, Ordering::SeqCst);
    let terminal = t
        .owner
        .drive_clock(&auth, r.binding.attempt_id, r.wait_id)
        .await
        .unwrap();
    assert_eq!(
        terminal.snapshot.outcome().unwrap().kind,
        wire::WaitOutcomeKind::DeadlineReached
    );
    assert_eq!(
        terminal.snapshot.deadline_ms(),
        first.snapshot.deadline_ms()
    );
    t.f.time.store((NOW + 20) as i64, Ordering::SeqCst);
    let restored = Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
    let again = restored
        .observe(&auth, r.binding.attempt_id, r.wait_id)
        .await
        .unwrap();
    assert_eq!(again.snapshot.original_created_at_ms(), NOW);
    assert_eq!(again.snapshot.deadline_ms(), NOW + 500);
    assert_eq!(again.snapshot.outcome(), terminal.snapshot.outcome());
}
async fn sibling(t: &Test) -> Fixture {
    // A second actual AQ08 profiled attempt under the same node/project, for a
    // DIFFERENT task. Its retained immutable selection/manifest/source rows are
    // verified by the ordinary authority owner, not supplied as a trusted Binding.
    let mut selection = t.f.selection.clone();
    selection.lineage.task_id = Uuid::new_v4();
    let mut profiled = common::profiled::state_from_selection(&selection, &t.f.node);
    profiled.source = t.f.profiled.source.clone();
    profiled.status = aq_server::execution::profiled::Status::Granted;
    profiled.grant = Some(aq_session_protocol::ProfiledGrant {
        binding: profiled.offer.binding.clone(),
        grant_id: Uuid::new_v4(),
        expires_at_ms: NOW + 1000,
    });
    profiled.execution_deadline_ms = Some(NOW + 30000);
    profiled.validate().unwrap();
    let f = Fixture {
        db: t.f.db.clone(),
        state: t.f.state.clone(),
        node: t.f.node.clone(),
        profiled,
        selection,
        decision: Uuid::new_v4(),
        actor: t.f.actor,
        credential: t.f.credential.clone(),
        time: t.f.time.clone(),
    };
    common::profiled::insert_sibling(&f).await;
    f
}
fn sibling_request(f: &Fixture) -> Vec<u8> {
    let binding = aq_admission::transport::lifecycle_binding(&f.profiled.offer).unwrap();
    let predicate = wire::Predicate::Single {
        member: wire::Member {
            member_id: Uuid::new_v4(),
            condition: wire::Condition::Timer {
                due_at_ms: NOW + 100,
            },
        },
    };
    let r = wire::WaitRegistration {
        scope: wire::Scope {
            project_id: binding.project_id,
            workspace_id: f.profiled.offer.manifest.bindings.graph_id,
        },
        binding,
        wait_id: Uuid::new_v4(),
        wait_generation: 1,
        continuation_generation: 1,
        policy: policy().reference,
        predicate_digest: wire::canonical_digest(&predicate).unwrap(),
        predicate,
        preference: wire::WaitPreference::Hibernate,
    };
    serde_json::to_vec(
        &wire::Request::seal(Uuid::new_v4(), wire::Command::RegisterWait(r)).unwrap(),
    )
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn installation_quota_race_across_actual_distinct_tasks_reserves_exactly_one_envelope() {
    let t = setup().await;
    let other = sibling(&t).await;
    let other_request = sibling_request(&other);
    let pool = t.f.db.pool().await;
    sqlx::query("UPDATE wait_owner_budgets SET envelope_limit=1 WHERE scope_key='installation'")
        .execute(&pool)
        .await
        .unwrap();
    let left = context(&t.f).await;
    let right = context(&other).await;
    let (a, b) = tokio::join!(
        t.owner.register(&left, reg(&t).binding.attempt_id, &t.raw),
        t.owner.register(
            &right,
            other.profiled.offer.binding.attempt_id,
            &other_request
        )
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "one real authority admission must win"
    );
    let row: (i64, i64) = sqlx::query_as(
        "SELECT envelopes,reserved_bytes FROM wait_owner_budgets WHERE scope_key='installation'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (1, 1048576));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM wait_owner_records")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_quota_race_counts_retired_history_and_rolls_back_installation_charge() {
    let t = setup().await;
    register(&t).await;
    cancel(&t).await;
    t.owner
        .observe(
            &context(&t.f).await,
            reg(&t).binding.attempt_id,
            reg(&t).wait_id,
        )
        .await
        .unwrap();
    let left = sibling(&t).await;
    let right = sibling(&t).await;
    let a_request = sibling_request(&left);
    let b_request = sibling_request(&right);
    let pool = t.f.db.pool().await;
    sqlx::query("UPDATE wait_owner_budgets SET envelope_limit=2 WHERE scope_key<>'installation'")
        .execute(&pool)
        .await
        .unwrap();
    let a_auth = context(&left).await;
    let b_auth = context(&right).await;
    let (a, b) = tokio::join!(
        t.owner
            .register(&a_auth, left.profiled.offer.binding.attempt_id, &a_request),
        t.owner
            .register(&b_auth, right.profiled.offer.binding.attempt_id, &b_request)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT envelopes,reserved_bytes FROM wait_owner_budgets ORDER BY scope_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows, vec![(2, 2097152), (2, 2097152)]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM wait_owner_records WHERE retired")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
}
#[tokio::test]
async fn known_cycle_through_wait_to_owner_bridge_refuses_without_reserving() {
    let mut t = setup().await;
    let other = sibling(&t).await;
    if let wire::Command::RegisterWait(r) = &mut t.request.body.command {
        let wire::Predicate::Single { member } = &mut r.predicate else {
            panic!()
        };
        member.condition = wire::Condition::Task {
            task_id: other.selection.lineage.task_id,
            revision: wire::Text::new("1").unwrap(),
            requirement: wire::TaskRequirement::Settled,
        };
        r.predicate_digest = wire::canonical_digest(&r.predicate).unwrap();
    }
    t.request.body_digest = wire::canonical_digest(&t.request.body).unwrap();
    t.raw = serde_json::to_vec(&t.request).unwrap();
    register(&t).await;
    let mut request: wire::Request = serde_json::from_slice(&sibling_request(&other)).unwrap();
    if let wire::Command::RegisterWait(r) = &mut request.body.command {
        let wire::Predicate::Single { member } = &mut r.predicate else {
            panic!()
        };
        member.condition = wire::Condition::WaitOutcome {
            wait: reg(&t).wait_ref(),
        };
        r.predicate_digest = wire::canonical_digest(&r.predicate).unwrap();
    }
    request.body_digest = wire::canonical_digest(&request.body).unwrap();
    let result = t
        .owner
        .register(
            &context(&other).await,
            other.profiled.offer.binding.attempt_id,
            &serde_json::to_vec(&request).unwrap(),
        )
        .await;
    assert!(matches!(result, Err(Error::Engine(engine::Error::Cycle))));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT envelopes FROM wait_owner_budgets WHERE scope_key='installation'"
        )
        .fetch_one(&t.f.db.pool().await)
        .await
        .unwrap(),
        1
    );
    Owner::open_fixture(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
}
#[tokio::test]
async fn default_owner_clock_profile_refuses_graph_and_fixture_history() {
    let t = setup().await;
    let local = Owner::open(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
    let auth = context(&t.f).await;
    let mut request = t.request.clone();
    if let wire::Command::RegisterWait(r) = &mut request.body.command {
        let wire::Predicate::Single { member } = &mut r.predicate else {
            panic!()
        };
        member.condition = wire::Condition::Task {
            task_id: Uuid::new_v4(),
            revision: wire::Text::new("1").unwrap(),
            requirement: wire::TaskRequirement::Settled,
        };
        r.predicate_digest = wire::canonical_digest(&r.predicate).unwrap();
    }
    request.body_digest = wire::canonical_digest(&request.body).unwrap();
    assert!(matches!(
        local
            .register(
                &auth,
                reg(&t).binding.attempt_id,
                &serde_json::to_vec(&request).unwrap()
            )
            .await,
        Err(Error::Unsupported)
    ));
    local
        .register(&auth, reg(&t).binding.attempt_id, &t.raw)
        .await
        .unwrap();
    local
        .drive_clock(&auth, reg(&t).binding.attempt_id, reg(&t).wait_id)
        .await
        .unwrap();
    t.f.time.store((NOW + 200) as i64, Ordering::SeqCst);
    assert_eq!(
        local
            .drive_clock(&auth, reg(&t).binding.attempt_id, reg(&t).wait_id)
            .await
            .unwrap()
            .snapshot
            .outcome()
            .unwrap()
            .kind,
        wire::WaitOutcomeKind::Satisfied
    );
    let profile: String = sqlx::query_scalar("SELECT source_profile FROM wait_owner_records")
        .fetch_one(&t.f.db.pool().await)
        .await
        .unwrap();
    assert_eq!(profile, "local_v1");
    Owner::open(t.f.state.clone(), t.keys.clone(), policy())
        .await
        .unwrap();
    let fixture = setup().await;
    register(&fixture).await;
    assert!(matches!(
        Owner::open(fixture.f.state.clone(), fixture.keys.clone(), policy()).await,
        Err(Error::Unsupported)
    ));
}
