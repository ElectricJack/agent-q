use super::*;
use std::collections::BTreeMap;

fn actions() -> Vec<ActionSpec> {
    vec![
        ActionSpec {
            kind: "task.create",
            grant: "task.create.subtask",
            target: TargetRule::ParentTask {
                field: "parent_id",
                unparented: "task.create.project",
            },
        },
        ActionSpec {
            kind: "task.graph.create",
            grant: "task.create.subtask",
            target: TargetRule::ParentTask {
                field: "parent_id",
                unparented: "task.create.project",
            },
        },
        ActionSpec {
            kind: "task.comment.add",
            grant: "comment.write",
            target: TargetRule::Subject,
        },
        ActionSpec {
            kind: "decision.resolve",
            grant: "decision.resolve",
            target: TargetRule::Subject,
        },
    ]
}

#[test]
fn registry_merges_actions_and_refuses_disagreement() {
    let r = Registry::new(&actions()).unwrap();
    let add = r.get("task.comment.add").unwrap();
    assert_eq!(add.owner, Some(Route::action()));
    assert_eq!(add.agent.unwrap().path, "/v1/agent/comments");
    let resolve = r.get("decision.resolve").unwrap();
    assert!(resolve.owner_only() && resolve.agent.is_none());
    assert!(!add.owner_only());
    assert_eq!(add.cli(), ["task", "comment", "add"]);
    let mut wrong = actions();
    wrong[2].grant = "comment.read";
    assert!(Registry::new(&wrong).is_err());
    // A catalog command that names the action union must be a registered action.
    assert!(Registry::new(&actions()[2..]).is_err());
}

#[test]
fn forward_builds_each_body_kind() {
    let r = Registry::new(&actions()).unwrap();
    let binding = json!({"attempt_id":"a"});
    let rid = Uuid::from_u128(1);
    let req = |params: Value| CommandRequest {
        params: params.as_object().unwrap().clone(),
        ..Default::default()
    };
    let show = r.get("task.show").unwrap();
    let p = Uuid::from_u128(2);
    let f = forward(
        show,
        show.owner.as_ref().unwrap(),
        &req(json!({"project_id":p,"task_id":"t-1","revision":3})),
        None,
        rid,
    )
    .unwrap();
    assert_eq!(
        f.uri,
        format!("/api/human/projects/{p}/tasks/t-1?revision=3")
    );
    assert!(f.body.is_none());
    let bad = forward(
        show,
        show.owner.as_ref().unwrap(),
        &req(json!({"project_id":"../x","task_id":"t"})),
        None,
        rid,
    );
    assert!(bad.is_err(), "path params are plain identifiers");
    let ask = r.get("question.ask").unwrap();
    let f = forward(
        ask,
        ask.agent.as_ref().unwrap(),
        &req(json!({"operation_id":"o","question":"q?"})),
        None,
        rid,
    )
    .unwrap();
    assert_eq!(
        f.body.unwrap(),
        json!({"api_version":1,"operation_id":"o","question":"q?"})
    );
    let search = r.get("knowledge.search").unwrap();
    let f = forward(
        search,
        search.agent.as_ref().unwrap(),
        &req(json!({"text":"release"})),
        Some(&binding),
        rid,
    )
    .unwrap();
    assert_eq!(
        f.body.unwrap(),
        json!({"api_version":1,"request_id":rid,"binding":binding,"query":{"kind":"search","text":"release","kind_filter":null,"limit":20,"cursor":null}})
    );
    let create = r.get("task.create").unwrap();
    let op = Uuid::from_u128(3);
    let mut request = req(json!({"title":"x"}));
    request.operation_id = Some(op);
    request.subject = Some(json!({"type":"project","id":"p","revision":"4"}));
    let f = forward(create, create.owner.as_ref().unwrap(), &request, None, rid).unwrap();
    assert_eq!((f.method, f.uri.as_str()), ("POST", "/api/v1/actions"));
    let body = f.body.unwrap();
    assert_eq!(
        (body["kind"].clone(), body["operation_id"].clone()),
        (json!("task.create"), json!(op))
    );
    assert_eq!(body["payload"], json!({"title":"x"}));
    request.subject = None;
    assert!(forward(create, create.owner.as_ref().unwrap(), &request, None, rid).is_err());
}

#[test]
fn document_lists_every_command_with_digests() {
    let r = Registry::new(&actions()).unwrap();
    let schemas: BTreeMap<String, Value> = actions()
        .iter()
        .map(|a| {
            (
                a.kind.to_string(),
                json!({"type":"object","additionalProperties":false}),
            )
        })
        .collect();
    let doc = document(&r, &schemas);
    let commands = doc["commands"].as_array().unwrap();
    assert_eq!(commands.len(), r.commands().len());
    let whoami = commands.iter().find(|c| c["name"] == "whoami").unwrap();
    assert_eq!(whoami["grant"], Value::Null);
    assert!(whoami["principals"]["agent"].is_object());
    for c in commands {
        for (_, p) in c["principals"].as_object().unwrap() {
            assert!(p["params_digest"].as_str().unwrap().starts_with("sha256:"));
        }
    }
}

#[test]
fn reviewed_routes_parse_and_are_not_also_commands() {
    let r = Registry::new(&actions()).unwrap();
    let reviewed = routes::reviewed();
    assert!(!reviewed.is_empty());
    let mut seen = std::collections::BTreeSet::new();
    for route in &reviewed {
        assert!(seen.insert(route.path.clone()), "duplicate {}", route.path);
        assert!(
            !route.note.trim().is_empty(),
            "{} needs a review note",
            route.path
        );
        if route.class != routes::Class::Command {
            let served = r.commands().iter().any(|c| {
                [&c.owner, &c.supervisor, &c.agent]
                    .into_iter()
                    .flatten()
                    .any(|x| x.path == route.path && x.body != Body::Local)
            });
            assert!(
                !served,
                "{} is a command route; drop it from routes.json",
                route.path
            );
        }
    }
}
