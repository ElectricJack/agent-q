use aq_server::{
    execution::{AuthorityStatus, Error},
    store::{NewNode, Store},
    test_support::TestDb,
    tokens::{new_node_credential, verifier},
};
use aq_worker_protocol::*;
use chrono::{Duration, Utc};
use uuid::Uuid;

const NOW: u64 = 1_000_000;
struct Harness {
    store: Store,
    identity: NodeIdentity,
    credential: String,
    db: TestDb,
}
impl Harness {
    async fn new() -> Self {
        let db = TestDb::create().await;
        let store = db.store().await;
        let now = Utc::now();
        let owner = store
            .bootstrap_owner("owner@localhost", "dev", "owner@localhost", now)
            .await
            .unwrap()
            .unwrap();
        let credential = new_node_credential();
        store
            .create_enrollment("test-code", owner.id, now, now + Duration::minutes(5))
            .await
            .unwrap();
        let node = store
            .redeem_enrollment(
                "test-code",
                NewNode {
                    name: "test".into(),
                    credential_hash: verifier(&credential),
                    worker_version: "0.1.0".into(),
                    protocol_version: 1,
                },
                now,
            )
            .await
            .unwrap()
            .unwrap();
        let identity = NodeIdentity {
            node_id: node.id,
            boot_id: Uuid::new_v4(),
        };
        store
            .execution_register(&register(&identity))
            .await
            .unwrap();
        Self {
            store,
            identity,
            credential,
            db,
        }
    }
    async fn offer(&self, task_id: Uuid) -> Offer {
        let offer = make_offer(&self.identity);
        self.store.execution_offer(task_id, &offer).await.unwrap();
        offer
    }
}
fn register(identity: &NodeIdentity) -> RegisterRequest {
    RegisterRequest {
        identity: identity.clone(),
        operation_id: Uuid::new_v4(),
        protocol_version: EXECUTION_PROTOCOL_VERSION,
        worker_version: "0.1.0".into(),
        capabilities: vec![FAKE_HARNESS_CAPABILITY.into()],
    }
}
fn make_offer(identity: &NodeIdentity) -> Offer {
    let spec = ExecutionSpec {
        project_id: Uuid::new_v4(),
        harness: FAKE_HARNESS_CAPABILITY.into(),
        resolved_spec_digest: String::new(),
        fixture_output: "fixture finished".into(),
        resources: Resources {
            slots: 1,
            cpu_millis: 100,
            memory_bytes: 1024,
            disk_bytes: 1024,
        },
        required_capabilities: vec![FAKE_HARNESS_CAPABILITY.into()],
    };
    let spec = spec.seal();
    Offer {
        offer_id: Uuid::new_v4(),
        identity: identity.clone(),
        attempt: AttemptRef {
            attempt_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            generation: 1,
        },
        spec,
        offer_expires_at_ms: NOW + 30_000,
        reservation_ttl_ms: 30_000,
    }
}
fn acceptance(offer: &Offer) -> AcceptRequest {
    AcceptRequest {
        identity: offer.identity.clone(),
        operation_id: Uuid::new_v4(),
        offer_id: offer.offer_id,
        attempt: offer.attempt.clone(),
        resolved_spec_digest: offer.spec.resolved_spec_digest.clone(),
        reservation_id: Uuid::new_v4(),
        reservation_deadline_ms: 500_000,
        reservation_remaining_ms: 20_000,
    }
}
fn report(offer: &Offer) -> ResultReport {
    ResultReport {
        identity: offer.identity.clone(),
        report_id: Uuid::new_v4(),
        supersedes_report_id: None,
        attempt: offer.attempt.clone(),
        resolved_spec_digest: offer.spec.resolved_spec_digest.clone(),
        outcome: ProcessOutcome::Success,
        stop_certainty: StopCertainty::ConfirmedStopped,
        claimed_success: true,
        output: "finished".into(),
        artifacts: vec![],
        pending_operation_ids: vec![],
    }
}
fn is_error<T: std::fmt::Debug>(result: Result<T, Error>, code: ErrorCode) {
    assert!(
        matches!(result, Err(Error::Refused(c, _)) if c == code),
        "expected {code:?}, got {result:?}"
    );
}

