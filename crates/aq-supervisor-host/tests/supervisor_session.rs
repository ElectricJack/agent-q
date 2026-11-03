use anyhow::Result;
use aq_central_authority::{owner::Snapshot, *};
use aq_session_host::{
    AdapterLaunch, HarnessReadiness, HostAdapter, HostInput, HostObservation, HostPlan,
    HostStartup, HostSubject, LocalCommand, LocalEnvironment, ResourcePolicy,
};
use aq_session_protocol::LaunchManifest;
use aq_supervisor_host::{PlanAuthority, Supervisor};
use std::{path::Path, sync::Arc, time::Duration};
use uuid::Uuid;

struct Admission;
#[async_trait::async_trait]
impl PlanAuthority for Admission {
    async fn authorize(
        &self,
        _: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _: &CentralPlan,
        _: &HostPlan,
    ) -> Result<()> {
        Ok(())
    }
}
struct FixtureAdapter;
impl HostAdapter for FixtureAdapter {
    fn prepare(&self, p: &HostPlan, _: &Path) -> Result<AdapterLaunch> {
        Ok(AdapterLaunch {
            resource_qualification: None,
            manifest_digest:p.digest()?, adapter:p.harness.adapter.clone(),cli_version:p.harness.cli_version.as_str().into(),
            environment:p.harness.environment.clone(),capabilities:p.harness.required_capabilities.clone(),
            command:LocalCommand {executable:"/bin/sh".into(),arguments:vec!["-c".into(),"cat \"$AQ_SESSION_DIRECTORY/context-0.bin\" > \"$AQ_SESSION_DIRECTORY/delivered\"; exec /bin/sleep 60".into()], environment:Default::default()},
        })
    }
    fn readiness(&self, _: &HostObservation) -> Result<HarnessReadiness> {
        Ok(HarnessReadiness::Unknown)
    }
}
fn pin(name: &str) -> ArtifactRef {
    ArtifactRef {
        id: Text::new(name).unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(name.as_bytes()),
    }
}
struct Case {
    _db: aq_server::test_support::TestDb,
    pool: sqlx::PgPool,
    root: tempfile::TempDir,
    local: LocalEnvironment,
    central: CentralPlan,
    host: HostPlan,
}
impl Case {
    async fn new() -> Self {
        let db = aq_server::test_support::TestDb::create().await;
        drop(db.store().await);
        let pool = db.pool().await;
        let root = tempfile::Builder::new()
            .prefix("aqsh-")
            .tempdir_in("/tmp")
            .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let examples: serde_json::Value = serde_json::from_str(include_str!(
            "../../aq-central-authority/artifacts/examples.json"
        ))
        .unwrap();
        let Document::Plan(mut central) =
            decode(&serde_json::to_vec(&examples["plan"]).unwrap()).unwrap()
        else {
            panic!()
        };
        central.wall_time_ms = 60_000;
        central.max_permit_ms = 30_000;
        let legacy: LaunchManifest = serde_json::from_str(include_str!(
            "../../../vendor/aq-session-host-0.1.6/tests/fixtures/manifest.json"
        ))
        .unwrap();
        let CentralSubject::CentralConversation {
            actor_id,
            conversation_id,
            conversation_revision,
            policy,
        } = &central.subject;
        let now = aq_supervisor_host::clock().unwrap();
        let mut resources = legacy.resources;
        resources.wall_time_ms = 60_000;
        resources.stop_grace_ms = 50;
        let mut terminal = legacy.terminal;
        terminal.input_enabled = false;
        let host = HostPlan {
            host_plan_version: 1,
            subject: HostSubject::CentralConversation {
                actor_id: *actor_id,
                conversation_id: *conversation_id,
                conversation_revision: *conversation_revision,
            },
            session: central.session.clone(),
            policy: policy.clone(),
            execution: central.profiles.execution.clone(),
            input: HostInput {
                dispatch_id: central.dispatch_id,
                host_operation_id: central.host_operation_id,
                batch: ArtifactRef {
                    digest: central.batch.digest().unwrap(),
                    ..pin("batch")
                },
            },
            harness: legacy.harness,
            resources,
            terminal,
            startup: HostStartup {
                descriptor: pin("startup"),
                payload: ArtifactRef {
                    digest: Digest::of(b"supervisor startup\n"),
                    ..pin("payload")
                },
                bytes: 19,
            },
            machine_boot_id: now.machine_boot_id,
            admitted_boot_ms: now.boot_ms,
            deadline_boot_ms: now.boot_ms + 60_000,
        };
        central.profiles.host_plan = ArtifactRef {
            digest: host.digest().unwrap(),
            ..pin("host")
        };
        central.profiles.adapter = host.harness.adapter.clone();
        central.profiles.environment = host.harness.environment.clone();
        central.profiles.startup = host.startup.descriptor.clone();
        let version = std::process::Command::new("/usr/bin/tmux")
            .arg("-V")
            .output()
            .unwrap();
        assert!(version.status.success());
        let local = LocalEnvironment {
            resource_policy: ResourcePolicy::CallerAdmission(pin("admission")),
            workspace: root.path().canonicalize().unwrap(),
            helper_executable: env!("CARGO_BIN_EXE_aq-session-host").into(),
            tmux_executable: "/usr/bin/tmux".into(),
            tmux_version: String::from_utf8(version.stdout).unwrap().trim().into(),
            credential_profiles: vec![],
            context_material: vec![],
        };
        Self {
            _db: db,
            pool,
            root,
            local,
            central,
            host,
        }
    }
    fn controller(&self) -> Supervisor<Admission, FixtureAdapter> {
        Supervisor::new(
            self.pool.clone(),
            self.root.path().into(),
            self.local.clone(),
            Arc::new(Admission),
            Arc::new(FixtureAdapter),
        )
    }
    async fn start(&self, host: &Supervisor<Admission, FixtureAdapter>) -> Snapshot {
        host.start(
            self.central.clone(),
            self.host.clone(),
            b"supervisor startup\n".to_vec(),
            30_000,
        )
        .await
        .unwrap()
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervisor_session_starts_under_central_permit() {
    let case = Case::new().await;
    let host = case.controller();
    let run = case.start(&host).await;
    assert_eq!(run.run.phase(), Phase::RunningReported);
    assert_eq!(run.run.plan().session, case.central.session);
    let observed = host.observe(case.central.subject.actor_id()).await.unwrap();
    assert_eq!(observed.phase, aq_session_host::HostPhase::Running);
    assert_eq!(observed.permit_sequence, 1);
    assert_eq!(
        observed.permit_deadline_boot_ms,
        run.run.permit().unwrap().expires_boot_ms
    );
    let directory = case
        .root
        .path()
        .join(observed.guardian.intent.launch_id.to_string());
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        std::fs::read(directory.join("delivered")).unwrap(),
        b"supervisor startup\n"
    );
    let stopped = host.stop(case.central.subject.actor_id()).await.unwrap();
    assert_eq!(stopped.run.phase(), Phase::StoppedReported);
    assert!(directory.exists());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supervisor_restart_resumes_without_duplicate_session() {
    let case = Case::new().await;
    let first = case.controller();
    let started = case.start(&first).await;
    let identity = first
        .observe(case.central.subject.actor_id())
        .await
        .unwrap()
        .guardian;
    drop(first);
    let second = case.controller();
    let resumed = second
        .reconcile(case.central.subject.actor_id())
        .await
        .unwrap();
    assert_eq!(resumed.run.plan().session, started.run.plan().session);
    assert_eq!(resumed.run.permit(), started.run.permit());
    let replay = case.start(&second).await;
    assert_eq!(replay.run.plan().generation, 1);
    assert_eq!(
        second
            .observe(case.central.subject.actor_id())
            .await
            .unwrap()
            .guardian,
        identity
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM central_host_launches")
        .fetch_one(&case.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    second.stop(case.central.subject.actor_id()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_start_has_one_guardian_and_cancel_prevents_revival() {
    let case = Case::new().await;
    let a = case.controller();
    let b = case.controller();
    let (x, y) = tokio::join!(
        a.start(
            case.central.clone(),
            case.host.clone(),
            b"supervisor startup\n".to_vec(),
            30_000
        ),
        b.start(
            case.central.clone(),
            case.host.clone(),
            b"supervisor startup\n".to_vec(),
            30_000
        )
    );
    assert_eq!(x.unwrap().run.plan(), y.unwrap().run.plan());
    assert_eq!(
        a.observe(case.central.subject.actor_id())
            .await
            .unwrap()
            .guardian,
        b.observe(case.central.subject.actor_id())
            .await
            .unwrap()
            .guardian
    );
    a.stop(case.central.subject.actor_id()).await.unwrap();
    assert!(
        b.renew(case.central.subject.actor_id(), 1, 30_000)
            .await
            .is_err()
    );
    assert!(
        b.start(
            case.central.clone(),
            case.host.clone(),
            b"supervisor startup\n".to_vec(),
            30_000
        )
        .await
        .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mismatched_host_plan_never_creates_a_writer() {
    let mut case = Case::new().await;
    let host = case.controller();
    case.host.input.dispatch_id = Uuid::new_v4();
    assert!(
        host.start(
            case.central.clone(),
            case.host.clone(),
            b"supervisor startup\n".to_vec(),
            30_000
        )
        .await
        .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM central_host_launches")
        .fetch_one(&case.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_supervisor_is_stopped_and_scratch_is_retained() {
    let case = Case::new().await;
    let host = case.controller();
    host.start(
        case.central.clone(),
        case.host.clone(),
        b"supervisor startup\n".to_vec(),
        2000,
    )
    .await
    .unwrap();
    let id = host
        .observe(case.central.subject.actor_id())
        .await
        .unwrap()
        .guardian
        .intent
        .launch_id;
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let stopped = host
        .reconcile(case.central.subject.actor_id())
        .await
        .unwrap();
    assert_eq!(stopped.run.phase(), Phase::StoppedReported);
    assert!(
        case.root
            .path()
            .join(id.to_string())
            .join("delivered")
            .exists()
    );
    assert!(
        host.renew(case.central.subject.actor_id(), 1, 30_000)
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn successor_requires_stop_and_retains_prior_generation() {
    let mut case = Case::new().await;
    let host = case.controller();
    case.start(&host).await;
    let actor = case.central.subject.actor_id();
    let original = case.central.clone();
    case.central.generation = 2;
    if let SessionIdentity::Central {
        session_id,
        generation,
        ..
    } = &mut case.central.session
    {
        *session_id = Uuid::new_v4();
        *generation = 2;
    }
    case.central.dispatch_id = Uuid::new_v4();
    case.central.host_operation_id = Uuid::new_v4();
    case.central.batch.messages.truncate(1);
    case.central.batch.messages[0].message_id = Uuid::new_v4();
    case.central.batch.messages[0].cursor = 100;
    case.central.batch.through_cursor = 100;
    case.host.session = case.central.session.clone();
    case.host.input.dispatch_id = case.central.dispatch_id;
    case.host.input.host_operation_id = case.central.host_operation_id;
    case.host.input.batch.digest = case.central.batch.digest().unwrap();
    let now = aq_supervisor_host::clock().unwrap();
    case.host.admitted_boot_ms = now.boot_ms;
    case.host.deadline_boot_ms = now.boot_ms + 60_000;
    case.central.profiles.host_plan.digest = case.host.digest().unwrap();
    assert!(
        host.start(
            case.central.clone(),
            case.host.clone(),
            b"supervisor startup\n".to_vec(),
            30_000
        )
        .await
        .is_err()
    );
    host.stop(actor).await.unwrap();
    let now = aq_supervisor_host::clock().unwrap();
    case.host.admitted_boot_ms = now.boot_ms;
    case.host.deadline_boot_ms = now.boot_ms + 60_000;
    case.central.profiles.host_plan.digest = case.host.digest().unwrap();
    let next = case.start(&host).await;
    assert_eq!(next.run.plan().generation, 2);
    let bytes: String = sqlx::query_scalar(
        "SELECT state_bytes FROM central_session_generations WHERE actor_id=$1 AND generation=1",
    )
    .bind(actor)
    .fetch_one(&case.pool)
    .await
    .unwrap();
    let Document::State(old) = decode(bytes.as_bytes()).unwrap() else {
        panic!()
    };
    assert_eq!(old.plan(), &original);
    assert_eq!(old.phase(), Phase::StoppedReported);
    host.stop(actor).await.unwrap();
}

struct Revoked;
#[async_trait::async_trait]
impl PlanAuthority for Revoked {
    async fn authorize(
        &self,
        _: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _: &CentralPlan,
        _: &HostPlan,
    ) -> Result<()> {
        anyhow::bail!("policy revoked")
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revoked_policy_stops_instead_of_readopting() {
    let case = Case::new().await;
    case.start(&case.controller()).await;
    let host = Supervisor::new(
        case.pool.clone(),
        case.root.path().into(),
        case.local.clone(),
        Arc::new(Revoked),
        Arc::new(FixtureAdapter),
    );
    assert_eq!(
        host.reconcile(case.central.subject.actor_id())
            .await
            .unwrap()
            .run
            .phase(),
        Phase::StoppedReported
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn physical_renewal_requires_next_sequence_and_later_deadline() {
    let case = Case::new().await;
    let host = case.controller();
    host.start(
        case.central.clone(),
        case.host.clone(),
        b"supervisor startup\n".to_vec(),
        5000,
    )
    .await
    .unwrap();
    let before = host.observe(case.central.subject.actor_id()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let next = host
        .renew(case.central.subject.actor_id(), 1, 10000)
        .await
        .unwrap();
    let after = host.observe(case.central.subject.actor_id()).await.unwrap();
    assert_eq!(after.guardian, before.guardian);
    assert_eq!(after.permit_sequence, 2);
    assert!(after.permit_deadline_boot_ms > before.permit_deadline_boot_ms);
    assert_eq!(
        after.permit_deadline_boot_ms,
        next.run.permit().unwrap().expires_boot_ms
    );
    assert!(
        host.renew(case.central.subject.actor_id(), 1, 10000)
            .await
            .is_err()
    );
    host.stop(case.central.subject.actor_id()).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_guardian_blocks_successor_and_retains_scratch() {
    let case = Case::new().await;
    let host = case.controller();
    case.start(&host).await;
    let observed = host.observe(case.central.subject.actor_id()).await.unwrap();
    use aq_session_host::ownership::{signal, verified_pidfd};
    let guardian = verified_pidfd(&observed.guardian.process).unwrap();
    signal(&guardian, libc::SIGKILL).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        host.reconcile(case.central.subject.actor_id())
            .await
            .is_err()
    );
    assert_eq!(
        host.inspect(case.central.subject.actor_id())
            .await
            .unwrap()
            .run
            .phase(),
        Phase::StopUncertain
    );
    assert!(
        case.root
            .path()
            .join(observed.guardian.intent.launch_id.to_string())
            .exists()
    );
    assert!(
        host.start(
            case.central.clone(),
            case.host.clone(),
            b"supervisor startup\n".to_vec(),
            30_000
        )
        .await
        .is_err()
    );
    // Test-only cleanup by exact pidfd. This is deliberately not stopped proof.
    if let Some(pane) = observed.pane {
        for p in [pane.pane, pane.server] {
            if let Ok(fd) = verified_pidfd(&p) {
                let _ = signal(&fd, libc::SIGKILL);
            }
        }
    }
}

struct SwitchAdmission(Arc<std::sync::atomic::AtomicBool>);
#[async_trait::async_trait]
impl PlanAuthority for SwitchAdmission {
    async fn authorize(
        &self,
        _: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _: &CentralPlan,
        _: &HostPlan,
    ) -> Result<()> {
        anyhow::ensure!(self.0.load(std::sync::atomic::Ordering::SeqCst), "revoked");
        Ok(())
    }
}
struct RevokeDuringPreparation(Arc<std::sync::atomic::AtomicBool>);
impl HostAdapter for RevokeDuringPreparation {
    fn prepare(&self, p: &HostPlan, w: &Path) -> Result<AdapterLaunch> {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
        FixtureAdapter.prepare(p, w)
    }
    fn readiness(&self, o: &HostObservation) -> Result<HarnessReadiness> {
        FixtureAdapter.readiness(o)
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revocation_before_physical_start_refuses_and_stops_guardian() {
    let case = Case::new().await;
    let allowed = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let host = Supervisor::new(
        case.pool.clone(),
        case.root.path().into(),
        case.local.clone(),
        Arc::new(SwitchAdmission(allowed.clone())),
        Arc::new(RevokeDuringPreparation(allowed)),
    );
    assert!(
        host.start(
            case.central.clone(),
            case.host.clone(),
            b"supervisor startup\n".to_vec(),
            30_000
        )
        .await
        .is_err()
    );
    assert_eq!(
        host.inspect(case.central.subject.actor_id())
            .await
            .unwrap()
            .run
            .phase(),
        Phase::StoppedReported
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM central_host_facts WHERE kind='start'")
            .fetch_one(&case.pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn main_delivery_database_upgrades_without_losing_history() {
    let db = aq_server::test_support::TestDb::create().await;
    let pool = db.pool().await;
    aq_server::test_support::migrate_profile(&pool, 45)
        .await
        .unwrap();
    sqlx::query("INSERT INTO delivery_intents(operation_id,repository_id,binding_generation,operation,inputs,input_digest,payload_digest,request) VALUES($1,$2,1,'candidate.show','{}',repeat('0',64),repeat('0',64),$3)")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(vec![1u8]).execute(&pool).await.unwrap();
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT to_regclass('central_session_heads') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    drop(db.store().await);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "47"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM delivery_intents")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT to_regclass('central_host_launches') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
}
