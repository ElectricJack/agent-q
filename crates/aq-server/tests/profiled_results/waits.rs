//! H4 durable waits: registration replay and a HumanDecision wake that is
//! recorded once and consumed once.
use super::*;
use aq_server::agent_access::waits;
use aq_wait_engine::wire;

struct WaitEnv {
    env: ReviewEnv,
    decision_id: Uuid,
    scope: wire::Scope,
    binding: wire::Binding,
}
impl WaitEnv {
    fn node(&self) -> Uuid {
        self.env.f.node.node_id
    }
    fn attempt(&self) -> Uuid {
        self.env.report.binding.attempt_id
    }
    fn registration(&self, wait_id: Uuid, condition: wire::Condition) -> wire::WaitRegistration {
        let predicate = wire::Predicate::Single {
            member: wire::Member {
                member_id: Uuid::new_v4(),
                condition,
            },
        };
        wire::WaitRegistration {
            scope: self.scope.clone(),
            binding: self.binding.clone(),
            wait_id,
            wait_generation: 1,
            continuation_generation: 1,
            policy: waits::default_policy().reference,
            predicate_digest: wire::canonical_digest(&predicate).unwrap(),
            predicate,
            preference: wire::WaitPreference::Economical,
        }
    }
    async fn condition(&self) -> wire::Condition {
        let row = sqlx::query("SELECT d.subject_digest,d.request_digest,s.report_id FROM human_decision_requests d JOIN review_subjects s USING(subject_digest) WHERE d.decision_id=$1")
            .bind(self.decision_id)
            .fetch_one(&self.env.f.db.pool().await)
            .await
            .unwrap();
        let subject: String = sqlx::Row::get(&row, "subject_digest");
        let asked: String = sqlx::Row::get(&row, "request_digest");
        wire::Condition::HumanDecision {
            decision_id: self.decision_id,
            subject_id: sqlx::Row::get(&row, "report_id"),
            subject_revision: wire::Text::new(subject).unwrap(),
            question_digest: serde_json::from_value(json!(asked)).unwrap(),
        }
    }
    async fn register(&self, request: &wire::Request) -> wire::Response {
        waits::register(&self.env.f.state, self.node(), self.attempt(), request)
            .await
            .unwrap()
    }
}

async fn wait_env() -> WaitEnv {
    let env = review_env(1).await;
    let f = &env.f;
    let request = aq_review_protocol::DecisionRequest {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        decision_id: Uuid::new_v4(),
        subject: env.prepared.subject().artifact().unwrap(),
        eligible_owners: vec![f.actor],
        eligibility_project_revision: 1,
        review_policy: env.prepared.subject().validation_policy.clone(),
        expires_at_ms: NOW + 3_600_000,
    };
    env.service
        .admit(f.actor, &env.prepared, &request)
        .await
        .unwrap();
    let (scope, binding) =
        waits::caller_identity(&f.state, f.node.node_id, env.report.binding.attempt_id)
            .await
            .unwrap();
    WaitEnv {
        decision_id: request.decision_id,
        env,
        scope,
        binding,
    }
}
fn refusal(response: &wire::Response) -> Option<wire::RefusalCode> {
    match &response.body {
        wire::ResponseBody::Refused { code, .. } => Some(code.clone()),
        _ => None,
    }
}
fn registered(response: &wire::Response) -> wire::RegisteredWait {
    match &response.body {
        wire::ResponseBody::WaitRegistered(wait) => (**wait).clone(),
        other => panic!("expected registration, got {other:?}"),
    }
}

#[tokio::test]
async fn wait_registration_replay_is_idempotent() {
    let w = Box::pin(wait_env()).await;
    let f = &w.env.f;
    let condition = w.condition().await;
    let wait_id = Uuid::new_v4();
    let operation = Uuid::new_v4();
    let request = wire::Request::seal(
        operation,
        wire::Command::RegisterWait(w.registration(wait_id, condition.clone())),
    )
    .unwrap();
    let first = w.register(&request).await;
    let original = registered(&first);
    // A replay after time passes returns the original record: same age, same
    // deadline, same budgets.
    f.time.store((NOW + 5_000) as i64, Ordering::SeqCst);
    let replay = w.register(&request).await;
    assert_eq!(json!(replay), json!(first));
    assert_eq!(registered(&replay).original_created_at_ms, NOW);
    assert_eq!(original.original_created_at_ms, NOW);
    // Same operation, changed bytes: conflict, never a second wait.
    let changed = wire::Request::seal(
        operation,
        wire::Command::RegisterWait(w.registration(Uuid::new_v4(), condition.clone())),
    )
    .unwrap();
    assert_eq!(
        refusal(&w.register(&changed).await),
        Some(wire::RefusalCode::ChangedPayload)
    );
    // A second active wait for the same attempt is refused.
    let second = wire::Request::seal(
        Uuid::new_v4(),
        wire::Command::RegisterWait(w.registration(Uuid::new_v4(), condition)),
    )
    .unwrap();
    assert_eq!(
        refusal(&w.register(&second).await),
        Some(wire::RefusalCode::InvalidRequest)
    );
    let pool = f.db.pool().await;
    let (rows, deadline): (i64, i64) =
        sqlx::query_as("SELECT count(*),max(deadline_ms) FROM agent_waits")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rows, 1);
    assert_eq!(deadline as u64, NOW + aq_wait_engine::MAX_AGE_MS);
}