#[tokio::test]
async fn acceptance_replay_is_durable_and_does_not_extend_authority() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    let accept = acceptance(&offer);
    let first = h.store.execution_accept(&accept, NOW).await.unwrap();
    let reconnected = Store::connect_with(h.db.options()).await.unwrap();
    let replay = reconnected
        .execution_accept(&accept, NOW + 10_000)
        .await
        .unwrap();
    assert_eq!(first.grant.grant_sequence, replay.grant.grant_sequence);
    assert_eq!(first.grant.reservation_id, replay.grant.reservation_id);
    assert_eq!(replay.grant.valid_for_ms, MAX_GRANT_MS - 10_000);
    is_error(
        reconnected
            .execution_accept(&accept, NOW + MAX_GRANT_MS)
            .await,
        ErrorCode::GrantExpired,
    );
    let mut changed = accept.clone();
    changed.reservation_id = Uuid::new_v4();
    is_error(
        reconnected.execution_accept(&changed, NOW + 10_000).await,
        ErrorCode::IdConflict,
    );
}

#[tokio::test]
async fn concurrent_claims_for_one_task_leave_one_unsettled_attempt() {
    let h = Harness::new().await;
    let task = Uuid::new_v4();
    let a = make_offer(&h.identity);
    let b = make_offer(&h.identity);
    let (a, b) = tokio::join!(
        h.store.execution_offer(task, &a),
        h.store.execution_offer(task, &b)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let offer = a.or(b).unwrap();
    let accept = acceptance(&offer);
    let (a, b) = tokio::join!(
        h.store.execution_accept(&accept, NOW),
        h.store.execution_accept(&accept, NOW)
    );
    assert_eq!(a.unwrap(), b.unwrap());
}

#[tokio::test]
async fn renewal_replay_preserves_expiry_and_rejects_stale_sequence() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let request = RenewRequest {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        attempt: offer.attempt.clone(),
        grant_sequence: 1,
    };
    let renewed = h
        .store
        .execution_renew(&request, NOW + 10_000)
        .await
        .unwrap();
    assert_eq!(renewed.grant.grant_sequence, 2);
    let replay = h
        .store
        .execution_renew(&request, NOW + 20_000)
        .await
        .unwrap();
    assert_eq!(replay.grant.valid_for_ms, MAX_GRANT_MS - 10_000);
    let mut stale = request.clone();
    stale.operation_id = Uuid::new_v4();
    is_error(
        h.store.execution_renew(&stale, NOW + 20_000).await,
        ErrorCode::StaleGeneration,
    );
    is_error(
        h.store
            .execution_renew(&request, NOW + 10_000 + MAX_GRANT_MS)
            .await,
        ErrorCode::GrantExpired,
    );
}

#[tokio::test]
async fn cancellation_wins_before_result_and_retains_late_output() {
    let h = Harness::new().await;
    let task = Uuid::new_v4();
    let offer = h.offer(task).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let op = Uuid::new_v4();
    h.store
        .execution_cancel(offer.attempt.attempt_id, op, "changed outcome")
        .await
        .unwrap();
    let mut next = make_offer(&h.identity);
    is_error(
        h.store.execution_offer(task, &next).await,
        ErrorCode::IdConflict,
    );
    let result = report(&offer);
    let response = h.store.execution_result(&result, NOW + 1).await.unwrap();
    assert_eq!(response.disposition, ResultDisposition::RetainedCancelled);
    assert_eq!(
        h.store.execution_result(&result, NOW + 2).await.unwrap(),
        response
    );
    next.spec.fixture_output = "replacement".into();
    next.spec = next.spec.seal();
    h.store.execution_offer(task, &next).await.unwrap();
    is_error(
        h.store
            .execution_cancel(offer.attempt.attempt_id, op, "different reason")
            .await,
        ErrorCode::IdConflict,
    );
}

#[tokio::test]
async fn accepted_result_cannot_be_retroactively_cancelled() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let result = report(&offer);
    assert_eq!(
        h.store
            .execution_result(&result, NOW + 1)
            .await
            .unwrap()
            .disposition,
        ResultDisposition::Accepted
    );
    let state = h
        .store
        .execution_cancel(offer.attempt.attempt_id, Uuid::new_v4(), "too late")
        .await
        .unwrap();
    assert!(state.cancellation_id.is_none());
    assert_eq!(
        state.disposition.unwrap().disposition,
        ResultDisposition::Accepted
    );
}

