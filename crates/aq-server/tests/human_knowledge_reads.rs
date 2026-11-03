//! G11 Knowledge view reads (X35 relationships, X36 revisions, X37 list total)
//! over a fake public Graph API.
mod common;
use aq_server::agent_access::{AgentAccessConfig, Runtime, human};
use axum::{
    Router,
    body::Body,
    extract::{Path, Query, State},
    http::{Request, StatusCode},
    response::Response,
    routing::get,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    os::unix::fs::PermissionsExt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone, Default)]
struct Graph {
    /// Every node in Graph order.
    nodes: Arc<Mutex<Vec<Value>>>,
    context: Arc<Mutex<HashMap<Uuid, Value>>>,
    history: Arc<Mutex<HashMap<Uuid, Vec<Value>>>>,
    /// Upstream may legally return fewer rows than requested.
    page_cap: Arc<AtomicUsize>,
    fail_count_pages: Arc<AtomicBool>,
}
fn reply(status: u16, body: &Value) -> Response {
    Response::builder()
        .status(status)
        .header("X-AQ-Graph-API-Version", "1")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
fn missing() -> Response {
    reply(
        404,
        &json!({"error":{"code":"not_found","message":"missing"}}),
    )
}
fn page(rows: Vec<Value>, q: &HashMap<String, String>, cap: usize) -> Value {
    let limit: usize = q["limit"].parse().unwrap();
    let start: usize = q.get("cursor").map_or(0, |c| c[1..].parse().unwrap());
    let take = limit.min(cap.max(1));
    let end = (start + take).min(rows.len());
    let next = (end < rows.len()).then(|| format!("o{end}"));
    json!({"items": rows[start..end], "next_cursor": next})
}
fn listed(g: &Graph, q: &HashMap<String, String>) -> Option<Response> {
    if q["limit"] == "200" && g.fail_count_pages.load(Ordering::SeqCst) {
        return Some(reply(503, &json!({"error":{"code":"down","message":"x"}})));
    }
    let rows: Vec<Value> = g
        .nodes
        .lock()
        .unwrap()
        .iter()
        .filter(|n| q.get("kind").is_none_or(|k| n["kind"] == k.as_str()))
        .filter(|n| {
            q.get("q")
                .is_none_or(|t| n["title"].as_str().unwrap().contains(t.as_str()))
        })
        .cloned()
        .collect();
    Some(reply(
        200,
        &page(rows, q, g.page_cap.load(Ordering::SeqCst)),
    ))
}
async fn nodes(State(g): State<Graph>, Query(q): Query<HashMap<String, String>>) -> Response {
    listed(&g, &q).unwrap()
}
async fn search(State(g): State<Graph>, Query(q): Query<HashMap<String, String>>) -> Response {
    assert!(q.contains_key("q"));
    listed(&g, &q).unwrap()
}
async fn one(State(g): State<Graph>, Path((_, id)): Path<(Uuid, Uuid)>) -> Response {
    match g
        .nodes
        .lock()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!(id))
    {
        Some(n) => reply(200, n),
        None => missing(),
    }
}
async fn context(State(g): State<Graph>, Path((_, id)): Path<(Uuid, Uuid)>) -> Response {
    match g.context.lock().unwrap().get(&id) {
        Some(c) => reply(200, c),
        None => missing(),
    }
}
async fn history(
    State(g): State<Graph>,
    Path((_, id)): Path<(Uuid, Uuid)>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let rows = g
        .history
        .lock()
        .unwrap()
        .get(&id)
        .cloned()
        .unwrap_or_default();
    reply(200, &page(rows, &q, usize::MAX))
}

fn node(id: Uuid, workspace: Uuid, kind: &str, title: &str) -> Value {
    let knowledge = kind == "knowledge";
    json!({"id":id,"workspace_id":workspace,"kind":kind,"version":3,"content_revision":2,
        "title":title,"body":"secret-body-text","acceptance_criteria":"","labels":[],
        "metadata":{"example:weight":2},"task_state":if knowledge {Value::Null} else {json!("active")},
        "verification":"unverified","provenance":[],"retired":false,"deleted":false,
        "created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","created_by":"fixture",
        "last_verified_by":null,"last_verified_at":null,"retirement_reason":null,
        "content_type":"note","summary":null,
        "summary_of_revision":null,"valid_from":null,"valid_until":null,"review_at":null,"successor_id":null})
}
fn edge(workspace: Uuid, source: Uuid, target: Uuid, edge_type: &str, deleted: bool) -> Value {
    json!({"id":Uuid::new_v4(),"workspace_id":workspace,"source":source,"target":target,
        "edge_type":edge_type,"source_revision":null,"target_revision":2,"version":1,
        "deleted":deleted,"created_at":"2026-01-01T00:00:00Z","description":"why"})
}