#[tokio::test]
async fn wait_on_decision_wakes_once_after_resolution() {
    let w = Box::pin(wait_env()).await;
    let f = &w.env.f;
    let pool = f.db.pool().await;
    let wait_id = Uuid::new_v4();
    let request = wire::Request::seal(
        Uuid::new_v4(),
        wire::Command::RegisterWait(w.registration(wait_id, w.condition().await)),
    )
    .unwrap();
    let wait = registered(&w.register(&request).await);
    let phase: String = sqlx::query_scalar("SELECT phase FROM agent_waits WHERE wait_id=$1")
        .bind(wait_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(phase, "pending");

    Box::pin(w.env.service.resolve(
        f.actor,
        &aq_review_protocol::ResolveRequest {
            schema_version: 1,
            operation_id: Uuid::new_v4(),
            decision_id: w.decision_id,
            expected_decision_revision: 1,
            choice: aq_review_protocol::Choice::Approve,
            comment: Some("Synthetic".into()),
        },
    ))
    .await
    .unwrap();
    f.time.store((NOW + 61_000) as i64, Ordering::SeqCst);
    // Duplicate reconciliation records exactly one outcome and one intent.
    waits::reconcile_due(&f.state, 64).await.unwrap();
    f.time.store((NOW + 125_000) as i64, Ordering::SeqCst);
    waits::reconcile_due(&f.state, 64).await.unwrap();
    let (outcomes, kind): (i64, Option<String>) =
        sqlx::query_as("SELECT count(*),max(kind) FROM agent_wait_outcomes WHERE wait_id=$1")
            .bind(wait_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((outcomes, kind.as_deref()), (1, Some("satisfied")));
    let intents: Vec<String> =
        sqlx::query_scalar("SELECT state FROM agent_wait_resume_intents WHERE wait_id=$1")
            .bind(wait_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(intents, ["ready"]);

    let inspect = wire::Request::seal(
        Uuid::new_v4(),
        wire::Command::Inspect(wire::InspectRequest {
            scope: w.scope.clone(),
            target: wire::InspectTarget::Wait {
                wait: wait.wait.clone(),
            },
            page: wire::PageRequest {
                limit: 1,
                cursor: None,
            },
        }),
    )
    .unwrap();
    let page = waits::inspect(&f.state, w.node(), w.attempt(), &inspect)
        .await
        .unwrap();
    let wire::ResponseBody::Inspection(page) = page.body else {
        panic!("expected inspection page");
    };
    let [wire::InspectionItem::Wait(view)] = page.items.as_slice() else {
        panic!("expected one wait view");
    };
    let outcome = view.outcome.clone().expect("recorded outcome");
    assert_eq!(outcome.kind, wire::WaitOutcomeKind::Satisfied);

    let ack = |admission_id: Uuid, generation: u64| {
        let mut continuation = wait.continuation.clone();
        continuation.generation = generation;
        wire::Request::seal(
            Uuid::new_v4(),
            wire::Command::ContinuationAck(wire::AckRequest {
                admission_id,
                continuation,
                successor: w.binding.clone(),
                attachment_digest: wire::Digest::of(b"synthetic attachment"),
                outcome: outcome.outcome.clone(),
            }),
        )
        .unwrap()
    };
    // A stale continuation generation cannot consume the intent.
    let stale = waits::ack(&f.state, w.node(), w.attempt(), &ack(Uuid::new_v4(), 2))
        .await
        .unwrap();
    assert_eq!(refusal(&stale), Some(wire::RefusalCode::StaleGeneration));
    let admission = Uuid::new_v4();
    let request = ack(admission, 1);
    let first = waits::ack(&f.state, w.node(), w.attempt(), &request)
        .await
        .unwrap();
    let duplicate = waits::ack(&f.state, w.node(), w.attempt(), &request)
        .await
        .unwrap();
    let wire::ResponseBody::ContinuationAcknowledged {
        acknowledgment_receipt,
        ..
    } = &first.body
    else {
        panic!("expected acknowledgment, got {:?}", first.body);
    };
    assert_eq!(json!(duplicate), json!(first));
    assert!(
        acknowledgment_receipt
            .id
            .as_str()
            .contains(&admission.to_string())
    );
    let intents: Vec<String> =
        sqlx::query_scalar("SELECT state FROM agent_wait_resume_intents WHERE wait_id=$1")
            .bind(wait_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(intents, ["consumed"]);
}