#[tokio::test]
async fn expired_execution_remains_held_until_stopped_and_reported() {
    let h = Harness::new().await;
    let task = Uuid::new_v4();
    let offer = h.offer(task).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let next = make_offer(&h.identity);
    is_error(
        h.store.execution_offer(task, &next).await,
        ErrorCode::IdConflict,
    );
    let mut result = report(&offer);
    result.stop_certainty = StopCertainty::StopUncertain;
    assert_eq!(
        h.store
            .execution_result(&result, NOW + MAX_GRANT_MS)
            .await
            .unwrap()
            .disposition,
        ResultDisposition::Rejected
    );
    is_error(
        h.store.execution_offer(task, &next).await,
        ErrorCode::IdConflict,
    );
    let mut corrected = report(&offer);
    corrected.supersedes_report_id = Some(result.report_id);
    assert_eq!(
        h.store
            .execution_result(&corrected, NOW + MAX_GRANT_MS + 1)
            .await
            .unwrap()
            .disposition,
        ResultDisposition::RetainedStale
    );
    h.store.execution_offer(task, &next).await.unwrap();
}

#[tokio::test]
async fn unresolved_external_operations_prevent_replacement_even_after_stop() {
    let h = Harness::new().await;
    let task = Uuid::new_v4();
    let offer = h.offer(task).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let mut result = report(&offer);
    result.pending_operation_ids.push(Uuid::new_v4());
    assert!(
        !h.store
            .execution_result(&result, NOW + 1)
            .await
            .unwrap()
            .retention_release
    );
    is_error(
        h.store
            .execution_offer(task, &make_offer(&h.identity))
            .await,
        ErrorCode::IdConflict,
    );
}

#[tokio::test]
async fn boot_replacement_requires_stop_and_old_results_never_settle_new_authority() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let next = NodeIdentity {
        node_id: h.identity.node_id,
        boot_id: Uuid::new_v4(),
    };
    let replacement = register(&next);
    is_error(
        h.store.execution_register(&replacement).await,
        ErrorCode::StaleBoot,
    );
    is_error(
        h.store.execution_replace_incarnation(&replacement).await,
        ErrorCode::CapacityUnavailable,
    );
    let observation = Observation {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        attempt: offer.attempt.clone(),
        sequence: 1,
        phase: Phase::Collecting,
        stop_certainty: StopCertainty::ConfirmedStopped,
        detail: "all writers stopped".into(),
    };
    h.store.execution_observe(&observation).await.unwrap();
    h.store
        .execution_replace_incarnation(&replacement)
        .await
        .unwrap();
    assert_eq!(
        h.store
            .execution_result(&report(&offer), NOW + 1)
            .await
            .unwrap()
            .disposition,
        ResultDisposition::RetainedStale
    );
}

#[tokio::test]
async fn reconciliation_does_not_extend_grants_or_take_over_old_incarnations() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let mut request = ReconcileRequest {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        attempts: vec![ReconcileAttempt {
            attempt: offer.attempt.clone(),
            previous_boot_id: h.identity.boot_id,
            phase: Phase::Running,
            launch_id: Some(Uuid::new_v4()),
            stop_certainty: StopCertainty::Running,
            report_id: None,
        }],
        cursor: None,
        max_items: 1,
    };
    let response = h
        .store
        .execution_reconcile(&request, NOW + 10_000)
        .await
        .unwrap();
    assert_eq!(
        response.attempts[0].grant.as_ref().unwrap().valid_for_ms,
        MAX_GRANT_MS - 10_000
    );
    request.identity.boot_id = Uuid::new_v4();
    let response = h
        .store
        .execution_reconcile(&request, NOW + 11_000)
        .await
        .unwrap();
    assert_eq!(
        response.attempts[0].action,
        ReconcileAction::StopAndPreserve
    );
    assert!(response.attempts[0].grant.is_none());
}

