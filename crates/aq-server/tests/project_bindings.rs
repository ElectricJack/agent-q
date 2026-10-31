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
