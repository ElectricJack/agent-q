//! G2 typed task and knowledge commands over a fake public Graph API.
mod common;
use aq_server::agent_access::{AgentAccessConfig, Runtime};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Request, StatusCode},
    response::Response,
    routing::{get, post},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    os::unix::fs::PermissionsExt,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone, Default)]
struct Graph {
    nodes: Arc<Mutex<HashMap<Uuid, Value>>>,
    posts: Arc<Mutex<Vec<Value>>>,
    reply: Arc<Mutex<Option<(u16, Value)>>>,
}
fn graph_response(status: u16, body: &Value) -> Response {
    Response::builder()
        .status(status)
        .header("X-AQ-Graph-API-Version", "1")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
async fn read_node(State(g): State<Graph>, Path((_, id)): Path<(Uuid, Uuid)>) -> Response {
    match g.nodes.lock().unwrap().get(&id) {
        Some(n) => graph_response(200, n),
        None => graph_response(
            404,
            &json!({"error":{"code":"not_found","message":"missing"}}),
        ),
    }
}
async fn mutate(State(g): State<Graph>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    assert_eq!(headers["X-AQ-Graph-API-Version"], "1");
    g.posts.lock().unwrap().push(body);
    let (status, reply) = g.reply.lock().unwrap().clone().expect("configured reply");
    graph_response(status, &reply)
}

struct Fixture {
    f: common::profiled::Fixture,
    graph: Graph,
    server: tokio::task::JoinHandle<()>,
    _dir: tempfile::TempDir,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn setup() -> Fixture {
    let mut f = common::profiled::fixture().await;
    common::profiled::insert(&f).await;
    let graph = Graph::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let app = Router::new()
        .route("/v1/workspaces/{w}/nodes/{id}", get(read_node))
        .route("/v1/workspaces/{w}/mutations", post(mutate))
        .with_state(graph.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("graph-key");
    std::fs::write(&key, b"synthetic-graph-service-key-000000000000").unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let runtime = Runtime::new(&AgentAccessConfig {
        graph_url: url,
        graph_token_file: key,
        mutation_actor_id: Some("dashboard-fixture-service".into()),
        credential_ttl_ms: 120000,
    })
    .await
    .unwrap();
    f.state.agent_access = Some(Arc::new(runtime));
    Fixture {
        f,
        graph,
        server,
        _dir: dir,
    }
}
impl Fixture {
    fn workspace(&self) -> Uuid {
        self.f.selection.lineage.workspace_id
    }
    fn project(&self) -> Uuid {
        self.f.selection.lineage.project_id
    }
    fn put(&self, node: Value) {
        let id = node["id"].as_str().unwrap().parse().unwrap();
        self.graph.nodes.lock().unwrap().insert(id, node);
    }
    fn reply(&self, status: u16, body: Value) {
        *self.graph.reply.lock().unwrap() = Some((status, body));
    }
    fn posts(&self) -> Vec<Value> {
        self.graph.posts.lock().unwrap().clone()
    }
    async fn act(&self, action: &Value) -> (StatusCode, Value) {
        let app = aq_server::build_admin_router(self.f.state.clone());
        let res = app
            .oneshot(
                Request::post("/api/v1/actions")
                    .header("x-aq-dev-email", "owner@localhost")
                    .header("x-aq-request", "1")
                    .header("content-type", "application/json")
                    .body(Body::from(action.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
}
fn node(id: Uuid, workspace: Uuid, kind: &str, version: u64) -> Value {
    let knowledge = kind == "knowledge";
    json!({"id":id,"workspace_id":workspace,"kind":kind,"version":version,"content_revision":version,
        "title":"Original title","body":"Original body","acceptance_criteria":"","labels":[],
        "metadata":{"example:weight":2},"task_state":if knowledge {Value::Null} else {json!("active")},
        "verification":"unverified","provenance":[],"retired":false,"deleted":false,
        "created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","created_by":"fixture",
        "last_verified_by":null,"last_verified_at":null,"retirement_reason":null,
        "content_type":if knowledge {json!("note")} else {Value::Null},"summary":null,
        "summary_of_revision":null,"valid_from":null,"valid_until":null,"review_at":null,"successor_id":null})
}
fn action(kind: &str, entity: &str, id: Uuid, revision: &str, payload: Value) -> Value {
    json!({"operation_id":payload["operation_id"],"kind":kind,
        "subject":{"type":entity,"id":id,"revision":revision},
        "expected_versions":{entity:revision},"payload":payload})
}
fn change_set(results: Vec<Value>) -> Value {
    json!({"change_set_id":Uuid::new_v4(),"results":results})
}

#[tokio::test]
async fn knowledge_correct_with_stale_version_conflicts() {
    let x = setup().await;
    let id = Uuid::new_v4();
    x.put(node(id, x.workspace(), "knowledge", 3));
    let op = Uuid::new_v4();
    let (status, body) = x
        .act(&action(
            "knowledge.correct",
            "knowledge",
            id,
            "2",
            json!({"operation_id":op,"project_id":x.project(),"expected_version":"2","title":"Corrected"}),
        ))
        .await;
    assert_eq!(body["data"]["disposition"], "conflict", "{status} {body}");
    assert_eq!(body["data"]["current_versions"], json!({"knowledge":"3"}));
    assert_eq!(body["data"]["recovery"], "refresh_and_review");
    assert!(x.posts().is_empty(), "a stale version never reaches Graph");
}

#[tokio::test]
async fn knowledge_correct_confirms_with_change_set() {
    let x = setup().await;
    let id = Uuid::new_v4();
    x.put(node(id, x.workspace(), "knowledge", 3));
    let mut updated = node(id, x.workspace(), "knowledge", 4);
    updated["title"] = json!("Corrected");
    let result = change_set(vec![updated]);
    x.reply(200, result.clone());
    let op = Uuid::new_v4();
    let (status, body) = x
        .act(&action(
            "knowledge.correct",
            "knowledge",
            id,
            "3",
            json!({"operation_id":op,"project_id":x.project(),"expected_version":"3","title":"Corrected"}),
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["disposition"], "confirmed", "{body}");
    assert_eq!(body["data"]["receipt_refs"][0]["type"], "graph_change_set");
    assert_eq!(
        body["data"]["receipt_refs"][0]["id"],
        result["change_set_id"]
    );
    assert_eq!(
        body["data"]["affected_entities"][0],
        json!({"type":"knowledge","id":id,"revision":"4"})
    );
    let posts = x.posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(
        posts[0]["idempotency_key"],
        format!("aq-dashboard-action:{op}")
    );
    assert_eq!(
        posts[0]["operations"],
        json!([{"op":"update_node","id":id,"expected_version":3,"patch":{"title":"Corrected"}}])
    );
}

#[tokio::test]
async fn task_reprioritize_preserves_other_metadata() {
    let x = setup().await;
    let task = x.f.selection.lineage.task_id;
    x.put(node(task, x.workspace(), "task", 5));
    let mut updated = node(task, x.workspace(), "task", 6);
    updated["metadata"]["aq:priority"] = json!(7);
    x.reply(200, change_set(vec![updated]));
    let op = Uuid::new_v4();
    let (status, body) = x
        .act(&action(
            "task.reprioritize",
            "task",
            task,
            "5",
            json!({"operation_id":op,"project_id":x.project(),"expected_version":"5","priority":7}),
        ))
        .await;
    assert_eq!(body["data"]["disposition"], "confirmed", "{status} {body}");
    let posts = x.posts();
    assert_eq!(
        posts[0]["operations"][0]["patch"],
        json!({"metadata":{"example:weight":2,"aq:priority":7}})
    );
}

#[tokio::test]
async fn knowledge_verify_forbidden_is_rejected() {
    let x = setup().await;
    let id = Uuid::new_v4();
    x.put(node(id, x.workspace(), "knowledge", 2));
    x.reply(
        403,
        json!({"error":{"code":"forbidden","message":"admin scope required"}}),
    );
    let op = Uuid::new_v4();
    let mut request = action(
        "knowledge.verify",
        "knowledge",
        id,
        "2",
        json!({"operation_id":op,"project_id":x.project(),"expected_version":"2","verification":"verified",
            "evidence":[{"source_id":"review-1","kind":"url","url":"https://example.test/review","observed_at":"2026-01-01T00:00:00Z","retained":false}]}),
    );
    let (status, missing_reason) = x.act(&request).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "verification needs a reason: {missing_reason}"
    );
    request["reason"] = json!("Reviewed against the source");
    let (status, body) = x.act(&request).await;
    assert_eq!(body["data"]["disposition"], "rejected", "{status} {body}");
    assert_eq!(x.posts()[0]["operations"][0]["op"], "verify_node");
}

#[tokio::test]
async fn knowledge_author_creates_unverified_note() {
    let x = setup().await;
    let op = Uuid::new_v4();
    let mut created = node(op, x.workspace(), "knowledge", 1);
    created["title"] = json!("New note");
    x.reply(200, change_set(vec![created]));
    let (status, body) = x
        .act(&action(
            "knowledge.author",
            "project",
            x.project(),
            "1",
            json!({"operation_id":op,"project_id":x.project(),"expected_project_revision":"1","title":"New note","body":"Durable finding"}),
        ))
        .await;
    assert_eq!(body["data"]["disposition"], "confirmed", "{status} {body}");
    let operation = &x.posts()[0]["operations"][0];
    assert_eq!(operation["op"], "create_node");
    assert_eq!(operation["id"], json!(op));
    assert_eq!(operation["kind"], "knowledge");
    assert_eq!(operation["verification"], "unverified");
    assert_eq!(operation["content_type"], "note");
    assert_eq!(operation["metadata"]["aq:project_id"], json!(x.project()));
}

#[tokio::test]
async fn task_retry_creates_replacement_attempt() {
    use aq_server::{
        execution::{
            authority::hot::{HotState, LaunchOwner},
            profiled::{Effects, Retention, Status, Stop},
        },
        profiled_execution::replacement::{self, Assessment},
    };
    let x = setup().await;
    let pool = x.f.db.pool().await;
    let task = x.f.selection.lineage.task_id;
    let predecessor = x.f.profiled.offer.binding.attempt_id;
    x.put(node(task, x.workspace(), "task", 1));
    // Synthetic B4 history: the fixture launch was offered at NOW.
    sqlx::query("ALTER TABLE profiled_launches DISABLE TRIGGER profiled_launch_guard")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE profiled_launches SET graph_task_observation=jsonb_set(graph_task_observation,'{native_offer}',jsonb_build_object('offered',jsonb_build_object('clock',jsonb_build_object('server_time_ms',$2::BIGINT)))) WHERE attempt_id=$1")
        .bind(predecessor).bind(common::profiled::NOW as i64).execute(&pool).await.unwrap();
    sqlx::query("ALTER TABLE profiled_launches ENABLE TRIGGER profiled_launch_guard")
        .execute(&pool)
        .await
        .unwrap();
    let mut hot = HotState::from_compat(&x.f.profiled).unwrap();
    sqlx::query("INSERT INTO profiled_authority_hot(attempt_id,state,record_revision,authority_revision,grant_revision) VALUES($1,$2,1,1,0)").bind(predecessor).bind(json!(hot)).execute(&pool).await.unwrap();
    let holds = || async {
        sqlx::query_scalar::<_, bool>(
            "SELECT holds_task FROM execution_attempts WHERE attempt_id=$1",
        )
        .bind(predecessor)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let retry = |op: Uuid| {
        action(
            "task.retry",
            "task",
            task,
            "1",
            json!({"operation_id":op,"project_id":x.project(),"expected_version":"1"}),
        )
    };

    // B4 alone decides: an unsettled predecessor is never force-replaced.
    let (status, body) = x.act(&retry(Uuid::new_v4())).await;
    assert_eq!(body["data"]["disposition"], "rejected", "{status} {body}");
    assert!(holds().await, "rejected retry leaves the claim untouched");

    hot.status = Status::Finished;
    hot.owner = Some(LaunchOwner {
        guardian_id: Uuid::new_v4(),
        launch_id: Uuid::new_v4(),
        reservation_id: Uuid::new_v4(),
    });
    hot.stop = Stop::Stopped {
        receipt: common::profiled::artifact("stop", b"stop"),
    };
    hot.retention = Retention::Retained {
        receipt: common::profiled::artifact("retained", b"retained"),
    };
    hot.effects = Effects::Quiescent {
        receipt: common::profiled::artifact("effects", b"effects"),
    };
    hot.validate_against(&x.f.profiled.offer.binding).unwrap();
    sqlx::query("UPDATE profiled_authority_hot SET state=$2 WHERE attempt_id=$1")
        .bind(predecessor)
        .bind(json!(hot))
        .execute(&pool)
        .await
        .unwrap();

    let (status, body) = x.act(&retry(Uuid::new_v4())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"]["disposition"], "confirmed", "{body}");
    assert_eq!(
        body["data"]["receipt_refs"][0],
        json!({"type":"replacement_release","id":predecessor})
    );
    assert!(!holds().await, "B4 released the failed predecessor");
    let released: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM profiled_replacement_releases WHERE attempt_id=$1 AND task_id=$2",
    )
    .bind(predecessor)
    .bind(task)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(released, 1);
    let Assessment::Replace(next) =
        replacement::assess_task(&x.f.state, task, common::profiled::NOW)
            .await
            .unwrap()
    else {
        panic!("scheduler may start the replacement attempt");
    };
    assert_eq!(next.predecessor, predecessor);
    assert!(!next.needs_release);
    assert!(
        x.posts().is_empty(),
        "retry never writes Graph task lifecycle"
    );
}
