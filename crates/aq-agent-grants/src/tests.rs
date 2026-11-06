use super::*;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

#[test]
fn scopes_round_trip_and_refuse_aliases() {
    for s in ["attempt:self", "task:self", "task:subtree", "project:self"] {
        let scope: Scope = s.parse().unwrap();
        assert_eq!(scope.to_string(), s);
    }
    let p = id(7);
    let scope: Scope = format!("project:{p}").parse().unwrap();
    assert_eq!(scope, Scope::Project(p));
    for bad in [
        "project:",
        "project:example",
        "project:00000000-0000-0000-0000-000000000000",
        "project:00000000-0000-0000-0000-00000000000A",
        "task:other",
        "*",
    ] {
        assert!(bad.parse::<Scope>().is_err(), "{bad}");
    }
    let g: Grant =
        serde_json::from_str(r#"{"operation":"task.read","scope":"project:self"}"#).unwrap();
    assert_eq!(g, Grant::new(op::TASK_READ, Scope::ProjectSelf));
    assert!(
        serde_json::from_str::<Grant>(r#"{"operation":"task.read","scope":"x","y":1}"#).is_err()
    );
}

#[test]
fn grant_sets_refuse_owner_only_unknown_and_duplicates() {
    assert!(GrantSet::new(vec![Grant::new("question.answer", Scope::ProjectSelf)]).is_err());
    assert!(GrantSet::new(vec![Grant::new("task.cancel", Scope::ProjectSelf)]).is_err());
    assert!(
        GrantSet::new(vec![
            Grant::new(op::TASK_READ, Scope::ProjectSelf),
            Grant::new(op::TASK_READ, Scope::TaskSelf)
        ])
        .is_err()
    );
    let a = GrantSet::new(vec![
        Grant::new(op::TASK_READ, Scope::ProjectSelf),
        Grant::new(op::COMMENT_READ, Scope::ProjectSelf),
    ])
    .unwrap();
    let b = GrantSet::new(vec![
        Grant::new(op::COMMENT_READ, Scope::ProjectSelf),
        Grant::new(op::TASK_READ, Scope::ProjectSelf),
    ])
    .unwrap();
    assert_eq!(a.digest(), b.digest());
}

#[test]
fn scope_check_pins_project_and_subtree() {
    let (own, other) = (id(1), id(2));
    let h = Holder { project: own };
    let task = |relation| Target::Task {
        project: own,
        relation,
    };
    assert!(allows(Scope::AttemptSelf, h, Target::OwnAttempt));
    assert!(!allows(Scope::AttemptSelf, h, task(Relation::Own)));
    assert!(allows(Scope::TaskSelf, h, task(Relation::Own)));
    assert!(!allows(Scope::TaskSelf, h, task(Relation::Descendant(1))));
    assert!(allows(Scope::TaskSubtree, h, task(Relation::Descendant(2))));
    assert!(!allows(Scope::TaskSubtree, h, task(Relation::Other)));
    assert!(!allows(Scope::TaskSubtree, h, Target::Project(own)));
    assert!(allows(Scope::ProjectSelf, h, task(Relation::Other)));
    assert!(allows(Scope::ProjectSelf, h, Target::Project(own)));
    assert!(!allows(Scope::ProjectSelf, h, Target::Project(other)));
    let foreign = Target::Task {
        project: other,
        relation: Relation::Other,
    };
    assert!(!allows(Scope::ProjectSelf, h, foreign));
    assert!(allows(Scope::Project(other), h, foreign));
    assert!(!allows(Scope::Project(other), h, task(Relation::Own)));
}

#[test]
fn check_names_the_missing_grant_and_held_family() {
    let project = id(1);
    let h = Holder { project };
    let grants = GrantSet::new(vec![
        Grant::new(op::TASK_CREATE_SUBTASK, Scope::TaskSubtree),
        Grant::new(op::TASK_READ, Scope::ProjectSelf),
        Grant::new(op::COMMENT_READ, Scope::ProjectSelf),
    ])
    .unwrap();
    let Decision::Deny(r) = check(
        &grants,
        h,
        "task.create",
        op::TASK_CREATE_PROJECT,
        Target::Project(project),
    ) else {
        panic!()
    };
    assert_eq!(r.code, RefusalCode::GrantMissing);
    assert_eq!(r.grant.as_deref(), Some("task.create.project"));
    assert_eq!(r.held.len(), 2, "only the task.* family is shown");
    assert!(r.message.contains("task.create.project"));
    assert_eq!(r.body()["code"], "grant_missing");
    assert_eq!(r.body()["held"][0]["scope"], "task:subtree");
    let Decision::Deny(r) = check(
        &grants,
        h,
        "task.create",
        op::TASK_CREATE_SUBTASK,
        Target::Task {
            project,
            relation: Relation::Other,
        },
    ) else {
        panic!()
    };
    assert_eq!(r.code, RefusalCode::ScopeDenied);
    let Decision::Deny(r) = check(
        &grants,
        h,
        "question.answer",
        "question.answer",
        Target::OwnAttempt,
    ) else {
        panic!()
    };
    assert_eq!(
        (r.code, r.code.legacy()),
        (RefusalCode::OwnerOnly, "unsupported_capability")
    );
    assert!(matches!(
        check(
            &grants,
            h,
            "task.show",
            op::TASK_READ,
            Target::Project(project)
        ),
        Decision::Allow(_)
    ));
}

#[test]
fn intersection_never_widens() {
    let creator = GrantSet::new(vec![
        Grant::new(op::TASK_CREATE_SUBTASK, Scope::TaskSubtree).limited(Limits {
            max_created_tasks: Some(5),
            max_depth: Some(2),
        }),
        Grant::new(op::COMMENT_WRITE, Scope::TaskSelf),
    ])
    .unwrap();
    let child = GrantSet::new(vec![
        Grant::new(op::TASK_CREATE_SUBTASK, Scope::ProjectSelf).limited(Limits {
            max_created_tasks: Some(9),
            max_depth: None,
        }),
        Grant::new(op::COMMENT_WRITE, Scope::ProjectSelf),
        Grant::new(op::TASK_CREATE_PROJECT, Scope::ProjectSelf),
    ])
    .unwrap();
    let got = child.intersect(&creator);
    assert!(got.get(op::TASK_CREATE_PROJECT).is_none());
    let sub = got.get(op::TASK_CREATE_SUBTASK).unwrap();
    assert_eq!(sub.scope, Scope::TaskSubtree);
    assert_eq!(sub.limits.max_created_tasks, Some(5));
    assert_eq!(sub.limits.max_depth, Some(2));
    assert_eq!(got.get(op::COMMENT_WRITE).unwrap().scope, Scope::TaskSelf);
    let other = GrantSet::new(vec![Grant::new(op::TASK_READ, Scope::Project(id(9)))]).unwrap();
    let mine = GrantSet::new(vec![Grant::new(op::TASK_READ, Scope::ProjectSelf)]).unwrap();
    assert!(other.intersect(&mine).grants().is_empty());
}

#[test]
fn every_route_grant_is_grantable() {
    let h = Holder { project: id(1) };
    for (path, command, grant) in AGENT_ROUTE_GRANTS {
        assert!(path.starts_with("/v1/agent/") && grantable(grant), "{path}");
        assert_eq!(route_grant(path, h).unwrap().0, command);
    }
    assert!(matches!(
        route_grant("/v1/agent/comments", h).unwrap().2,
        Target::Task { .. }
    ));
    assert_eq!(
        route_grant("/v1/agent/waits/ack", h).unwrap().2,
        Target::OwnAttempt
    );
    assert!(route_grant("/v1/agent/messages/send", h).is_none());
    for o in OWNER_ONLY {
        assert!(!grantable(o));
    }
}

#[test]
fn vocabulary_matches_published_contract() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../protocol/commands/v1/schema.json")).unwrap();
    let ops: Vec<&str> = schema["$defs"]["Operation"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(ops, op::ALL);
    let codes = schema["$defs"]["RefusalCode"]["enum"].as_array().unwrap();
    for code in CODES {
        assert!(codes.contains(&serde_json::json!(code.as_str())));
        assert_eq!(serde_json::to_value(code).unwrap(), code.as_str());
    }
    assert_eq!(codes.len(), CODES.len());
}

#[test]
fn published_fixtures_are_what_the_crate_produces() {
    let fixture: Refusal = serde_json::from_str(include_str!(
        "../../../protocol/commands/v1/fixtures/refusal-grant-missing.json"
    ))
    .unwrap();
    let held = fixture.held.clone();
    let made = Refusal::grant_missing("task.create", op::TASK_CREATE_PROJECT, held)
        .with_request(fixture.request_id.unwrap());
    assert_eq!(made, fixture);
    let fixture: Refusal = serde_json::from_str(include_str!(
        "../../../protocol/commands/v1/fixtures/refusal-scope-denied.json"
    ))
    .unwrap();
    let made = Refusal::scope_denied("comment.list", op::COMMENT_READ, fixture.held.clone())
        .with_request(fixture.request_id.unwrap());
    assert_eq!(made, fixture);
}