#[tokio::test]
async fn expired_unaccepted_offer_and_explicit_decline_release_task_claim() {
    let h = Harness::new().await;
    let task = Uuid::new_v4();
    let offer = h.offer(task).await;
    let poll = PollRequest {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        inventory_sequence: 1,
        available: offer.spec.resources.clone(),
        max_items: 16,
    };
    assert!(
        h.store
            .execution_poll(&poll, offer.offer_expires_at_ms)
            .await
            .unwrap()
            .offers
            .is_empty()
    );
    let next = make_offer(&h.identity);
    h.store.execution_offer(task, &next).await.unwrap();
    let decline = DeclineRequest {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        offer_id: next.offer_id,
        reason: "capacity changed".into(),
    };
    let ack = h.store.execution_decline(&decline).await.unwrap();
    assert_eq!(h.store.execution_decline(&decline).await.unwrap(), ack);
    assert_eq!(
        h.store
            .execution_get(next.attempt.attempt_id)
            .await
            .unwrap()
            .status,
        AuthorityStatus::Declined
    );
}

#[tokio::test]
async fn observation_replay_cannot_regress_or_resurrect_stopped_processes() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let observation = Observation {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        attempt: offer.attempt.clone(),
        sequence: 1,
        phase: Phase::Collecting,
        stop_certainty: StopCertainty::ConfirmedStopped,
        detail: "done".into(),
    };
    let ack = h.store.execution_observe(&observation).await.unwrap();
    assert_eq!(h.store.execution_observe(&observation).await.unwrap(), ack);
    let renew = RenewRequest {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        attempt: offer.attempt.clone(),
        grant_sequence: 1,
    };
    is_error(
        h.store.execution_renew(&renew, NOW + 1).await,
        ErrorCode::Cancelled,
    );
    let mut regression = observation.clone();
    regression.operation_id = Uuid::new_v4();
    regression.sequence = 2;
    regression.phase = Phase::Running;
    regression.stop_certainty = StopCertainty::Running;
    is_error(
        h.store.execution_observe(&regression).await,
        ErrorCode::InvalidRequest,
    );
}

#[tokio::test]
async fn execution_http_authenticates_node_binding_and_keeps_routes_separate() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let h = Harness::new().await;
    let config: aq_server::config::Config = toml::from_str(
        r#"
format_version=1
environment="development"
[server]
admin_bind="127.0.0.1:0"
worker_bind="127.0.0.1:0"
public_origin="http://127.0.0.1:8080"
[auth]
mode="dev"
dev_email="owner@localhost"
[bootstrap]
enabled=true
owner_email="owner@localhost"
"#,
    )
    .unwrap();
    let state = aq_server::AppState {
        identity: aq_server::identity::verifier_from_config(&config.auth, reqwest::Client::new()),
        config: std::sync::Arc::new(config),
        store: h.store.clone(),
        clock: aq_server::system_clock(),
    };
    let app = aq_server::build_worker_router(state.clone());
    let mut request = register(&h.identity);
    request.identity.node_id = Uuid::new_v4();
    let req = Request::builder()
        .method("POST")
        .uri("/v2/worker/register")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", h.credential))
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();
    assert_eq!(
        app.oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    let req = Request::builder()
        .method("POST")
        .uri("/v2/worker/register")
        .header("x-aq-request", "1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        aq_server::build_admin_router(state)
            .oneshot(req)
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}

fn recovery(identity: &NodeIdentity) -> ReconcileRequest {
    ReconcileRequest {
        identity: identity.clone(),
        operation_id: Uuid::new_v4(),
        attempts: vec![],
        cursor: None,
        max_items: 64,
    }
}

#[tokio::test]
async fn explicit_recovery_permits_only_exact_boot_after_all_writers_stop() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    let next = NodeIdentity {
        node_id: h.identity.node_id,
        boot_id: Uuid::new_v4(),
    };
    let registration = register(&next);
    is_error(
        h.store.execution_register(&registration).await,
        ErrorCode::StaleBoot,
    );
    // Unaccepted offers cannot have launched a writer. The recovery request
    // grants a one-boot permit; registration retires the old offers.
    h.store
        .execution_reconcile(&recovery(&next), NOW)
        .await
        .unwrap();
    let wrong = NodeIdentity {
        node_id: h.identity.node_id,
        boot_id: Uuid::new_v4(),
    };
    is_error(
        h.store.execution_register(&register(&wrong)).await,
        ErrorCode::StaleBoot,
    );
    h.store.execution_register(&registration).await.unwrap();
    assert_eq!(
        h.store
            .execution_get(offer.attempt.attempt_id)
            .await
            .unwrap()
            .status,
        AuthorityStatus::Declined
    );
    is_error(
        h.store.execution_accept(&acceptance(&offer), NOW).await,
        ErrorCode::StaleBoot,
    );
}

