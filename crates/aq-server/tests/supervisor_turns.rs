//! Actual mailbox/reservation/session persistence with synthetic lifecycle evidence.
//! No host processes, provider calls, or sleeps.
use aq_central_authority::{owner::*, *};
use aq_server::{
    AppState,
    agent_access::supervisor_conversation::{
        self as conversation, dispatch, reply,
        reservations::{
            Cancel, Configure, LocalPolicy, PolicyKind, Preview, PreviewRequest, PreviewStatus,
            ReservationOwner, ReservationState, Reserve,
        },
    },
    config::Config,
    identity::verifier_from_config,
    store::User,
    test_support::{TestDb, with_store_profile},
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct Evidence;
#[async_trait::async_trait]
impl EvidenceVerifier for Evidence {
    async fn authorize_plan(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        plan: &CentralPlan,
    ) -> OwnerResult<()> {
        let digest: String = sqlx::query_scalar(
            "SELECT input_digest FROM central_supervisor_dispatches WHERE dispatch_id=$1",
        )
        .bind(plan.dispatch_id)
        .fetch_one(&mut **tx)
        .await?;
        let digest = serde_json::from_value(json!(digest)).unwrap();
        dispatch::authorize_dispatch(tx, plan, &digest)
            .await
            .map_err(|_| OwnerError::Conflict)
    }
    async fn verify_event(
        &self,
        _: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _: &Run,
        _: &Event,
    ) -> OwnerResult<()> {
        Ok(())
    }
}
fn artifact() -> ArtifactRef {
    ArtifactRef {
        id: Text::new("synthetic-evidence").unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(b"synthetic"),
    }
}
fn clock() -> Clock {
    Clock {
        machine_boot_id: Uuid::from_u128(9),
        boot_ms: 1000,
    }
}
fn observation(plan: &CentralPlan) -> HostObservationRef {
    HostObservationRef {
        binding: plan.binding().unwrap(),
        launch_id: Uuid::from_u128(plan.generation as u128 + 100),
        guardian: artifact(),
        observation: artifact(),
    }
}
struct Fixture {
    db: TestDb,
    state: AppState,
    user: User,
    pool: PgPool,
    app: Router,
    actor: Uuid,
    conversation: Uuid,
}
impl Fixture {
    async fn new(version: u32) -> Self {
        let db = TestDb::create().await;
        let store = with_store_profile(version, db.store()).await;
        let now = Utc.timestamp_millis_opt(1_000_000).unwrap();
        let user = store
            .bootstrap_owner("owner@localhost", "dev", "owner@localhost", now)
            .await
            .unwrap()
            .unwrap();
        let config: Config = toml::from_str("format_version=1\nenvironment='development'\n[server]\nadmin_bind='127.0.0.1:0'\nworker_bind='127.0.0.1:0'\npublic_origin='http://127.0.0.1:8080'\n[auth]\nmode='dev'\ndev_email='owner@localhost'\n[bootstrap]\nowner_email='owner@localhost'\nenabled=true\n").unwrap();
        let state = AppState {
            store,
            identity: verifier_from_config(&config.auth, reqwest::Client::new()),
            config: Arc::new(config),
            clock: Arc::new(move || now),
            agent_access: None,
            source_retention: None,
        };
        let app = conversation::routes(&state)
            .merge(conversation::supervisor_routes(&state))
            .with_state(state.clone());
        let pool = db.pool().await;
        let conversation =
            sqlx::query_scalar("SELECT conversation_id FROM global_supervisor_conversation")
                .fetch_one(&pool)
                .await
                .unwrap();
        let f = Self {
            db,
            state,
            user,
            pool,
            app,
            actor: Uuid::new_v4(),
            conversation,
        };
        f.owner()
            .configure_actor(
                &f.user,
                &serde_json::to_vec(&Configure {
                    operation_id: Uuid::new_v4(),
                    actor_id: f.actor,
                    expected_actor_revision: 0,
                    policy: LocalPolicy {
                        schema_version: 1,
                        kind: PolicyKind::GlobalSupervisorReservation,
                        actor_id: f.actor,
                        conversation_id: f.conversation,
                        enabled: true,
                    },
                })
                .unwrap(),
            )
            .await
            .unwrap();
        f
    }
    fn owner(&self) -> ReservationOwner {
        ReservationOwner::new(&self.state)
    }
    fn sessions(&self) -> SessionOwner<Evidence> {
        SessionOwner::new(self.pool.clone(), Evidence)
    }
    async fn post(&self, path: &str, token: Option<&str>, value: Value) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json");
        request = if let Some(token) = token {
            request.header("authorization", format!("Bearer {token}"))
        } else {
            request
                .header("x-aq-dev-email", "owner@localhost")
                .header("x-aq-request", "1")
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::from(value.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    async fn send(&self) -> Uuid {
        let id = Uuid::new_v4();
        let (status, result) = self.post(&format!("/api/human/conversations/{}/messages", self.conversation), None, json!({"operation_id":Uuid::new_v4(),"message_id":id,"expected_conversation_revision":"1","body":"next owner message","context":[],"attachments":[]})).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        id
    }
    async fn preview(&self) -> Preview {
        let revision = self
            .owner()
            .inspect_actor(&self.user, self.actor)
            .await
            .unwrap()
            .revision;
        self.owner()
            .preview_batch(
                &self.user,
                PreviewRequest {
                    actor_id: self.actor,
                    expected_actor_revision: revision,
                    scan_after: 0,
                    through_cursor: None,
                },
            )
            .await
            .unwrap()
    }
    async fn reserve(&self) -> conversation::reservations::Receipt {
        let p = self.preview().await;
        assert_eq!(p.status, PreviewStatus::Ready);
        self.owner()
            .reserve_batch(
                &self.user,
                &serde_json::to_vec(&Reserve {
                    operation_id: Uuid::new_v4(),
                    reservation_id: Uuid::new_v4(),
                    actor_id: self.actor,
                    expected_actor_revision: p.actor_revision,
                    expected_policy_revision: p.policy_revision,
                    scan_after: p.scan_after,
                    through_cursor: p.through_cursor,
                    selection_digest: p.selection_digest,
                })
                .unwrap(),
            )
            .await
            .unwrap()
    }
    async fn plan(&self, reservation: Uuid, generation: u64) -> CentralPlan {
        let examples: Value = serde_json::from_str(include_str!(
            "../../aq-central-authority/artifacts/examples.json"
        ))
        .unwrap();
        let Document::Plan(mut p) =
            decode(&serde_json::to_vec(&examples["plan"]).unwrap()).unwrap()
        else {
            panic!()
        };
        let CentralSubject::CentralConversation {
            actor_id,
            conversation_id,
            ..
        } = &mut p.subject;
        *actor_id = self.actor;
        *conversation_id = self.conversation;
        p.generation = generation;
        let SessionIdentity::Central {
            session_id,
            generation: g,
            ..
        } = &mut p.session
        else {
            panic!()
        };
        *session_id = Uuid::new_v4();
        *g = generation;
        let turn = dispatch::prepare_turn(
            &self.pool,
            self.actor,
            reservation,
            generation,
            *session_id,
            1_000_000,
        )
        .await
        .unwrap();
        p.batch = turn.batch;
        p.dispatch_id = turn.dispatch_id;
        p.host_operation_id = turn.host_operation_id;
        p
    }
    async fn start(&self, p: &CentralPlan) -> Snapshot {
        self.sessions().prepare(p.clone(), clock()).await.unwrap();
        let receipt = self
            .sessions()
            .start(
                &p.binding().unwrap(),
                HostLaunchRef {
                    binding: p.binding().unwrap(),
                    launch_id: observation(p).launch_id,
                    intent: artifact(),
                },
                100_000,
                clock(),
            )
            .await
            .unwrap();
        assert_eq!(receipt.decision, Decision::Applied);
        receipt.snapshot
    }
    async fn report(&self, p: &CentralPlan, event: Event) -> Snapshot {
        let receipt = self
            .sessions()
            .report(&p.binding().unwrap(), event, clock())
            .await
            .unwrap();
        assert!(
            !matches!(receipt.decision, Decision::Refused { .. }),
            "{:?}",
            receipt.decision
        );
        receipt.snapshot
    }
    async fn assert_consumed(&self, reservation: Uuid) {
        assert!(
            self.owner()
                .inspect_actor(&self.user, self.actor)
                .await
                .unwrap()
                .current_reservation
                .is_none()
        );
        assert_eq!(
            self.owner()
                .history(&self.user, self.actor, None, 16)
                .await
                .unwrap()
                .items
                .iter()
                .find(|r| r.id == reservation)
                .unwrap()
                .state,
            ReservationState::Consumed
        );
    }
    async fn answer(&self, p: &CentralPlan) -> (String, Value) {
        let token = reply::issue_credential(&self.pool, self.actor, 60_000, 999_000)
            .await
            .unwrap();
        let request = json!({"operation_id":Uuid::new_v4(),"message_id":Uuid::new_v4(),"dispatch_id":p.dispatch_id,"body":"supervisor answer"});
        let (status, receipt) = self
            .post(
                "/v1/supervisor/conversation/replies",
                Some(&token),
                request.clone(),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{receipt}");
        (token, request)
    }
}

#[tokio::test]
async fn reply_consumes_reservation_and_allows_successor_turn() {
    let f = Fixture::new(aq_server::STATE_FORMAT_VERSION).await;
    let first = f.send().await;
    let receipt = f.reserve().await;
    let reservation = receipt.reservation.as_ref().unwrap();
    let p = f.plan(reservation.id, 1).await;
    let s = f.start(&p).await;
    f.report(
        &p,
        Event::Handoff {
            evidence: HandoffEvidence {
                association: s.run.association().unwrap().clone(),
                disposition: HandoffDisposition::HandedOff,
                receipt: artifact(),
            },
        },
    )
    .await;
    let (token, request) = f.answer(&p).await;
    f.assert_consumed(reservation.id).await;
    let lookup = f
        .owner()
        .lookup_operation(&f.user, f.actor, receipt.operation_id)
        .await
        .unwrap();
    assert_eq!(
        lookup.receipt, receipt,
        "the original reserve receipt remains unchanged"
    );
    assert_eq!(
        lookup.current_reservation.unwrap().state,
        ReservationState::Consumed
    );
    let second = f.send().await;
    let next_receipt = f.reserve().await;
    let next = next_receipt.reservation.unwrap();
    assert_eq!(next.batch.messages.len(), 1);
    assert_eq!(next.batch.messages[0].message_id, second);
    assert_ne!(first, second);
    // Replaying an old reply cannot clear the newer reservation.
    let (status, _) = f
        .post(
            "/v1/supervisor/conversation/replies",
            Some(&token),
            request.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        f.owner()
            .inspect_actor(&f.user, f.actor)
            .await
            .unwrap()
            .current_reservation
            .unwrap()
            .id,
        next.id
    );
    // One physical generation remains one immutable turn.
    assert!(
        dispatch::prepare_turn(
            &f.pool,
            f.actor,
            next.id,
            1,
            dispatch_session(&p),
            1_000_000
        )
        .await
        .is_err()
    );
    let p2 = f.plan(next.id, 2).await;
    assert!(
        f.sessions().prepare(p2.clone(), clock()).await.is_err(),
        "successor still needs stopped writers"
    );
    f.report(
        &p,
        Event::Stopped {
            evidence: StopEvidenceRef {
                host: observation(&p),
                stopped_writers: artifact(),
            },
        },
    )
    .await;
    f.start(&p2).await;
    assert_eq!(
        f.post("/v1/supervisor/conversation/replies", Some(&token), request)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    f.answer(&p2).await;
    f.assert_consumed(next.id).await;
    assert!(
        f.preview().await.batch.is_none(),
        "completed mail is never reselected"
    );
    // Reopening validates consumed proofs and retains both generations.
    f.db.store().await;
    assert!(f.sessions().generation(f.actor, 1).await.unwrap().is_some());
    assert!(
        dispatch::prepare_turn(
            &f.pool,
            f.actor,
            reservation.id,
            2,
            dispatch_session(&p2),
            1_000_000
        )
        .await
        .is_err()
    );
}
fn dispatch_session(p: &CentralPlan) -> Uuid {
    match p.session {
        SessionIdentity::Central { session_id, .. } => session_id,
        _ => panic!(),
    }
}

#[tokio::test]
async fn terminal_not_delivered_consumes_but_unknown_and_running_hold_mail() {
    let f = Fixture::new(aq_server::STATE_FORMAT_VERSION).await;
    let first = f.send().await;
    let receipt = f.reserve().await;
    let r = receipt.reservation.unwrap();
    let p = f.plan(r.id, 1).await;
    let s = f.start(&p).await;
    f.report(
        &p,
        Event::Running {
            observation: observation(&p),
        },
    )
    .await;
    assert_eq!(
        f.preview().await.status,
        PreviewStatus::ReservationOutstanding
    );
    let association = s.run.association().unwrap().clone();
    f.report(
        &p,
        Event::ReportUnknown {
            association: association.clone(),
        },
    )
    .await;
    f.send().await;
    assert_eq!(
        f.preview().await.status,
        PreviewStatus::ReservationOutstanding
    );
    // The old pre-plan cancellation route cannot release an uncertain dispatch.
    assert!(
        f.owner()
            .cancel_reservation(
                &f.user,
                &serde_json::to_vec(&Cancel {
                    operation_id: Uuid::new_v4(),
                    actor_id: f.actor,
                    reservation_id: r.id,
                    expected_reservation_revision: 1
                })
                .unwrap()
            )
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE central_supervisor_reservations SET state='consumed',revision=2 WHERE reservation_id=$1").bind(r.id).execute(&f.pool).await.is_err(), "unknown is not terminal consumption evidence");
    f.db.store().await;
    assert_eq!(
        f.preview().await.status,
        PreviewStatus::ReservationOutstanding
    );
    let event = Event::ReconcileHandoff {
        evidence: HandoffEvidence {
            association,
            disposition: HandoffDisposition::NotHandedOff,
            receipt: artifact(),
        },
    };
    f.report(&p, event.clone()).await;
    f.assert_consumed(r.id).await;
    // Repeated terminal reports are idempotent; terminal failure is not an automatic retry.
    f.report(&p, event).await;
    let next = f.reserve().await.reservation.unwrap();
    assert!(next.batch.messages.iter().all(|m| m.message_id != first));
    f.db.store().await;
}

#[tokio::test]
async fn unknown_reply_releases_reservation_without_resending_unknown_input() {
    let f = Fixture::new(aq_server::STATE_FORMAT_VERSION).await;
    let first = f.send().await;
    let r = f.reserve().await.reservation.unwrap();
    let p = f.plan(r.id, 1).await;
    let s = f.start(&p).await;
    f.report(
        &p,
        Event::ReportUnknown {
            association: s.run.association().unwrap().clone(),
        },
    )
    .await;
    f.answer(&p).await;
    f.assert_consumed(r.id).await;
    let retained = f.sessions().inspect(f.actor).await.unwrap().unwrap();
    assert!(matches!(
        retained.run.dispatch(),
        DispatchState::Unknown { .. }
    ));
    let second = f.send().await;
    let next = f.reserve().await.reservation.unwrap();
    assert_eq!(
        next.batch
            .messages
            .iter()
            .map(|m| m.message_id)
            .collect::<Vec<_>>(),
        vec![second]
    );
    assert_ne!(first, second);
    let p2 = f.plan(next.id, 2).await;
    f.report(
        &p,
        Event::Stopped {
            evidence: StopEvidenceRef {
                host: observation(&p),
                stopped_writers: artifact(),
            },
        },
    )
    .await;
    f.start(&p2).await;
    assert!(matches!(
        f.sessions()
            .generation(f.actor, 1)
            .await
            .unwrap()
            .unwrap()
            .run
            .dispatch(),
        DispatchState::Unknown { .. }
    ));
}

#[tokio::test]
async fn upgrade_consumes_already_recorded_reply_and_keeps_receipt() {
    let f = Fixture::new(48).await;
    f.send().await;
    let receipt = f.reserve().await;
    let r = receipt.reservation.as_ref().unwrap();
    let p = f.plan(r.id, 1).await;
    f.start(&p).await;
    f.answer(&p).await;
    assert!(
        f.owner()
            .inspect_actor(&f.user, f.actor)
            .await
            .unwrap()
            .current_reservation
            .is_some()
    );
    f.db.store().await;
    f.assert_consumed(r.id).await;
    assert_eq!(
        f.owner()
            .lookup_operation(&f.user, f.actor, receipt.operation_id)
            .await
            .unwrap()
            .receipt,
        receipt
    );
    let second = f.send().await;
    assert_eq!(
        f.reserve().await.reservation.unwrap().batch.messages[0].message_id,
        second
    );
}

#[tokio::test]
async fn upgrade_consumes_known_failure_and_retains_unknown_delivery() {
    for unknown in [false, true] {
        let f = Fixture::new(48).await;
        f.send().await;
        let r = f.reserve().await.reservation.unwrap();
        let p = f.plan(r.id, 1).await;
        let s = f.start(&p).await;
        let association = s.run.association().unwrap().clone();
        let event = if unknown {
            Event::ReportUnknown { association }
        } else {
            Event::Handoff {
                evidence: HandoffEvidence {
                    association,
                    disposition: HandoffDisposition::NotHandedOff,
                    receipt: artifact(),
                },
            }
        };
        f.report(&p, event).await;
        f.db.store().await;
        if unknown {
            assert_eq!(
                f.preview().await.status,
                PreviewStatus::ReservationOutstanding
            );
        } else {
            f.assert_consumed(r.id).await;
            assert_eq!(f.preview().await.status, PreviewStatus::NoEligibleMail);
        }
    }
}