struct Fixture {
    f: common::profiled::Fixture,
    graph: Graph,
    app: Router,
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
    graph.page_cap.store(usize::MAX, Ordering::SeqCst);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let upstream = Router::new()
        .route("/v1/workspaces/{w}/nodes", get(nodes))
        .route("/v1/workspaces/{w}/search", get(search))
        .route("/v1/workspaces/{w}/nodes/{id}", get(one))
        .route("/v1/workspaces/{w}/nodes/{id}/context", get(context))
        .route("/v1/workspaces/{w}/nodes/{id}/history", get(history))
        .with_state(graph.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, upstream).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("graph-key");
    std::fs::write(&key, b"synthetic-graph-service-key-000000000000").unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    f.state.agent_access = Some(Arc::new(
        Runtime::new(&AgentAccessConfig {
            graph_url: url,
            graph_token_file: key,
            mutation_actor_id: None,
            credential_ttl_ms: 120000,
        })
        .await
        .unwrap(),
    ));
    let app = human::routes(&f.state).with_state(f.state.clone());
    Fixture {
        f,
        graph,
        app,
        server,
        _dir: dir,
    }
}
impl Fixture {
    fn workspace(&self) -> Uuid {
        self.f.selection.lineage.workspace_id
    }
    fn base(&self) -> String {
        format!(
            "/api/human/projects/{}",
            self.f.selection.lineage.project_id
        )
    }
    fn add(&self, kind: &str, title: &str) -> Uuid {
        let id = Uuid::new_v4();
        let n = node(id, self.workspace(), kind, title);
        self.graph.nodes.lock().unwrap().push(n);
        id
    }
    fn get(&self, id: Uuid) -> Value {
        let nodes = self.graph.nodes.lock().unwrap();
        nodes.iter().find(|n| n["id"] == json!(id)).unwrap().clone()
    }
    async fn read(&self, path: &str) -> (StatusCode, Value) {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::get(format!("{}{path}", self.base()))
                    .header("x-aq-dev-email", "owner@localhost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"text":String::from_utf8_lossy(&bytes)}));
        if status == StatusCode::OK {
            let schema: Value =
                serde_json::from_str(include_str!("../../../protocol/human-graph/v1/schema.json"))
                    .unwrap();
            assert!(
                aq_agent_api::check(&schema, &schema, &body),
                "schema: {body}"
            );
        }
        (status, body)
    }
}

#[tokio::test]
async fn knowledge_list_total_is_exact_estimated_or_unknown_never_zero() {
    let f = setup().await;
    for title in ["Alpha note", "Beta note", "Gamma memo"] {
        f.add("knowledge", title);
    }
    f.add("task", "Alpha task");
    let (status, first) = f.read("/knowledge?limit=2").await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["data"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        first["data"]["total"],
        json!({"value":3,"accuracy":"exact"})
    );
    let cursor = first["data"]["next_cursor"].as_str().unwrap();
    let (_, second) = f.read(&format!("/knowledge?limit=2&cursor={cursor}")).await;
    assert_eq!(second["data"]["items"].as_array().unwrap().len(), 1);
    assert!(second["data"].get("total").is_none(), "{second}");
    let (_, filtered) = f.read("/knowledge?q=note").await;
    assert_eq!(
        filtered["data"]["total"],
        json!({"value":2,"accuracy":"exact"})
    );
    let (_, tasks) = f.read("/tasks").await;
    assert!(tasks["data"].get("total").is_none(), "{tasks}");

    // Several upstream pages may straddle concurrent writes.
    f.graph.page_cap.store(2, Ordering::SeqCst);
    let (_, paged) = f.read("/knowledge?limit=2").await;
    assert_eq!(
        paged["data"]["total"],
        json!({"value":3,"accuracy":"estimated"})
    );
    // Beyond four upstream pages the bound is exceeded: unknown, not a guess.
    for n in 0..6 {
        f.add("knowledge", &format!("Extra {n}"));
    }
    let (_, over) = f.read("/knowledge?limit=2").await;
    assert_eq!(
        over["data"]["total"],
        json!({"value":null,"accuracy":"unknown"})
    );
    // A failed count never fails the list and is never reported as zero.
    f.graph.page_cap.store(usize::MAX, Ordering::SeqCst);
    f.graph.fail_count_pages.store(true, Ordering::SeqCst);
    let (status, failed) = f.read("/knowledge?limit=2").await;
    assert_eq!(status, StatusCode::OK, "{failed}");
    assert_eq!(failed["data"]["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        failed["data"]["total"],
        json!({"value":null,"accuracy":"unknown"})
    );
}

#[tokio::test]
async fn knowledge_relationships_return_edges_linked_work_and_counts() {
    let f = setup().await;
    let w = f.workspace();
    let doc = f.add("knowledge", "Design note");
    let producer = f.add("task", "Producing task");
    let reader = f.add("task", "Reading task");
    let related = f.add("knowledge", "Related note");
    let removed = f.add("task", "Removed link");
    let produces = edge(w, producer, doc, "produces", false);
    let context = json!({
        "node": f.get(doc),
        "nodes": [f.get(producer), f.get(reader), f.get(related), f.get(removed)],
        "edges": [
            produces,
            edge(w, doc, reader, "references", false),
            edge(w, doc, related, "supports", false),
            edge(w, doc, removed, "references", true),
            // Not incident to the document: not part of its relationship row.
            edge(w, producer, related, "blocks", false),
        ],
        "pinned_revisions": [],
        "truncated": false,
    });
    f.graph.context.lock().unwrap().insert(doc, context.clone());
    let (status, body) = f.read(&format!("/knowledge/{doc}/relationships")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let data = &body["data"];
    assert_eq!(data["root_id"], json!(doc));
    assert_eq!(data["nodes"][0]["id"], json!(doc));
    assert_eq!(data["edges"].as_array().unwrap().len(), 3);
    assert_eq!(data["linked_work"], json!([producer, reader]));
    assert_eq!(
        data["counts"],
        json!({"relationships":{"value":3,"accuracy":"exact"},
               "linked_work":{"value":2,"accuracy":"exact"}})
    );
    assert_eq!(data["truncated"], false);
    assert_eq!(data["edges"][0]["edge_type"], "produces");
    assert_eq!(data["edges"][0]["target_revision"], "2");
    assert!(!body.to_string().contains("example:weight"));

    let mut cut = context;
    cut["truncated"] = json!(true);
    f.graph.context.lock().unwrap().insert(doc, cut);
    let (_, partial) = f.read(&format!("/knowledge/{doc}/relationships")).await;
    assert_eq!(
        partial["data"]["counts"]["linked_work"],
        json!({"value":null,"accuracy":"unknown"})
    );

    // A task root is not a knowledge document.
    let task_context = json!({"node": f.get(producer), "nodes": [], "edges": [],
        "pinned_revisions": [], "truncated": false});
    f.graph
        .context
        .lock()
        .unwrap()
        .insert(producer, task_context);
    let (status, _) = f
        .read(&format!("/knowledge/{producer}/relationships"))
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, _) = f
        .read(&format!("/knowledge/{}/relationships", Uuid::new_v4()))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn knowledge_revisions_list_history_without_bodies() {
    let f = setup().await;
    let w = f.workspace();
    let doc = f.add("knowledge", "Design note");
    let other = f.add("knowledge", "Other note");
    let task = f.add("task", "Linked task");
    let mut snapshot = f.get(doc);
    snapshot["content_revision"] = json!(1);
    snapshot["version"] = json!(1);
    let change = Uuid::new_v4();
    let entry = |sequence: u64, category: &str, operation: &str, data: Value| {
        json!({"sequence":sequence,"entry_id":Uuid::new_v4(),"category":category,
            "actor_id":"agent:writer","operation":operation,"reason":"because",
            "change_set_id":change,"data":data,"created_at":"2026-01-02T00:00:00Z"})
    };
    let link = edge(w, task, doc, "produces", false);
    f.graph.history.lock().unwrap().insert(
        doc,
        vec![
            entry(1, "revision", "created", snapshot),
            entry(2, "edge", "added", link.clone()),
            entry(
                3,
                "lifecycle",
                "retired",
                json!({"from_revision":1,"version":3,"task_state":null,"retired":true,"deleted":false}),
            ),
        ],
    );
    let (status, first) = f.read(&format!("/knowledge/{doc}/revisions?limit=2")).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let items = first["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["sequence"], "1");
    assert_eq!(items[0]["revision"], "1");
    assert_eq!(items[0]["version"], "1");
    assert_eq!(items[0]["title"], "Design note");
    assert_eq!(items[0]["edge"], Value::Null);
    assert_eq!(
        items[1]["edge"],
        json!({"id":link["id"],"edge_type":"produces","source":task,"target":doc})
    );
    assert_eq!(items[1]["revision"], Value::Null);
    let text = first.to_string();
    assert!(!text.contains("secret-body-text"), "{text}");
    assert!(!text.contains("example:weight"), "{text}");
    assert!(
        !text.contains("o2"),
        "upstream cursor must not escape: {text}"
    );

    let cursor = first["data"]["next_cursor"].as_str().unwrap();
    let (_, second) = f
        .read(&format!(
            "/knowledge/{doc}/revisions?limit=2&cursor={cursor}"
        ))
        .await;
    assert_eq!(second["data"]["items"][0]["category"], "lifecycle");
    assert_eq!(second["data"]["items"][0]["version"], "3");
    assert_eq!(second["data"]["next_cursor"], Value::Null);
    // The cursor binds the node it was issued for.
    let (status, _) = f
        .read(&format!(
            "/knowledge/{other}/revisions?limit=2&cursor={cursor}"
        ))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = f.read(&format!("/knowledge/{task}/revisions")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (status, _) = f
        .read(&format!("/knowledge/{doc}/revisions?limit=65"))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