#[tokio::test]
async fn recovery_inventory_cannot_omit_a_writer_or_race_a_new_acceptance() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    let next = NodeIdentity {
        node_id: h.identity.node_id,
        boot_id: Uuid::new_v4(),
    };
    let registration = register(&next);
    h.store
        .execution_reconcile(&recovery(&next), NOW)
        .await
        .unwrap();
    // Old boot gets a grant after reconciliation, before replacement register.
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    is_error(
        h.store.execution_register(&registration).await,
        ErrorCode::StaleBoot,
    );
    // Neither omitting the writer nor waiting past expiry establishes stop.
    h.store
        .execution_reconcile(&recovery(&next), NOW + MAX_GRANT_MS)
        .await
        .unwrap();
    is_error(
        h.store.execution_register(&registration).await,
        ErrorCode::StaleBoot,
    );
    h.store
        .execution_result(&report(&offer), NOW + MAX_GRANT_MS)
        .await
        .unwrap();
    h.store
        .execution_reconcile(&recovery(&next), NOW + MAX_GRANT_MS)
        .await
        .unwrap();
    h.store.execution_register(&registration).await.unwrap();
}

#[tokio::test]
async fn corrected_reports_preserve_original_immutable_body_and_admission() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let mut original = report(&offer);
    original.stop_certainty = StopCertainty::StopUncertain;
    original.output = "first retained evidence".into();
    let first = h.store.execution_result(&original, NOW).await.unwrap();
    let mut correction = report(&offer);
    correction.supersedes_report_id = Some(original.report_id);
    correction.output = "corrected evidence".into();
    let second = h
        .store
        .execution_result(&correction, NOW + 1)
        .await
        .unwrap();
    let reconnected = Store::connect_with(h.db.options()).await.unwrap();
    assert_eq!(
        reconnected
            .execution_report(h.identity.node_id, original.report_id)
            .await
            .unwrap(),
        (original.clone(), first.clone())
    );
    assert_eq!(
        reconnected
            .execution_report(h.identity.node_id, correction.report_id)
            .await
            .unwrap(),
        (correction, second)
    );
    assert_eq!(
        reconnected
            .execution_result(&original, NOW + 2)
            .await
            .unwrap(),
        first
    );
    let mut changed = original;
    changed.output = "tampered".into();
    is_error(
        reconnected.execution_result(&changed, NOW + 3).await,
        ErrorCode::IdConflict,
    );
}

#[tokio::test]
async fn repeated_cancellations_coalesce_one_worker_directive() {
    let h = Harness::new().await;
    let offer = h.offer(Uuid::new_v4()).await;
    h.store
        .execution_accept(&acceptance(&offer), NOW)
        .await
        .unwrap();
    let first_id = Uuid::new_v4();
    let first = h
        .store
        .execution_cancel(offer.attempt.attempt_id, first_id, "stop")
        .await
        .unwrap();
    for _ in 0..80 {
        let state = h
            .store
            .execution_cancel(offer.attempt.attempt_id, Uuid::new_v4(), "again")
            .await
            .unwrap();
        assert_eq!(state.cancellation_id, Some(first_id));
    }
    let poll = PollRequest {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        inventory_sequence: 1,
        available: offer.spec.resources.clone(),
        max_items: 64,
    };
    let result = h.store.execution_poll(&poll, NOW).await.unwrap();
    assert_eq!(result.directives.len(), 1);
    assert_eq!(result.directives[0].directive_id, first_id);
    assert_eq!(
        h.store
            .execution_cancel(offer.attempt.attempt_id, first_id, "stop")
            .await
            .unwrap()
            .cancellation_id,
        first.cancellation_id
    );
}

