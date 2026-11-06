//! Spec 0006 phase 1: the generic command surface on both listeners. An
//! agent reaches its commands through `/api/v1/commands` with the same
//! grants, refusals and audit as the `/v1/agent/*` routes; the owner reaches
//! the same handlers as the REST routes.
use crate::messaging::{send, setup_graph, setup_with};
use axum::http::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

fn grants() -> Value {
    json!([
        {"operation":"comment.write","scope":"task:subtree"},
        {"operation":"comment.read","scope":"project:self"},
        {"operation":"message.send","scope":"project:self"}
    ])
}
async fn audit_rows(e: &crate::messaging::Env, command: &str) -> Vec<(String, Option<String>)> {
    sqlx::query_as("SELECT decision,reason_code FROM command_audit WHERE attempt_id=$1 AND command=$2 ORDER BY recorded_at_ms")
        .bind(Uuid::parse_str(e.binding["attempt_id"].as_str().unwrap()).unwrap())
        .bind(command)
        .fetch_all(&e.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn agent_whoami_and_discovery_follow_its_grants() {
    let e = setup_with(Some(grants())).await;
    let (status, me) = send(
        &e.app,
        "/api/v1/commands/whoami",
        Some(&e.token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["principal"], "agent");
    assert_eq!(me["attempt_id"], e.binding["attempt_id"]);
    assert_eq!(me["task_id"], e.binding["task_id"]);
    assert_eq!(me["project_id"], e.binding["project_id"]);
    assert_eq!(me["grants"].as_array().unwrap().len(), 3);
    assert!(me["grants_digest"].as_str().unwrap().starts_with("sha256:"));
    assert!(me["credential"]["expires_at"].is_string());
    // Enough to address its own session mailbox and the supervisor.
    assert_eq!(me["session_id"], e.binding["session_id"]);
    assert_eq!(me["session_generation"], e.binding["generation"]);
    let supervisor: Option<Uuid> = sqlx::query_scalar(
        "SELECT conversation_id FROM global_supervisor_conversation WHERE singleton",
    )
    .fetch_optional(&e.pool)
    .await
    .unwrap();
    assert_eq!(me["supervisor_conversation_id"], json!(supervisor));

    let (status, list) = send(&e.app, "/api/v1/commands", Some(&e.token), None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let names: Vec<&str> = list["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    for expected in [
        "whoami",
        "operation.show",
        "task.comment.add",
        "comment.list",
        "message.send",
    ] {
        assert!(names.contains(&expected), "{expected} in {names:?}");
    }
    // Published means runnable: no grant, no entry.
    for absent in [
        "question.ask",
        "wait.register",
        "task.show",
        "decision.resolve",
    ] {
        assert!(!names.contains(&absent), "{absent}");
    }
    let add = list["commands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "task.comment.add")
        .unwrap();
    assert_eq!(add["scope"], "task:subtree");
    let (_, all) = send(&e.app, "/api/v1/commands?all=true", Some(&e.token), None).await;
    let reason = |name: &str| {
        all["unavailable"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .map(|c| c["reason"].clone())
    };
    assert_eq!(reason("question.ask"), Some(json!("grant_missing")));
    assert_eq!(reason("decision.resolve"), Some(json!("not_available")));
    assert_eq!(reason("task.show"), Some(json!("grant_missing")));
    assert_eq!(reason("task.list"), Some(json!("not_available")));
    // No credential.
    let (status, body) = send(&e.app, "/api/v1/commands", None, None).await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::UNAUTHORIZED, json!("unauthorized"))
    );
}

#[tokio::test]
async fn agent_invokes_commands_through_their_routes_once_audited() {
    let e = setup_with(Some(grants())).await;
    let op = Uuid::new_v4();
    let (status, added) = send(
        &e.app,
        "/api/v1/commands/task.comment.add",
        Some(&e.token),
        Some(json!({"params":{"operation_id":op,"body":"Through the command surface."}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{added}");
    assert_eq!(added["comment"]["comment_id"], json!(op));
    assert_eq!(added["comment"]["author"]["kind"], "agent");
    // One audit row for the call, not one per layer.
    assert_eq!(
        audit_rows(&e, "task.comment.add").await,
        vec![("allow".into(), None)]
    );
    let (status, page) = send(
        &e.app,
        "/api/v1/commands/comment.list",
        Some(&e.token),
        Some(json!({"params":{"limit":5}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["items"][0]["comment_id"], json!(op));

    // Refusals are the §6.8 shape and audited.
    let (status, body) = send(
        &e.app,
        "/api/v1/commands/question.ask",
        Some(&e.token),
        Some(json!({"params":{"operation_id":Uuid::new_v4(),"question":"May I?"}})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let refusal: aq_agent_grants::Refusal = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(refusal.code, aq_agent_grants::RefusalCode::GrantMissing);
    assert_eq!(refusal.grant.as_deref(), Some("question.ask"));
    assert_eq!(
        audit_rows(&e, "question.ask").await,
        vec![("deny".into(), Some("grant_missing".into()))]
    );
    let (status, body) = send(
        &e.app,
        "/api/v1/commands/decision.resolve",
        Some(&e.token),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::FORBIDDEN, json!("owner_only")),
        "{body}"
    );
    let (status, body) = send(
        &e.app,
        "/api/v1/commands/task.list",
        Some(&e.token),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::FORBIDDEN, json!("not_available")),
        "{body}"
    );
    let (status, body) = send(
        &e.app,
        "/api/v1/commands/no.such",
        Some(&e.token),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::NOT_FOUND, json!("unknown_command")),
        "{body}"
    );
    let (status, body) = send(
        &e.app,
        "/api/v1/commands/comment.list",
        Some(&e.token),
        Some(json!({"bogus":1})),
    )
    .await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::BAD_REQUEST, json!("invalid_request")),
        "{body}"
    );
    // A foreign project is outside project:self.
    let (status, body) = send(
        &e.app,
        "/api/v1/commands/knowledge.search",
        Some(&e.token),
        Some(json!({"params":{"text":"x","project_id":Uuid::new_v4()}})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
}

#[tokio::test]
async fn owner_commands_reach_the_same_handlers() {
    let e = setup_with(Some(grants())).await;
    let (status, me) = send(&e.admin, "/api/v1/commands/whoami", None, Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["principal"], "owner");
    // A read: the command's response is the REST route's response.
    let (status, direct) = send(&e.admin, "/api/waits", None, None).await;
    assert_eq!(status, StatusCode::OK, "{direct}");
    let (status, via) = send(
        &e.admin,
        "/api/v1/commands/wait.list",
        None,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{via}");
    assert_eq!(via, direct);
    let (status, listed) = send(&e.admin, "/api/v1/commands", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = listed["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"decision.resolve") && names.contains(&"task.show"));
    assert!(
        !names.contains(&"wait.register"),
        "agent-only commands are not the owner's"
    );
    // A mutation: the command builds the same ActionRequest as /api/v1/actions.
    let (status, body) = send(
        &e.admin,
        "/api/v1/commands/task.comment.add",
        None,
        Some(json!({"params":{"operation_id":Uuid::new_v4(),"project_id":e.binding["project_id"],"body":"x"}})),
    )
    .await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::BAD_REQUEST, json!("invalid_request")),
        "subject is required: {body}"
    );
    let owner_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM command_audit WHERE caller_kind='owner'")
            .fetch_one(&e.pool)
            .await
            .unwrap();
    assert!(owner_rows >= 3);
}

/// Spec 0006 phase 2: subtask creation is confined to the attempt's subtree,
/// limited in count and depth, and cannot pick a child's profile.
#[tokio::test]
async fn agent_task_creation_is_scoped_limited_and_cannot_escalate() {
    let e = setup_with(Some(json!([
        {"operation":"task.create.subtask","scope":"task:subtree","limits":{"max_created_tasks":3,"max_depth":2}},
        {"operation":"task.read","scope":"project:self"}
    ])))
    .await;
    let attempt = Uuid::parse_str(e.binding["attempt_id"].as_str().unwrap()).unwrap();
    let own = Uuid::parse_str(e.binding["task_id"].as_str().unwrap()).unwrap();
    let project = Uuid::parse_str(e.binding["project_id"].as_str().unwrap()).unwrap();
    let create = |params: Value| {
        let app = e.app.clone();
        let token = e.token.clone();
        async move {
            send(
                &app,
                "/api/v1/commands/task.create",
                Some(&token),
                Some(json!({"params":params})),
            )
            .await
        }
    };
    let code = |b: &Value| b["code"].as_str().unwrap_or_default().to_string();
    // No parent: a top-level task needs task.create.project, which is not held.
    let (status, body) =
        create(json!({"operation_id":Uuid::new_v4(),"title":"Unrelated top-level task"})).await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::FORBIDDEN, "grant_missing".into()),
        "{body}"
    );
    assert_eq!(body["grant"], "task.create.project");
    assert_eq!(body["held"][0]["operation"], "task.create.subtask");
    // A parent outside the subtree.
    let (status, body) =
        create(json!({"operation_id":Uuid::new_v4(),"title":"x","parent_id":Uuid::new_v4()})).await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::FORBIDDEN, "scope_denied".into()),
        "{body}"
    );
    // A hint that could launch the child under another profile.
    let (status, body) = create(json!({"operation_id":Uuid::new_v4(),"title":"x","parent_id":"self","profile_hint":"admin"})).await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::FORBIDDEN, "scope_denied".into()),
        "{body}"
    );
    // Depth: a grandchild (depth 2) is allowed, a great-grandchild is not.
    let (child, grandchild) = (Uuid::new_v4(), Uuid::new_v4());
    for (task, parent, depth) in [(child, own, 1), (grandchild, child, 2)] {
        sqlx::query("INSERT INTO agent_created_tasks(task_id,attempt_id,project_id,parent_id,root_task_id,depth,operation_id,created_at_ms) VALUES($1,$2,$3,$4,$5,$6,$1,0)")
            .bind(task).bind(attempt).bind(project).bind(parent).bind(own).bind(depth)
            .execute(&e.pool).await.unwrap();
    }
    let (status, body) =
        create(json!({"operation_id":Uuid::new_v4(),"title":"x","parent_id":grandchild})).await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::FORBIDDEN, "quota_exceeded".into()),
        "{body}"
    );
    assert!(body["message"].as_str().unwrap().contains("max_depth"));
    // Within scope and depth: admitted and reserved against the quota (the
    // Graph write itself has no project binding in this fixture).
    let op = Uuid::new_v4();
    let (status, body) =
        create(json!({"operation_id":op,"title":"Verify changelog links","parent_id":child})).await;
    assert!(
        !matches!(
            code(&body).as_str(),
            "grant_missing" | "scope_denied" | "quota_exceeded"
        ),
        "{status} {body}"
    );
    let depth: Option<i32> =
        sqlx::query_scalar("SELECT depth FROM agent_created_tasks WHERE task_id=$1")
            .bind(op)
            .fetch_one(&e.pool)
            .await
            .unwrap();
    assert_eq!(depth, Some(2));
    // Quota: three created by this attempt; a fourth is refused.
    let (status, body) =
        create(json!({"operation_id":Uuid::new_v4(),"title":"x","parent_id":"self"})).await;
    assert_eq!(
        (status, code(&body)),
        (StatusCode::FORBIDDEN, "quota_exceeded".into()),
        "{body}"
    );
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("max_created_tasks")
    );
    let denials: i64 = sqlx::query_scalar("SELECT count(*) FROM command_audit WHERE attempt_id=$1 AND command='task.create' AND decision='deny'")
        .bind(attempt).fetch_one(&e.pool).await.unwrap();
    assert!(denials >= 5, "{denials}");
}

/// A graph standing in for aq-graph: the agent-access qualified fixture.
async fn fake_graph() -> (String, tokio::task::JoinHandle<()>) {
    let data: Value = serde_json::from_str(include_str!(
        "../../../../protocol/agent-access/v1/fixtures/qualified-graph.json"
    ))
    .unwrap();
    let app = axum::Router::new().fallback(move |uri: axum::http::Uri| {
        let data = data.clone();
        async move {
            let path = uri.path();
            let body = if path.ends_with("/context") {
                data["context"].clone()
            } else {
                data["nodes"][path.rsplit('/').next().unwrap()].clone()
            };
            ([("X-AQ-Graph-API-Version", "1")], axum::Json(body))
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (
        url,
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
    )
}
const TASK_3: &str = "00000000-0000-0000-0000-000000000003";
const KNOWLEDGE_2: &str = "00000000-0000-0000-0000-000000000002";

/// The `task.*` reads an agent runs in a sandbox: its own task and the tasks
/// its `task.read` scope reaches, as scoped graph reads of task nodes only.
#[tokio::test]
async fn agent_reads_tasks_in_its_scope_and_only_tasks() {
    let (url, graph) = fake_graph().await;
    let e = setup_graph(
        Some(json!([
            {"operation":"task.read","scope":"project:self"},
            {"operation":"comment.read","scope":"project:self"}
        ])),
        &url,
    )
    .await;
    let run = |name: &'static str, params: Value| {
        let (app, token) = (e.app.clone(), e.token.clone());
        async move {
            send(
                &app,
                &format!("/api/v1/commands/{name}"),
                Some(&token),
                Some(json!({"params":params})),
            )
            .await
        }
    };
    let (status, own) = run("task.show", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{own}");
    assert_eq!(own["result"]["node"]["id"], e.binding["task_id"]);
    assert_eq!(own["result"]["node"]["kind"], "task");
    let (status, other) = run("task.show", json!({"task_id":TASK_3})).await;
    assert_eq!(status, StatusCode::OK, "{other}");
    assert_eq!(other["result"]["node"]["id"], TASK_3);
    // A knowledge record is not a task: the task grant does not read it.
    let (status, body) = run("task.show", json!({"task_id":KNOWLEDGE_2})).await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::NOT_FOUND, json!("not_found")),
        "{body}"
    );
    let (status, children) = run("task.children", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{children}");
    let edges = children["result"]["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1, "{children}");
    assert_eq!(edges[0]["target"], TASK_3);
    // Another project is refused before any read.
    let (status, body) = run("task.show", json!({"project_id":Uuid::new_v4()})).await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::FORBIDDEN, json!("scope_denied")),
        "{body}"
    );
    // Comments of another task in scope.
    let (status, page) = run("comment.list", json!({"task_id":TASK_3})).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["task_id"], TASK_3);
    assert_eq!(
        audit_rows(&e, "task.show").await,
        vec![
            ("allow".into(), None),
            ("allow".into(), None),
            ("allow".into(), None),
            ("deny".into(), Some("scope_denied".into()))
        ]
    );

    // task:self reads only the attempt's own task; task:subtree also the
    // tasks it created below it.
    let e = setup_graph(
        Some(json!([
            {"operation":"task.read","scope":"task:subtree"},
            {"operation":"comment.read","scope":"task:self"}
        ])),
        &url,
    )
    .await;
    let run = |name: &'static str, params: Value| {
        let (app, token) = (e.app.clone(), e.token.clone());
        async move {
            send(
                &app,
                &format!("/api/v1/commands/{name}"),
                Some(&token),
                Some(json!({"params":params})),
            )
            .await
        }
    };
    let (status, body) = run("task.show", json!({"task_id":TASK_3})).await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::FORBIDDEN, json!("scope_denied")),
        "{body}"
    );
    assert_eq!(
        body["held"],
        json!([{"operation":"task.read","scope":"task:subtree"}])
    );
    let id = |k: &str| Uuid::parse_str(e.binding[k].as_str().unwrap()).unwrap();
    sqlx::query("INSERT INTO agent_created_tasks(task_id,attempt_id,project_id,parent_id,root_task_id,depth,operation_id,created_at_ms) VALUES($1,$2,$3,$4,$4,1,$1,0)")
        .bind(Uuid::parse_str(TASK_3).unwrap()).bind(id("attempt_id")).bind(id("project_id")).bind(id("task_id"))
        .execute(&e.pool).await.unwrap();
    let (status, body) = run("task.show", json!({"task_id":TASK_3})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // comment.read at task:self: the own thread, not the child's, on the
    // command surface and on the route itself.
    let (status, _) = run("comment.list", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = run("comment.list", json!({"task_id":TASK_3})).await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::FORBIDDEN, json!("scope_denied")),
        "{body}"
    );
    let (status, body) = send(
        &e.app,
        "/v1/agent/comments/list",
        Some(&e.token),
        Some(json!({"api_version":1,"task_id":TASK_3})),
    )
    .await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::FORBIDDEN, json!("scope_denied")),
        "{body}"
    );
    graph.abort();
}

