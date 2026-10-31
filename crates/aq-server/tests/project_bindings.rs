//! AQ-19 public-owner routes, fake authoritative peers and explicit real-peer gate.
use aq_server::{
    AppState,
    agent_access::{AgentAccessConfig, Runtime as GraphRuntime},
    config::Config,
    source_retention::{Runtime as GitRuntime, SourceRetentionConfig},
    store::NewNode,
    test_support::TestDb,
    tokens,
};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::get,
};
use chrono::{Duration, TimeZone, Utc};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU8, AtomicU64, Ordering},
    },
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tower::ServiceExt;
use uuid::Uuid;
struct Env {
    app: Router,
    state: AppState,
    pool: sqlx::PgPool,
    node: Uuid,
    body: Value,
    generation: Arc<AtomicU64>,
    git_status: Arc<AtomicU8>,
    graph_status: Arc<AtomicU8>,
    time: Arc<AtomicI64>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
    _dir: tempfile::TempDir,
    _db: TestDb,
    _gate: tokio::sync::OwnedMutexGuard<()>,
}
impl Drop for Env {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
async fn graph(
    State((status, workspace)): State<(Arc<AtomicU8>, Uuid)>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let status = if id != workspace {
        StatusCode::NOT_FOUND
    } else {
        match status.load(Ordering::SeqCst) {
            1 => StatusCode::FORBIDDEN,
            2 => StatusCode::NOT_FOUND,
            3 => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::OK,
        }
    };
    (
        status,
        [("X-AQ-Graph-API-Version", "1")],
        Json(if status == StatusCode::OK {
            json!({"items":[],"next_cursor":null})
        } else {
            json!({"error":"unavailable"})
        }),
    )
}
async fn call(app: &Router, path: &str, body: Option<Value>, email: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header("content-type", "application/json")
        .header("X-AQ-Request", "1")
        .header("x-aq-dev-email", email);
    let response = app
        .clone()
        .oneshot(
            request
                .body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"raw":String::from_utf8_lossy(&bytes)})),
    )
}
async fn setup(real: bool) -> Env {
    // The production external gate is deliberately process-wide and fail-fast.
    // Independent fixture tests serialize so they do not manufacture overload.
    static GATE: std::sync::OnceLock<Arc<tokio::sync::Mutex<()>>> = std::sync::OnceLock::new();
    let gate = GATE
        .get_or_init(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
        .lock_owned()
        .await;
    let db = TestDb::create().await;
    let store = db.store().await;
    let pool = db.pool().await;
    let dir = tempfile::tempdir().unwrap();
    let generation = Arc::new(AtomicU64::new(1));
    let git_status = Arc::new(AtomicU8::new(0));
    let graph_status = Arc::new(AtomicU8::new(0));
    let mut tasks = Vec::new();
    let workspace = if real {
        std::env::var("AQ_PROJECT_WORKSPACE_ID")
            .unwrap()
            .parse()
            .unwrap()
    } else {
        Uuid::from_u128(3)
    };
    let repository = if real {
        std::env::var("AQ_PROJECT_REPOSITORY_ID")
            .unwrap()
            .parse()
            .unwrap()
    } else {
        Uuid::from_u128(2)
    };
    let socket = if real {
        PathBuf::from(std::env::var("AQ_PROJECT_GIT_SOCKET").unwrap())
    } else {
        dir.path().join("git.sock")
    };
    if !real {
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let g = generation.clone();
        let status = git_status.clone();
        tasks.push(tokio::spawn(async move {loop {let(stream,_)=listener.accept().await.unwrap();let g=g.clone();let status=status.clone();tokio::spawn(async move {let mut reader=BufReader::new(stream);let mut line=Vec::new();reader.read_until(b'\n',&mut line).await.unwrap();let r:Value=serde_json::from_slice(&line).unwrap();assert_eq!(r["operation"],"repo.show");assert_eq!(r["protocol_version"],1);let expected=r["expected_binding_generation"].as_u64().unwrap();let current=g.load(Ordering::SeqCst);let mut out=json!({"protocol_version":1,"request_id":r["request_id"]});
            if r["repository_id"]!=repository.to_string() || status.load(Ordering::SeqCst)==1 {out["ok"]=false.into();out["error"]=json!({"code":"repository_not_found","message":"absent","details":{},"retry":"refresh_inputs"});}
            else if expected!=current {out["ok"]=false.into();out["error"]=json!({"code":"binding_conflict","message":"stale","details":{"binding_generation":current},"retry":"refresh_inputs"});}
            else {out["ok"]=true.into();out["result"]=json!({"repository_id":repository,"binding_generation":current,"mode":if status.load(Ordering::SeqCst)==2{"worker"}else{"central"},"object_format":"sha1"});}
            let mut bytes=serde_json::to_vec(&out).unwrap();bytes.push(b'\n');reader.into_inner().write_all(&bytes).await.unwrap();});}}));
    }
    let graph_url = if real {
        std::env::var("AQ_AGENT_GRAPH_TEST_URL").unwrap()
    } else {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/v1/workspaces/{id}/nodes", get(graph))
            .with_state((graph_status.clone(), workspace));
        tasks.push(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap()
        }));
        format!("http://{addr}")
    };
    let token_file = if real {
        PathBuf::from(std::env::var("AQ_AGENT_GRAPH_TEST_TOKEN_FILE").unwrap())
    } else {
        let p = dir.path().join("graph-token");
        std::fs::write(&p, "synthetic-project-read-token-0000000000000000").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        p
    };
    let graph = GraphRuntime::new(&AgentAccessConfig {
        graph_url,
        graph_token_file: token_file,
        credential_ttl_ms: 10_000,
        mutation_actor_id: None,
    })
    .await
    .unwrap();
    let git = GitRuntime::new(SourceRetentionConfig {
        staging_root: dir.path().join("staging"),
        git_root: dir.path().join("git"),
        git_socket: socket,
        max_staging_bytes: 1024 * 1024,
        max_bundle_bytes: 1024 * 1024,
        bulk_concurrency: 1,
        incomplete_ttl_secs: 60,
    })
    .unwrap();
    let now = Utc::now();
    let owner = store
        .bootstrap_owner("owner@localhost", "dev", "owner@localhost", now)
        .await
        .unwrap()
        .unwrap();
    store
        .invite_user("viewer@localhost", aq_server::store::Role::Viewer, now)
        .await
        .unwrap();
    store
        .create_enrollment("project-enroll", owner.id, now, now + Duration::minutes(5))
        .await
        .unwrap();
    let token = tokens::new_node_credential();
    let node = store
        .redeem_enrollment(
            "project-enroll",
            NewNode {
                name: "project-worker".into(),
                credential_hash: tokens::verifier(&token),
                worker_version: "0.1.0".into(),
                protocol_version: 1,
            },
            now,
        )
        .await
        .unwrap()
        .unwrap();
    sqlx::query("UPDATE nodes SET last_seen_at=$2 WHERE id=$1")
        .bind(node.id)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();
    let mut config: Config = toml::from_str(
        r#"format_version=1
environment="development"
[server]
admin_bind="127.0.0.1:0"
worker_bind="127.0.0.1:0"
public_origin="http://127.0.0.1:8080"
[auth]
mode="dev"
dev_email="owner@localhost"
[bootstrap]
owner_email="owner@localhost"
enabled=true
"#,
    )
    .unwrap();
    config
        .projects
        .worker_groups
        .insert("engineering".into(), vec![node.id]);
    let time = Arc::new(AtomicI64::new(now.timestamp_millis()));
    let clock = time.clone();
    let state = AppState {
        config: Arc::new(config),
        store,
        identity: Arc::new(aq_server::identity::DevIdentity::new(
            "owner@localhost".into(),
        )),
        clock: Arc::new(move || {
            Utc.timestamp_millis_opt(clock.load(Ordering::SeqCst))
                .unwrap()
        }),
        agent_access: Some(Arc::new(graph)),
        source_retention: Some(Arc::new(git)),
    };
    let app = aq_server::build_admin_router(state.clone());
    let catalog = aq_profiles::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../aq-profiles/examples/catalog"),
    )
    .unwrap();
    let activated=call(&app,"/api/profiles/catalogs/activate",Some(json!({"operation_id":Uuid::new_v4(),"expected_revision":0,"artifact_json":String::from_utf8(catalog.canonical_bytes().unwrap()).unwrap()})),"owner@localhost").await;
    assert_eq!(activated.0, StatusCode::OK, "{}", activated.1);
    let body = json!({"schema_version":1,"project_id":Uuid::from_u128(1),"name":"Example project","enabled":true,"repository_id":repository,"repository_binding_generation":1,"repository_adapter_reference":"aq.git.configured","graph_adapter_reference":"aq.graph.configured","graph_workspace_id":workspace,"catalog_digest":catalog.digest(),"allowed_agent_profiles":["implementer@1"],"allowed_intelligence_profiles":["engineering@1"],"allowed_execution_profiles":["confined-development@1"],"allowed_node_ids":[],"allowed_worker_groups":["engineering"],"allowed_environments":["confined-worktree"],"policy":{"identity":"policy","revision":"1","digest":"b".repeat(64)},"context":[],"ceilings":{"permissions":[{"operation":"task.read","scope_reference":"project:example"}],"context_bytes":8192,"wall_time_ms":60000,"memory_bytes":268435456,"cpu_millis":60000,"process_limit":8,"cost_limit_microunits":null,"currency":null,"require_hard_cost_enforcement":false}});
    Env {
        app,
        state,
        pool,
        node: node.id,
        body,
        generation,
        git_status,
        graph_status,
        time,
        tasks,
        _dir: dir,
        _db: db,
        _gate: gate,
    }
}
fn path() -> String {
    format!("/api/projects/{}", Uuid::from_u128(1))
}
fn admission(env: &Env) -> Value {
    json!({"expected_project_revision":1,"expected_catalog_revision":1,"node_id":env.node,"environment":"confined-worktree","agent_profile":"implementer@1","intelligence_profile":"engineering@1","execution_profile":"confined-development@1"})
}
async fn bind(env: &Env) -> Value {
    let (status, v) = call(
        &env.app,
        &path(),
        Some(json!({"operation_id":Uuid::new_v4(),"expected_revision":0,"binding":env.body})),
        "owner@localhost",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    v
}
#[tokio::test]
async fn revisioned_binding_replay_history_and_retained_snapshot_survive_updates() {
    let env = setup(false).await;
    let operation = Uuid::new_v4();
    let request = json!({"operation_id":operation,"expected_revision":0,"binding":env.body});
    let (status, first) = call(&env.app, &path(), Some(request.clone()), "owner@localhost").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let frozen = call(
        &env.app,
        &format!("{}/eligibility", path()),
        Some(admission(&env)),
        "owner@localhost",
    )
    .await;
    assert_eq!(frozen.0, StatusCode::OK);
    assert_eq!(frozen.1["eligible"], true, "{}", frozen.1);
    assert_eq!(
        call(&env.app, &path(), Some(request.clone()), "owner@localhost")
            .await
            .1,
        first
    );
    let mut changed = request.clone();
    changed["binding"]["name"] = "Changed".into();
    assert_eq!(
        call(&env.app, &path(), Some(changed), "owner@localhost")
            .await
            .0,
        StatusCode::CONFLICT
    );
    let mut update = request;
    update["operation_id"] = json!(Uuid::new_v4());
    update["expected_revision"] = 1.into();
    update["binding"]["ceilings"]["context_bytes"] = 4096.into();
    let updated = call(&env.app, &path(), Some(update.clone()), "owner@localhost").await;
    assert_eq!(updated.0, StatusCode::OK);
    assert_eq!(updated.1["record"]["revision"], 2);
    update["operation_id"] = json!(Uuid::new_v4());
    assert_eq!(
        call(&env.app, &path(), Some(update), "owner@localhost")
            .await
            .0,
        StatusCode::CONFLICT
    );
    let old = call(
        &env.app,
        &format!("{}/revisions/1", path()),
        None,
        "owner@localhost",
    )
    .await;
    assert_eq!(old.1["binding"]["ceilings"]["context_bytes"], 8192);
    assert_eq!(frozen.1["frozen_binding"]["project"]["revision"], 1);
    let stale = call(
        &env.app,
        &format!("{}/eligibility", path()),
        Some(admission(&env)),
        "owner@localhost",
    )
    .await;
    assert_eq!(stale.1["eligible"], false);
}
#[tokio::test]
async fn absent_stale_forbidden_and_offline_components_refuse_and_disable_stays_available() {
    let env = setup(false).await;
    bind(&env).await;
    for (git, graph) in [(1, 0), (0, 1), (0, 2), (0, 3), (2, 0)] {
        env.git_status.store(git, Ordering::SeqCst);
        env.graph_status.store(graph, Ordering::SeqCst);
        let out = call(
            &env.app,
            &format!("{}/eligibility", path()),
            Some(admission(&env)),
            "owner@localhost",
        )
        .await;
        assert_eq!(out.0, StatusCode::OK);
        assert_eq!(out.1["eligible"], false);
        assert!(out.1["frozen_binding"].is_null());
    }
    env.git_status.store(0, Ordering::SeqCst);
    env.graph_status.store(0, Ordering::SeqCst);
    env.generation.store(2, Ordering::SeqCst);
    let stale = call(
        &env.app,
        &format!("{}/eligibility", path()),
        Some(admission(&env)),
        "owner@localhost",
    )
    .await;
    assert!(
        stale.1["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("repository_stale"))
    );
    env.graph_status.store(3, Ordering::SeqCst);
    let disabled = call(
        &env.app,
        &format!("{}/disable", path()),
        Some(json!({"operation_id":Uuid::new_v4(),"expected_revision":1})),
        "owner@localhost",
    )
    .await;
    assert_eq!(disabled.0, StatusCode::OK);
    assert_eq!(disabled.1["record"]["binding"]["enabled"], false);
}
#[tokio::test]
async fn owner_scope_node_restrictions_and_mutation_unknown_fields_are_enforced() {
    let env = setup(false).await;
    let request = json!({"operation_id":Uuid::new_v4(),"expected_revision":0,"binding":env.body});
    assert_eq!(
        call(&env.app, &path(), Some(request.clone()), "viewer@localhost")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let mut forged = request.clone();
    forged["binding"]["graph_url"] = "http://attacker".into();
    assert_eq!(
        call(&env.app, &path(), Some(forged), "owner@localhost")
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let mut unknown = request.clone();
    unknown["binding"]["allowed_worker_groups"] = json!(["worker-self-asserted"]);
    assert_eq!(
        call(&env.app, &path(), Some(unknown), "owner@localhost")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    bind(&env).await;
    let mut denied = admission(&env);
    denied["environment"] = "unconfined".into();
    assert_eq!(
        call(
            &env.app,
            &format!("{}/eligibility", path()),
            Some(denied),
            "owner@localhost"
        )
        .await
        .1["eligible"],
        false
    );
    env.time.fetch_add(120_000, Ordering::SeqCst);
    assert_eq!(
        call(
            &env.app,
            &format!("{}/eligibility", path()),
            Some(admission(&env)),
            "owner@localhost"
        )
        .await
        .1["eligible"],
        false
    );
    assert_eq!(
        call(&env.app, "/api/projects?limit=1", None, "owner@localhost")
            .await
            .1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
#[ignore = "requires immutable selected aq-git and aq-graph components via scripts/test-project-bindings.py"]
async fn selected_real_public_repository_and_graph_workspace_bind() {
    let env = setup(true).await;
    let result = bind(&env).await;
    assert_eq!(
        result["record"]["binding"]["repository_binding_generation"],
        1
    );
    let decision = call(
        &env.app,
        &format!("{}/eligibility", path()),
        Some(admission(&env)),
        "owner@localhost",
    )
    .await;
    assert_eq!(decision.0, StatusCode::OK);
    assert_eq!(decision.1["eligible"], true, "{}", decision.1);
    let mut stale = env.body.clone();
    stale["repository_binding_generation"] = 2.into();
    let refused = call(
        &env.app,
        &path(),
        Some(json!({"operation_id":Uuid::new_v4(),"expected_revision":1,"binding":stale})),
        "owner@localhost",
    )
    .await;
    assert_eq!(refused.0, StatusCode::CONFLICT);
    let unchanged = call(&env.app, &path(), None, "owner@localhost").await;
    assert_eq!(unchanged.1["revision"], 1);
    println!(
        "qualified public repo.show expected-generation binding + graph API1 workspace read, CAS stale refusal; no task lifecycle or Git-private-state writes"
    );
}
async fn wait_for_blocked(pool: &sqlx::PgPool, fragment: &str) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE $1").bind(format!("%{fragment}%")).fetch_one(pool).await.unwrap();
        if count > 0 {
            return;
        }
        assert!(
            std::time::Instant::now() < end,
            "request did not reach intended lock wait"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
}
#[tokio::test]
async fn observation_expiring_during_mutation_lock_wait_never_commits_fresh_validation() {
    let env = setup(false).await;
    let mut blocker = env.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('aq-project-binding-write-v1',0))")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let app = env.app.clone();
    let request = json!({"operation_id":Uuid::new_v4(),"expected_revision":0,"binding":env.body});
    let task =
        tokio::spawn(async move { call(&app, &path(), Some(request), "owner@localhost").await });
    wait_for_blocked(&env.pool, "aq-project-binding-write-v1").await;
    env.time.fetch_add(30_001, Ordering::SeqCst);
    blocker.commit().await.unwrap();
    let response = task.await.unwrap();
    assert_eq!(response.0, StatusCode::CONFLICT, "{}", response.1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM project_execution_bindings")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
#[tokio::test]
async fn final_local_fence_rechecks_node_revocation_after_lock_wait() {
    let env = setup(false).await;
    bind(&env).await;
    let mut blocker = env.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE")
        .bind(env.node)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let app = env.app.clone();
    let r = admission(&env);
    let task = tokio::spawn(async move {
        call(
            &app,
            &format!("{}/eligibility", path()),
            Some(r),
            "owner@localhost",
        )
        .await
    });
    wait_for_blocked(&env.pool, "SELECT revoked_at,last_seen_at FROM nodes").await;
    sqlx::query("UPDATE nodes SET revoked_at=now() WHERE id=$1")
        .bind(env.node)
        .execute(&mut *blocker)
        .await
        .unwrap();
    blocker.commit().await.unwrap();
    let response = task.await.unwrap();
    assert_eq!(response.0, StatusCode::OK);
    assert_eq!(response.1["eligible"], false);
    assert!(response.1["frozen_binding"].is_null());
}
#[tokio::test]
async fn catalog_activation_blocks_future_eligibility_and_keeps_frozen_project_revision() {
    let env = setup(false).await;
    bind(&env).await;
    let old = call(
        &env.app,
        &format!("{}/eligibility", path()),
        Some(admission(&env)),
        "owner@localhost",
    )
    .await
    .1;
    assert_eq!(old["eligible"], true);
    let seed = aq_profiles::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../aq-profiles/examples/catalog"),
    )
    .unwrap();
    let mut definitions = seed.artifact().catalog.clone();
    let mut instructions = seed.artifact().instructions.clone();
    let mut module = definitions.modules[0].clone();
    module.id = "additional-policy".into();
    module.instruction = "instructions/additional-policy.md".into();
    definitions.modules.push(module);
    instructions.insert(
        "additional-policy@1".into(),
        "Additional future policy\n".into(),
    );
    let next = aq_profiles::validate(definitions, instructions).unwrap();
    let activated=call(&env.app,"/api/profiles/catalogs/activate",Some(json!({"operation_id":Uuid::new_v4(),"expected_revision":1,"artifact_json":String::from_utf8(next.canonical_bytes().unwrap()).unwrap()})),"owner@localhost").await;
    assert_eq!(activated.0, StatusCode::OK, "{}", activated.1);
    let decision = call(
        &env.app,
        &format!("{}/eligibility", path()),
        Some(admission(&env)),
        "owner@localhost",
    )
    .await
    .1;
    assert_eq!(decision["eligible"], false);
    assert!(
        decision["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("catalog_activation_changed"))
    );
    assert_eq!(
        old["frozen_binding"]["project"]["binding"]["catalog_digest"],
        seed.digest()
    );
    let history = call(
        &env.app,
        &format!("{}/revisions/1", path()),
        None,
        "owner@localhost",
    )
    .await
    .1;
    assert_eq!(history["binding"]["catalog_digest"], seed.digest());
    assert_eq!(history["revision"], 1);
}

#[tokio::test]
async fn exact_replay_survives_group_configuration_changes_but_new_mutations_do_not() {
    let env = setup(false).await;
    let request = json!({"operation_id":Uuid::new_v4(),"expected_revision":0,"binding":env.body});
    let first = call(&env.app, &path(), Some(request.clone()), "owner@localhost").await;
    assert_eq!(first.0, StatusCode::OK);
    let mut state = env.state.clone();
    let mut config = (*state.config).clone();
    config.projects.worker_groups.clear();
    state.config = Arc::new(config);
    let reconfigured = aq_server::build_admin_router(state);
    let replay = call(
        &reconfigured,
        &path(),
        Some(request.clone()),
        "owner@localhost",
    )
    .await;
    assert_eq!(replay.0, StatusCode::OK);
    assert_eq!(replay.1, first.1);
    let mut changed = request.clone();
    changed["binding"]["name"] = "Changed".into();
    assert_eq!(
        call(&reconfigured, &path(), Some(changed), "owner@localhost")
            .await
            .0,
        StatusCode::CONFLICT
    );
    let mut new = request;
    new["operation_id"] = json!(Uuid::new_v4());
    new["expected_revision"] = 1.into();
    assert_eq!(
        call(&reconfigured, &path(), Some(new), "owner@localhost")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &reconfigured,
            &path(),
            Some(json!({"operation_id":Uuid::new_v4(),"expected_revision":1,"binding":env.body})),
            "viewer@localhost"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
}

struct SelectionFixture {
    env: Env,
    input: aq_selection::SelectionInputs,
    frozen: aq_server::project_bindings::domain::FrozenProjectBinding,
    actor: Uuid,
    lineage: aq_selection::TaskLineage,
    manual: Value,
    unrelated: aq_selection::AuthorizedCredential,
    time: u64,
    operation: Uuid,
}
async fn selection_fixture() -> SelectionFixture {
    use aq_selection::*;
    use aq_server::intelligence::config::{CredentialConfig, IntelligenceConfig};
    let mut env = setup(false).await;
    let fixture: Value = serde_json::from_str(include_str!("fixtures/selection-v2.json")).unwrap();
    let mut input: SelectionInputs = serde_json::from_value(fixture["inputs"].clone()).unwrap();
    let activation=call(&env.app,"/api/profiles/catalogs/activate",Some(json!({"operation_id":Uuid::new_v4(),"expected_revision":1,"artifact_json":fixture["catalog_artifact_json"]})),"owner@localhost").await;
    assert_eq!(activation.0, StatusCode::OK, "{}", activation.1);
    env.body["catalog_digest"] = json!(input.project.catalog_digest);
    env.body["allowed_agent_profiles"] = json!(["implementer@2"]);
    env.body["allowed_intelligence_profiles"] = json!(["engineering@2"]);
    env.body["allowed_execution_profiles"] = json!(["confined-development@2"]);
    env.body["ceilings"]["permissions"] = json!(
        input
            .project
            .permission_ceiling
            .iter()
            .map(|p| json!({"operation":p.operation,"scope_reference":p.resource}))
            .collect::<Vec<_>>()
    );
    bind(&env).await;
    let request=serde_json::from_value(json!({"expected_project_revision":1,"expected_catalog_revision":2,"node_id":env.node,"environment":"confined-worktree","agent_profile":"implementer@2","intelligence_profile":"engineering@2","execution_profile":"confined-development@2"})).unwrap();
    let frozen =
        aq_server::project_bindings::check_eligibility(&env.state, Uuid::from_u128(1), &request)
            .await
            .unwrap()
            .frozen_binding
            .unwrap();
    let boot = Uuid::new_v4();
    env.state
        .store
        .execution_register(&aq_worker_protocol::RegisterRequest {
            identity: aq_worker_protocol::NodeIdentity {
                node_id: env.node,
                boot_id: boot,
            },
            operation_id: Uuid::new_v4(),
            protocol_version: 2,
            worker_version: "0.1.0".into(),
            capabilities: vec![aq_worker_protocol::FAKE_HARNESS_CAPABILITY.into()],
        })
        .await
        .unwrap();
    let key = env._dir.path().join("synthetic-provider-key");
    std::fs::write(&key, b"synthetic-not-a-provider-key").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut configured: Vec<_> = input
        .credentials
        .iter()
        .cloned()
        .map(|mut c| {
            c.provisionable = false;
            CredentialConfig {
                credential: c,
                allowed_node_ids: vec![env.node],
                secret_file: Some(key.clone()),
                delivery_mode: None,
            }
        })
        .collect();
    let mut unrelated = configured[0].credential.clone();
    unrelated.profile = ArtifactRef {
        id: Text::new("unrelated-account").unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(b"unrelated"),
    };
    unrelated.provider = "unrelated-provider".into();
    unrelated.account_reference = "unrelated-account".into();
    configured.push(CredentialConfig {
        credential: unrelated.clone(),
        allowed_node_ids: vec![],
        secret_file: Some(key.clone()),
        delivery_mode: None,
    });
    let config = IntelligenceConfig {
        revision: 1,
        credentials: configured,
        qualified_harnesses: input.node.harnesses.clone(),
    };
    let mut cfg = (*env.state.config).clone();
    cfg.intelligence = Some(Arc::new(config.load().unwrap()));
    env.state.config = Arc::new(cfg);
    env.app = aq_server::build_admin_router(env.state.clone());
    assert_eq!(
        call(
            &env.app,
            "/api/intelligence/register",
            Some(json!({})),
            "owner@localhost"
        )
        .await
        .0,
        StatusCode::OK
    );
    let time = env.time.load(Ordering::SeqCst) as u64;
    input.node.target = ExecutionTarget::Worker {
        node_id: env.node,
        boot_id: boot,
    };
    input.node.observed_at_ms = time;
    input.node.expires_at_ms = time + 30_000;
    for c in &mut input.node.credential_bindings {
        c.observed_at_ms = time;
        c.expires_at_ms = time + 30_000;
    }
    sqlx::query("INSERT INTO intelligence_node_snapshots(node_id,boot_id,sequence,revision,snapshot) VALUES($1,$2,1,1,$3)").bind(env.node).bind(boot).bind(json!(input.node)).execute(&env.pool).await.unwrap();
    // Fresh operator observation for the preferred account. No provider probe.
    let scope = input.observations[0].scope.clone();
    let manual = json!({"operation_id":Uuid::new_v4(),"expected_revision":1,"sequence":1,"scope":scope,"state":"available","ttl_ms":30_000,"cooldown_ms":0});
    assert_eq!(
        call(
            &env.app,
            "/api/intelligence/observations",
            Some(manual.clone()),
            "owner@localhost"
        )
        .await
        .0,
        StatusCode::OK
    );
    let actor: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE role='owner'")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    let operation = Uuid::new_v4();
    let lineage = TaskLineage {
        project_id: Uuid::from_u128(1),
        workspace_id: env.body["graph_workspace_id"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
        task_id: Uuid::new_v4(),
    };
    SelectionFixture {
        env,
        input,
        frozen,
        actor,
        lineage,
        manual,
        unrelated,
        time,
        operation,
    }
}

// AQ-20 composes the already-qualified project seam with synthetic selection
// facts. No provider or worker execution is started by this test.
#[tokio::test]
async fn intelligence_decision_is_immutable_and_final_fence_rechecks_scoped_facts() {
    use aq_server::intelligence::resolver::{self, ResolveRequest};
    let SelectionFixture {
        env,
        input,
        frozen,
        actor,
        lineage,
        manual,
        unrelated,
        time,
        operation,
    } = selection_fixture().await;
    let req = ResolveRequest {
        operation_id: operation,
        lineage: lineage.clone(),
        project: frozen.clone(),
        compositions: input.compositions.clone(),
    };
    let decision = resolver::resolve_trusted(&env.state, actor, req)
        .await
        .unwrap();
    assert!(
        decision.outcome.resolved.is_some(),
        "{:?}",
        decision.outcome.reason_codes
    );
    let mut tx = env.pool.begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await
        .unwrap();
    resolver::fence_for_offer(&env.state, &mut tx, &decision)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    let mut tx = env.pool.begin().await.unwrap();
    assert!(
        resolver::fence_for_offer(&env.state, &mut tx, &decision)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let replay = resolver::resolve_trusted(
        &env.state,
        actor,
        ResolveRequest {
            operation_id: operation,
            lineage: lineage.clone(),
            project: frozen.clone(),
            compositions: input.compositions.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(replay.decision_id, decision.decision_id);
    // Expiry is checked after an observed lock wait, not only before entry.
    let mut held = env.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE")
        .bind(env.node)
        .execute(&mut *held)
        .await
        .unwrap();
    let (pid_tx, pid_rx) = tokio::sync::oneshot::channel();
    let pool = env.pool.clone();
    let state = env.state.clone();
    let frozen_decision = decision.clone();
    let waiting = tokio::spawn(async move {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut *tx)
            .await
            .unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        pid_tx.send(pid).unwrap();
        let result = resolver::fence_for_offer(&state, &mut tx, &frozen_decision).await;
        tx.rollback().await.unwrap();
        result
    });
    let pid = pid_rx.await.unwrap();
    let observer = env._db.pool().await;
    let mut observed = false;
    for _ in 0..100 {
        let blocked: bool = sqlx::query_scalar(
            "SELECT COALESCE(wait_event_type='Lock',false) FROM pg_stat_activity WHERE pid=$1",
        )
        .bind(pid)
        .fetch_one(&observer)
        .await
        .unwrap();
        if blocked {
            observed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(
        observed,
        "fence must be observed waiting on the retained node lock"
    );
    env.time.store((time + 2001) as i64, Ordering::SeqCst);
    held.rollback().await.unwrap();
    let error = waiting.await.unwrap().unwrap_err();
    assert!(format!("{error:?}").contains("expired"));
    env.time.store(time as i64, Ordering::SeqCst);
    // Other-account observations do not poison this decision's scope fence.
    let unrelated_observation = json!({"operation_id":Uuid::new_v4(),"expected_revision":2,"sequence":1,"scope":{"credential_revision":1,"provider":unrelated.provider,"credential_profile":unrelated.profile,"account_reference":unrelated.account_reference,"endpoint_reference":unrelated.endpoint_reference,"model":null,"node_id":null},"state":"quota_unavailable","ttl_ms":30_000,"cooldown_ms":30_000});
    assert_eq!(
        call(
            &env.app,
            "/api/intelligence/observations",
            Some(unrelated_observation),
            "owner@localhost"
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut tx = env.pool.begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await
        .unwrap();
    resolver::fence_for_offer(&env.state, &mut tx, &decision)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    // An applicable change does, even while the original artifact stays intact.
    let mut changed = manual;
    changed["operation_id"] = json!(Uuid::new_v4());
    changed["expected_revision"] = 3.into();
    changed["sequence"] = 2.into();
    changed["state"] = "quota_unavailable".into();
    assert_eq!(
        call(
            &env.app,
            "/api/intelligence/observations",
            Some(changed),
            "owner@localhost"
        )
        .await
        .0,
        StatusCode::OK
    );
    let mut tx = env.pool.begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(
        resolver::fence_for_offer(&env.state, &mut tx, &decision)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let mut tampered = decision.clone();
    tampered.inputs.budget.spent_microunits = Some(0);
    let mut tx = env.pool.begin().await.unwrap();
    assert!(
        resolver::fence_for_offer(&env.state, &mut tx, &tampered)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn project_policy_decisions_have_no_human_actor_and_recheck_installed_policy() {
    use aq_server::{
        intelligence::resolver::{self, ResolveRequest},
        profiled_execution::config::{ProfiledConfig, ProjectPolicy},
    };
    let SelectionFixture {
        mut env,
        input,
        frozen,
        actor,
        lineage,
        operation,
        ..
    } = selection_fixture().await;
    let request = || ResolveRequest {
        operation_id: operation,
        lineage: lineage.clone(),
        project: frozen.clone(),
        compositions: input.compositions.clone(),
    };
    assert!(
        resolver::resolve_for_project_policy(&env.state, request())
            .await
            .is_err()
    );
    let policy = ProfiledConfig {
        definition_bodies: vec![],
        context_qualifications: vec![],
        revision: 1,
        qualified_runtimes: vec![],
        projects: vec![ProjectPolicy {
            project_id: lineage.project_id,
            enabled: true,
            priority: 0,
            source: aq_admission::SourceSelection::Immutable {
                object: aq_admission::ObjectId {
                    format: aq_admission::ObjectFormat::Sha1,
                    hex: "1".repeat(40),
                },
            },
            retention_handle: Uuid::new_v4(),
        }],
    };
    let record = policy.project_policy_record(lineage.project_id).unwrap();
    let reference = record.artifact().unwrap();
    let mut config = (*env.state.config).clone();
    config.profiled_execution = Some(Arc::new(policy.clone()));
    env.state.config = Arc::new(config);
    assert!(
        resolver::resolve_for_project_policy(&env.state, request())
            .await
            .is_err(),
        "configured alone is not installed policy"
    );
    let installed = aq_server::profiled_execution::policies::install(&env.state)
        .await
        .unwrap();
    assert_eq!(installed.installed, 1);
    let human = resolver::resolve_trusted(&env.state, actor, request())
        .await
        .unwrap();
    let decision = resolver::resolve_for_project_policy(&env.state, request())
        .await
        .unwrap();
    assert!(decision.outcome.resolved.is_some());
    assert_ne!(human.decision_id, decision.decision_id);
    let row:(Option<Uuid>,String,Option<Uuid>,Option<Value>,Option<i64>)=sqlx::query_as("SELECT actor_id,authority_kind,authority_project_id,authority_policy_ref,authority_policy_revision FROM intelligence_decisions WHERE decision_id=$1").bind(decision.decision_id).fetch_one(&env.pool).await.unwrap();
    assert_eq!(
        row,
        (
            None,
            "project_policy".into(),
            Some(lineage.project_id),
            Some(json!(reference)),
            Some(1)
        )
    );
    let replay = resolver::resolve_for_project_policy(&env.state, request())
        .await
        .unwrap();
    assert_eq!(replay.decision_id, decision.decision_id);
    let mut changed = request();
    changed.compositions.clear();
    assert!(
        resolver::resolve_for_project_policy(&env.state, changed)
            .await
            .is_err()
    );
    let fence = |state: AppState, decision: resolver::Decision| {
        let pool = env.pool.clone();
        async move {
            let mut tx = pool.begin().await.unwrap();
            sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
                .execute(&mut *tx)
                .await
                .unwrap();
            let result = resolver::fence_for_offer(&state, &mut tx, &decision).await;
            tx.rollback().await.unwrap();
            result
        }
    };
    fence(env.state.clone(), decision.clone()).await.unwrap();
    // The policy lock is acquired after the selection/project fence; expiry
    // still has to be checked after this last authorization wait.
    let mut held = env.pool.begin().await.unwrap();
    sqlx::query("SELECT project_id FROM admission_source_policies WHERE project_id=$1 FOR UPDATE")
        .bind(lineage.project_id)
        .execute(&mut *held)
        .await
        .unwrap();
    let (pid_tx, pid_rx) = tokio::sync::oneshot::channel();
    let pool = env.pool.clone();
    let state = env.state.clone();
    let captured = decision.clone();
    let waiting = tokio::spawn(async move {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut *tx)
            .await
            .unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        pid_tx.send(pid).unwrap();
        let result = resolver::fence_for_offer(&state, &mut tx, &captured).await;
        tx.rollback().await.unwrap();
        result
    });
    let pid = pid_rx.await.unwrap();
    let observer = env._db.pool().await;
    let mut blocked = false;
    for _ in 0..100 {
        blocked = sqlx::query_scalar(
            "SELECT COALESCE(wait_event_type='Lock',false) FROM pg_stat_activity WHERE pid=$1",
        )
        .bind(pid)
        .fetch_one(&observer)
        .await
        .unwrap();
        if blocked {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(
        blocked,
        "policy fence must actually wait for the locked policy"
    );
    let current_time = env.time.load(Ordering::SeqCst);
    env.time.store(current_time + 2001, Ordering::SeqCst);
    held.rollback().await.unwrap();
    assert!(format!("{:?}", waiting.await.unwrap().unwrap_err()).contains("expired"));
    env.time.store(current_time, Ordering::SeqCst);
    // Current human-role revocation affects the human namespace only.
    sqlx::query("UPDATE users SET role='viewer' WHERE id=$1")
        .bind(actor)
        .execute(&env.pool)
        .await
        .unwrap();
    assert!(
        resolver::resolve_trusted(&env.state, actor, request())
            .await
            .is_err()
    );
    assert!(fence(env.state.clone(), human.clone()).await.is_err());
    fence(env.state.clone(), decision.clone()).await.unwrap();
    // A future installed policy defeats both stale daemon state and old decisions.
    let mut newer = policy.clone();
    newer.revision = 2;
    newer.projects[0].enabled = false;
    let mut future = env.state.clone();
    let mut config = (*future.config).clone();
    config.profiled_execution = Some(Arc::new(newer.clone()));
    future.config = Arc::new(config);
    assert_eq!(
        aq_server::profiled_execution::policies::install(&future)
            .await
            .unwrap()
            .disabled,
        1
    );
    assert!(
        resolver::resolve_for_project_policy(&env.state, request())
            .await
            .is_err()
    );
    assert!(fence(env.state.clone(), decision.clone()).await.is_err());
    let mut config = (*env.state.config).clone();
    config.profiled_execution = Some(Arc::new(newer));
    env.state.config = Arc::new(config);
    assert!(
        resolver::resolve_for_project_policy(&env.state, request())
            .await
            .is_err()
    );
    let retained: Value =
        sqlx::query_scalar("SELECT decision FROM intelligence_decisions WHERE decision_id=$1")
            .bind(decision.decision_id)
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(retained, json!(decision));
    assert!(sqlx::query("UPDATE intelligence_decisions SET authority_kind='human_owner',actor_id=$2 WHERE decision_id=$1").bind(decision.decision_id).bind(actor).execute(&env.pool).await.is_err());
}

#[tokio::test]
async fn policy_install_is_atomic_retains_tombstones_and_refuses_configuration_rollback() {
    use aq_server::profiled_execution::{
        config::{ProfiledConfig, ProjectPolicy},
        policies,
    };
    let SelectionFixture {
        mut env, lineage, ..
    } = selection_fixture().await;
    let source = aq_admission::SourceSelection::Immutable {
        object: aq_admission::ObjectId {
            format: aq_admission::ObjectFormat::Sha1,
            hex: "a".repeat(40),
        },
    };
    let configured = ProfiledConfig {
        definition_bodies: vec![],
        context_qualifications: vec![],
        revision: 1,
        qualified_runtimes: vec![],
        projects: vec![
            ProjectPolicy {
                project_id: lineage.project_id,
                enabled: true,
                priority: 0,
                source: source.clone(),
                retention_handle: Uuid::new_v4(),
            },
            ProjectPolicy {
                project_id: Uuid::new_v4(),
                enabled: true,
                priority: 0,
                source,
                retention_handle: Uuid::new_v4(),
            },
        ],
    };
    let mut config = (*env.state.config).clone();
    config.profiled_execution = Some(Arc::new(configured.clone()));
    env.state.config = Arc::new(config);
    let first = policies::install(&env.state).await.unwrap();
    assert_eq!((first.installed, first.disabled, first.unbound), (1, 0, 1));
    let retry = policies::install(&env.state).await.unwrap();
    assert_eq!((retry.installed, retry.disabled, retry.unbound), (0, 0, 1));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM admission_source_policies")
        .fetch_one(&env.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let newer = ProfiledConfig {
        definition_bodies: vec![],
        context_qualifications: vec![],
        revision: 2,
        qualified_runtimes: vec![],
        projects: vec![],
    };
    let mut future = env.state.clone();
    let mut config = (*future.config).clone();
    config.profiled_execution = Some(Arc::new(newer));
    future.config = Arc::new(config);
    sqlx::raw_sql("CREATE FUNCTION test_policy_failure() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic policy write failure'; END $$; CREATE TRIGGER test_policy_failure BEFORE UPDATE ON admission_source_policies FOR EACH ROW EXECUTE FUNCTION test_policy_failure();").execute(&env.pool).await.unwrap();
    assert!(policies::install(&future).await.is_err());
    let meta: String =
        sqlx::query_scalar("SELECT value FROM meta WHERE key='profiled_policy_configuration_v1'")
            .fetch_one(&env.pool)
            .await
            .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&meta).unwrap()["revision"], 1);
    let enabled: bool = sqlx::query_scalar(
        "SELECT (policy->>'enabled')::boolean FROM admission_source_policies WHERE project_id=$1",
    )
    .bind(lineage.project_id)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert!(enabled);
    sqlx::query("DROP TRIGGER test_policy_failure ON admission_source_policies")
        .execute(&env.pool)
        .await
        .unwrap();
    assert_eq!(policies::install(&future).await.unwrap().disabled, 1);
    let enabled: bool = sqlx::query_scalar(
        "SELECT (policy->>'enabled')::boolean FROM admission_source_policies WHERE project_id=$1",
    )
    .bind(lineage.project_id)
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert!(!enabled);
    assert!(
        policies::install(&env.state).await.is_err(),
        "older daemon cannot reinstall removed projects"
    );
    let mut changed = configured;
    changed.revision = 2;
    let mut config = (*env.state.config).clone();
    config.profiled_execution = Some(Arc::new(changed));
    env.state.config = Arc::new(config);
    assert!(
        policies::install(&env.state).await.is_err(),
        "same revision changed content refuses"
    );
    assert_eq!(policies::install(&future).await.unwrap().disabled, 0);
}
#[tokio::test]
async fn empty_policy_configuration_still_retains_global_revision_owner() {
    use aq_server::profiled_execution::{config::ProfiledConfig, policies};
    let mut env = setup(false).await;
    let mut config = (*env.state.config).clone();
    config.profiled_execution = Some(Arc::new(ProfiledConfig {
        definition_bodies: vec![],
        context_qualifications: vec![],
        revision: 10,
        qualified_runtimes: vec![],
        projects: vec![],
    }));
    env.state.config = Arc::new(config);
    assert_eq!(
        policies::install(&env.state).await.unwrap().revision,
        Some(10)
    );
    let mut config = (*env.state.config).clone();
    config.profiled_execution = Some(Arc::new(ProfiledConfig {
        definition_bodies: vec![],
        context_qualifications: vec![],
        revision: 9,
        qualified_runtimes: vec![],
        projects: vec![],
    }));
    env.state.config = Arc::new(config);
    assert!(policies::install(&env.state).await.is_err());
}

#[tokio::test]
async fn committed_binding_installs_pending_policy_and_historical_ack_survives_retirement() {
    use aq_server::profiled_execution::{
        config::{ProfiledConfig, ProjectPolicy},
        policies,
    };
    let mut env = setup(false).await;
    let configured = ProfiledConfig {
        definition_bodies: vec![],
        context_qualifications: vec![],
        revision: 1,
        qualified_runtimes: vec![],
        projects: vec![ProjectPolicy {
            project_id: Uuid::from_u128(1),
            enabled: true,
            priority: 0,
            source: aq_admission::SourceSelection::Immutable {
                object: aq_admission::ObjectId {
                    format: aq_admission::ObjectFormat::Sha1,
                    hex: "a".repeat(40),
                },
            },
            retention_handle: Uuid::new_v4(),
        }],
    };
    let mut cfg = (*env.state.config).clone();
    cfg.profiled_execution = Some(Arc::new(configured));
    env.state.config = Arc::new(cfg);
    env.app = aq_server::build_admin_router(env.state.clone());
    assert_eq!(policies::install(&env.state).await.unwrap().unbound, 1);
    let request = json!({"operation_id":Uuid::new_v4(),"expected_revision":0,"binding":env.body});
    let (status, first) = call(&env.app, &path(), Some(request.clone()), "owner@localhost").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let enabled: bool = sqlx::query_scalar(
        "SELECT (policy->>'enabled')::boolean FROM admission_source_policies WHERE project_id=$1",
    )
    .bind(Uuid::from_u128(1))
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert!(
        enabled,
        "post-commit installer did not publish the previously unbound policy"
    );
    let mut newer = env.state.clone();
    let mut cfg = (*newer.config).clone();
    cfg.profiled_execution = Some(Arc::new(ProfiledConfig {
        definition_bodies: vec![],
        context_qualifications: vec![],
        revision: 2,
        qualified_runtimes: vec![],
        projects: vec![],
    }));
    newer.config = Arc::new(cfg);
    policies::install(&newer).await.unwrap();
    let (status, retry) = call(&env.app, &path(), Some(request), "owner@localhost").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry, first);
    let enabled: bool = sqlx::query_scalar(
        "SELECT (policy->>'enabled')::boolean FROM admission_source_policies WHERE project_id=$1",
    )
    .bind(Uuid::from_u128(1))
    .fetch_one(&env.pool)
    .await
    .unwrap();
    assert!(!enabled, "historical ack reinstalled a retired policy");
}
