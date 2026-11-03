//! F4 automatic notification policy against isolated PostgreSQL. Fake transport
//! only: these tests never dispatch HTTP.
use aq_lifecycle_api::Scope;
use aq_notifications::{RetryPolicy, Route};
use aq_server::{
    notifications::{
        config::Loaded,
        persistence::{RouteRequest, Service},
        policy::{Event, Kind, Policy, Transition},
    },
    test_support::TestDb,
};
use chrono::{TimeZone, Utc};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
use uuid::Uuid;

struct Env {
    _db: TestDb,
    policy: Policy,
    scope: Scope,
    time: Arc<AtomicI64>,
}
impl Env {
    async fn new(routed: bool) -> Self {
        let db = TestDb::create().await;
        let store = db.store().await;
        let pool = db.pool().await;
        let owner = store
            .bootstrap_owner(
                "policy-owner@localhost",
                "dev",
                "policy-owner@localhost",
                Utc.timestamp_millis_opt(1000).unwrap(),
            )
            .await
            .unwrap()
            .unwrap()
            .id;
        let scope = Scope {
            project_id: Uuid::new_v4(),
            workspace_id: Uuid::new_v4(),
        };
        sqlx::query("INSERT INTO project_execution_bindings(project_id,revision,binding,validated_at_ms,binding_observation) VALUES($1,1,$2,1,'{}')")
            .bind(scope.project_id).bind(json!({"enabled":true,"graph_workspace_id":scope.workspace_id})).execute(&pool).await.unwrap();
        // Window-aligned start so a burst stays inside one flood window.
        let time = Arc::new(AtomicI64::new(600_000));
        let t = time.clone();
        let service = Service {
            pool,
            configuration: routed.then(|| {
                Arc::new(Loaded::fixture(
                    1,
                    "discord-test",
                    1,
                    "http://127.0.0.1:1/api/v10/webhooks/42/fixture_token_1234567890",
                ))
            }),
            public_origin: "https://aq.example".into(),
            clock: Arc::new(move || Utc.timestamp_millis_opt(t.load(Ordering::SeqCst)).unwrap()),
        };
        service.install(Uuid::new_v4()).await.unwrap();
        if routed {
            service
                .set_route(
                    owner,
                    RouteRequest {
                        operation_id: Uuid::new_v4(),
                        expected_revision: 0,
                        route: Route {
                            route_id: Uuid::new_v4(),
                            revision: 1,
                            scope: scope.clone(),
                            recipient_user_id: owner,
                            endpoint_key: "discord-test".into(),
                            endpoint_revision: 1,
                            enabled: true,
                            retry: RetryPolicy {
                                max_attempts: 3,
                                max_elapsed_ms: 30000,
                                initial_delay_ms: 250,
                                max_delay_ms: 2000,
                                allow_duplicate_risk: false,
                            },
                        },
                    },
                )
                .await
                .unwrap();
        }
        Self {
            _db: db,
            policy: Policy::new(service),
            scope,
            time,
        }
    }
    fn event(&self, kind: Kind, subject: &str, transition: Transition) -> Event {
        Event {
            source_event_id: Uuid::new_v4(),
            kind,
            subject: subject.into(),
            scope: self.scope.clone(),
            transition,
            title: format!("{subject} needs attention"),
        }
    }
    async fn messages(&self) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT definition->>'summary' FROM notification_messages ORDER BY sequence",
        )
        .fetch_all(&self.policy.service().pool)
        .await
        .unwrap()
    }
    async fn operations(&self) -> i64 {
        sqlx::query_scalar("SELECT operation_count FROM notification_budget WHERE singleton")
            .fetch_one(&self.policy.service().pool)
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn credential_invalid_notifies_once_per_episode() {
    let e = Env::new(true).await;
    let opened = e.event(Kind::CredentialInvalid, "profile-a", Transition::Opened);
    let first = e.policy.observe(opened.clone()).await.unwrap();
    assert_eq!(first["outcome"], "notified");
    assert_eq!(first["external_state"], "routed");
    assert_eq!(first["generation"], 1);
    assert_eq!(e.messages().await.len(), 1);
    let operations = e.operations().await;

    // A replayed source event creates no second intent and no new operation.
    assert_eq!(e.policy.observe(opened.clone()).await.unwrap(), first);
    assert_eq!(e.messages().await.len(), 1);
    assert_eq!(e.operations().await, operations);

    // The same episode reported again by a new source event stays silent.
    let again = e
        .policy
        .observe(e.event(Kind::CredentialInvalid, "profile-a", Transition::Opened))
        .await
        .unwrap();
    assert_eq!(again["outcome"], "already_open");
    assert_eq!(e.messages().await.len(), 1);

    // An independent subject is its own episode.
    let other = e
        .policy
        .observe(e.event(Kind::CredentialInvalid, "profile-b", Transition::Opened))
        .await
        .unwrap();
    assert_eq!(other["outcome"], "notified");
    assert_eq!(e.messages().await.len(), 2);

    // Resolve, then reopen: exactly one more notification, with a new identity.
    let resolved = e
        .policy
        .observe(e.event(Kind::CredentialInvalid, "profile-a", Transition::Resolved))
        .await
        .unwrap();
    assert_eq!(resolved["outcome"], "resolved");
    let quiet = e
        .policy
        .observe(e.event(Kind::CredentialInvalid, "profile-a", Transition::Resolved))
        .await
        .unwrap();
    assert_eq!(quiet["outcome"], "not_open");
    e.time.fetch_add(120_000, Ordering::SeqCst);
    let reopened = e
        .policy
        .observe(e.event(Kind::CredentialInvalid, "profile-a", Transition::Opened))
        .await
        .unwrap();
    assert_eq!(reopened["outcome"], "notified");
    assert_eq!(reopened["generation"], 2);
    assert_ne!(reopened["message_id"], first["message_id"]);
    let messages = e.messages().await;
    assert_eq!(messages.len(), 3);
    assert!(
        messages[0].contains("Credential invalid"),
        "{}",
        messages[0]
    );
    assert!(messages[0].contains("profile-a"));
    assert!(messages.iter().all(|m| m.len() <= 600));
}

#[tokio::test]
async fn flood_of_failures_collapses_to_bounded_notifications() {
    let e = Env::new(true).await;
    for n in 0..50 {
        let outcome = e
            .policy
            .observe(e.event(
                Kind::TaskFailedOrBlocked,
                &format!("task-{n}"),
                Transition::Opened,
            ))
            .await
            .unwrap();
        let expected = if n < 3 { "notified" } else { "collapsed" };
        assert_eq!(outcome["outcome"], expected, "event {n}");
    }
    assert_eq!(e.messages().await.len(), 3);
    // The window is still open: nothing to summarize yet.
    assert_eq!(e.policy.flush().await.unwrap(), 0);
    assert_eq!(e.messages().await.len(), 3);
    e.time.fetch_add(60_000, Ordering::SeqCst);
    assert_eq!(e.policy.flush().await.unwrap(), 1);
    assert_eq!(e.policy.flush().await.unwrap(), 0);
    let messages = e.messages().await;
    assert_eq!(messages.len(), 4, "{messages:?}");
    assert!(messages[3].contains("47 more"), "{}", messages[3]);
    // Collapsed episodes are still open: a repeat does not notify.
    let repeat = e
        .policy
        .observe(e.event(Kind::TaskFailedOrBlocked, "task-49", Transition::Opened))
        .await
        .unwrap();
    assert_eq!(repeat["outcome"], "already_open");
    assert_eq!(e.messages().await.len(), 4);
}

#[tokio::test]
async fn no_external_route_is_reported_not_claimed() {
    let e = Env::new(false).await;
    let outcome = e
        .policy
        .observe(e.event(Kind::QuestionAsked, "question-1", Transition::Opened))
        .await
        .unwrap();
    assert_eq!(outcome["outcome"], "notified");
    assert_eq!(outcome["external_state"], "no_external_route");
    let deliveries: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_deliveries")
        .fetch_one(&e.policy.service().pool)
        .await
        .unwrap();
    assert_eq!(deliveries, 0);
}

#[test]
fn every_kind_has_a_stable_wire_name_and_label() {
    let names: Vec<_> = Kind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(
        names,
        [
            "task.failed_or_blocked",
            "decision.needed",
            "question.asked",
            "credential.invalid",
            "milestone.gate_ready",
            "worker.offline"
        ]
    );
    assert_eq!(
        Kind::from_failure_reason("credential_invalid"),
        Some(Kind::CredentialInvalid)
    );
    assert_eq!(Kind::from_failure_reason("timeout"), None);
}
