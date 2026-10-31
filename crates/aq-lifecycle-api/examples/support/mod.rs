use aq_lifecycle_api::*;
use uuid::Uuid;
pub fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}
pub fn artifact_ref(s: &str) -> ArtifactRef {
    ArtifactRef {
        id: Text::new(s).unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(s.as_bytes()),
    }
}
pub fn registration() -> WaitRegistration {
    let binding:Binding=serde_json::from_value(serde_json::json!({"project_id":id(1),"task_id":id(2),"lineage_id":id(3),"attempt_id":id(4),"attempt_generation":1,"session":{"kind":"remote","node_id":id(5),"boot_id":id(6),"session_id":id(7),"attempt_id":id(4),"assignment_generation":1}})).unwrap();
    let predicate = Predicate::Any {
        members: vec![
            Member {
                member_id: id(20),
                condition: Condition::Timer { due_at_ms: 100 },
            },
            Member {
                member_id: id(21),
                condition: Condition::Task {
                    task_id: id(22),
                    revision: Text::new("1").unwrap(),
                    requirement: TaskRequirement::Settled,
                },
            },
        ],
    };
    WaitRegistration {
        scope: Scope {
            project_id: id(1),
            workspace_id: id(8),
        },
        binding,
        wait_id: id(9),
        wait_generation: 1,
        continuation_generation: 1,
        policy: artifact_ref("wait-policy"),
        predicate_digest: canonical_digest(&predicate).unwrap(),
        predicate,
        preference: WaitPreference::Economical,
    }
}
pub fn peer(r: &WaitRegistration) -> ConformanceApi {
    ConformanceApi::new(
        CallerContext {
            role: ActorRole::SessionAgent,
            scope: r.scope.clone(),
            binding: Some(r.binding.clone()),
            current_operation_id: None,
            current_run_id: None,
        },
        vec![Capability {
            id: Text::new(WAIT_CAPABILITY).unwrap(),
            version: 1,
        }],
    )
    .unwrap()
}
pub fn fixtures() -> serde_json::Value {
    let r = registration();
    let request = Request::seal(id(30), Command::RegisterWait(r.clone())).unwrap();
    let response = peer(&r).receive(&request, 10).unwrap();
    response.validate_against(&request).unwrap();
    serde_json::json!({"register_any_wait":request,"registered_policy_pending":response})
}