/// An agent edits and deletes its own comments at their current revision,
/// and no one else's.
#[tokio::test]
async fn agent_edits_and_deletes_only_its_own_comments() {
    let e = setup_with(Some(grants())).await;
    let id = |k: &str| Uuid::parse_str(e.binding[k].as_str().unwrap()).unwrap();
    let run = |name: &'static str, params: Value| {
        let (app, token) = (e.app.clone(), e.token.clone());
        async move {
            send(
                &app,
                &format!("/api/v1/commands/{name}"),
                Some(&token),
                Some(json!({"params":params})),
            )
            .await
        }
    };
    let mine = Uuid::new_v4();
    let (status, _) = run(
        "task.comment.add",
        json!({"operation_id":mine,"body":"First draft."}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let edit = json!({"operation_id":Uuid::new_v4(),"comment_id":mine,"expected_revision":"1","body":"Edited."});
    let (status, body) = run("task.comment.edit", edit.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["revision"].clone(), body["comment"]["body"].clone()),
        (json!("2"), json!("Edited."))
    );
    // Replay returns the recorded effect; a stale revision changes nothing.
    let (status, again) = run("task.comment.edit", edit.clone()).await;
    assert_eq!(
        (status, again["revision"].clone()),
        (StatusCode::OK, json!("2"))
    );
    let mut stale = edit;
    stale["operation_id"] = json!(Uuid::new_v4());
    let (status, body) = run("task.comment.edit", stale).await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::CONFLICT, json!("stale_revision")),
        "{body}"
    );
    // The owner's comment on the same task is not the agent's to change.
    let theirs = Uuid::new_v4();
    let owner: Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(&e.pool)
        .await
        .unwrap();
    aq_task_comments::add(
        &e.pool,
        theirs,
        aq_task_comments::Thread {
            project_id: id("project_id"),
            task_id: id("task_id"),
        },
        &aq_task_comments::Author::Owner { user_id: owner },
        "Owner note.",
        chrono::Utc::now(),
    )
    .await
    .unwrap();
    for target in [theirs, Uuid::new_v4()] {
        let (status, body) = run(
            "task.comment.delete",
            json!({"operation_id":Uuid::new_v4(),"comment_id":target,"expected_revision":"1"}),
        )
        .await;
        assert_eq!(
            (status, body["code"].clone()),
            (StatusCode::FORBIDDEN, json!("scope_denied")),
            "{body}"
        );
        assert_eq!(body["grant"], "comment.write");
    }
    // The route's own author check, without the command surface.
    let (status, body) = send(
        &e.app,
        "/v1/agent/comments/edit",
        Some(&e.token),
        Some(json!({"api_version":1,"operation_id":Uuid::new_v4(),"comment_id":theirs,"expected_revision":"1","body":"Mine now."})),
    )
    .await;
    assert_eq!(
        (status, body["code"].clone()),
        (StatusCode::FORBIDDEN, json!("scope_denied")),
        "{body}"
    );
    let (status, body) = run(
        "task.comment.delete",
        json!({"operation_id":Uuid::new_v4(),"comment_id":mine,"expected_revision":"2"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["comment"]["body"], Value::Null);
    assert_eq!(
        audit_rows(&e, "task.comment.delete").await,
        vec![
            ("deny".into(), Some("scope_denied".into())),
            ("deny".into(), Some("scope_denied".into())),
            ("allow".into(), None)
        ]
    );
}

/// `wait.list` and `wait.show` show an agent its own attempt's waits only.
#[tokio::test]
async fn agent_lists_and_shows_only_its_own_waits() {
    let e = setup_with(Some(
        json!([{"operation":"wait.read","scope":"attempt:self"}]),
    ))
    .await;
    let id = |k: &str| Uuid::parse_str(e.binding[k].as_str().unwrap()).unwrap();
    let (mine, theirs) = (Uuid::new_v4(), Uuid::new_v4());
    for (wait, attempt) in [(mine, id("attempt_id")), (theirs, Uuid::new_v4())] {
        sqlx::query("INSERT INTO agent_waits(wait_id,caller_key,operation_id,project_id,workspace_id,task_id,attempt_id,node_id,wait_generation,continuation_generation,predicate_digest,request_digest,phase,revision,next_due_ms,deadline_ms,created_at_ms,snapshot,response) VALUES($1,'k',$1,$2,$3,$4,$5,$6,1,1,'p','r','terminal',1,0,0,0,decode('00','hex'),'{}')")
            .bind(wait).bind(id("project_id")).bind(id("workspace_id")).bind(id("task_id")).bind(attempt).bind(id("node_id"))
            .execute(&e.pool).await.unwrap();
    }
    let (status, list) = send(
        &e.app,
        "/api/v1/commands/wait.list",
        Some(&e.token),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let ids: Vec<&Value> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| &i["wait_id"])
        .collect();
    assert_eq!(ids, vec![&json!(mine)]);
    for other in [theirs, Uuid::new_v4()] {
        let (status, body) = send(
            &e.app,
            "/api/v1/commands/wait.show",
            Some(&e.token),
            Some(json!({"params":{"wait_id":other}})),
        )
        .await;
        assert_eq!(
            (status, body["code"].clone()),
            (StatusCode::NOT_FOUND, json!("not_found")),
            "{body}"
        );
    }
    assert_eq!(
        audit_rows(&e, "wait.show").await,
        vec![("allow".into(), None), ("allow".into(), None)]
    );
}