#[tokio::test]
async fn fixture_assignment_requires_owner_dev_mode_and_sealed_inputs() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let h = Harness::new().await;
    let mut config: aq_server::config::Config = toml::from_str(
        r#"
format_version=1
environment="development"
[server]
admin_bind="127.0.0.1:0"
worker_bind="127.0.0.1:0"
public_origin="http://127.0.0.1:8080"
[auth]
mode="dev"
dev_email="owner@localhost"
[bootstrap]
enabled=true
owner_email="owner@localhost"
"#,
    )
    .unwrap();
    // Use an already authenticated test identity to reach the production-mode
    // guard; JWT verification itself has its own foundation suite.
    let identity = aq_server::identity::verifier_from_config(&config.auth, reqwest::Client::new());
    let clock: aq_server::Clock =
        std::sync::Arc::new(|| chrono::DateTime::from_timestamp_millis(NOW as i64).unwrap());
    let app = |config| {
        aq_server::build_admin_router(aq_server::AppState {
            config: std::sync::Arc::new(config),
            identity: identity.clone(),
            store: h.store.clone(),
            clock: clock.clone(),
        })
    };
    let offer = make_offer(&h.identity);
    let request = |offer: &Offer, email: &str| {
        Request::builder()
            .method("POST")
            .uri("/api/execution/fixtures")
            .header("content-type", "application/json")
            .header("x-aq-request", "1")
            .header("x-aq-dev-email", email)
            .body(Body::from(
                serde_json::to_vec(&serde_json::json!({"task_id":Uuid::new_v4(), "offer":offer}))
                    .unwrap(),
            ))
            .unwrap()
    };
    // An unknown development visitor is not an owner.
    assert_eq!(
        app(config.clone())
            .oneshot(request(&offer, "visitor@localhost"))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let mut changed = offer.clone();
    changed.spec.fixture_output = "unsealed edit".into();
    assert_eq!(
        app(config.clone())
            .oneshot(request(&changed, "owner@localhost"))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app(config.clone())
            .oneshot(request(&offer, "owner@localhost"))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    config.auth = aq_server::config::AuthConfig::CloudflareAccess {
        team_domain: "https://example.cloudflareaccess.com".into(),
        audience: "fixture".into(),
    };
    assert_eq!(
        app(config)
            .oneshot(request(&make_offer(&h.identity), "owner@localhost"))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn cancelled_unaccepted_offer_never_queues_an_unknown_process_directive() {
    let h = Harness::new().await;
    let task = Uuid::new_v4();
    let old = h.offer(task).await;
    h.store
        .execution_cancel(old.attempt.attempt_id, Uuid::new_v4(), "not needed")
        .await
        .unwrap();
    let next = h.offer(task).await;
    let poll = PollRequest {
        identity: h.identity.clone(),
        operation_id: Uuid::new_v4(),
        inventory_sequence: 1,
        available: next.spec.resources.clone(),
        max_items: 64,
    };
    let response = h.store.execution_poll(&poll, NOW).await.unwrap();
    assert!(response.directives.is_empty());
    assert_eq!(response.offers, vec![next]);
    is_error(
        h.store.execution_accept(&acceptance(&old), NOW).await,
        ErrorCode::IdConflict,
    );
}

#[tokio::test]
async fn conflicting_task_claims_on_different_nodes_share_one_central_fence() {
    let h = Harness::new().await;
    let now = Utc::now();
    let owner = h
        .store
        .find_user_by_binding("dev", "owner@localhost")
        .await
        .unwrap()
        .unwrap();
    let credential = new_node_credential();
    h.store
        .create_enrollment("second-node", owner.id, now, now + Duration::minutes(5))
        .await
        .unwrap();
    let node = h
        .store
        .redeem_enrollment(
            "second-node",
            NewNode {
                name: "second".into(),
                credential_hash: verifier(&credential),
                worker_version: "0.1.0".into(),
                protocol_version: 1,
            },
            now,
        )
        .await
        .unwrap()
        .unwrap();
    let other = NodeIdentity {
        node_id: node.id,
        boot_id: Uuid::new_v4(),
    };
    h.store.execution_register(&register(&other)).await.unwrap();
    let task = Uuid::new_v4();
    let a = make_offer(&h.identity);
    let b = make_offer(&other);
    let (a, b) = tokio::join!(
        h.store.execution_offer(task, &a),
        h.store.execution_offer(task, &b)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let losing = if a.is_err() { a } else { b };
    is_error(losing, ErrorCode::IdConflict);
}
