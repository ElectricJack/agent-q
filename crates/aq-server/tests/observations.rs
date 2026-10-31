mod common;
use aq_accounting::{
    Layout,
    projection::Projection,
    reconcile::{ParserAuthority, Policy},
    wire::*,
};
use aq_server::{
    execution::profiled::{Status, Stop},
    observations::{self, OwnerSource},
};
use aq_session_protocol::ProfiledGrant;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use common::profiled::{Fixture, NOW, artifact, fixture, insert};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;
fn text(s: &str) -> Text {
    Text::new(s).unwrap()
}
async fn setup() -> (Fixture, OwnerSource) {
    let mut f = fixture().await;
    f.profiled.status = Status::Granted;
    f.profiled.execution_deadline_ms = Some(NOW + 60_000);
    f.profiled.grant = Some(ProfiledGrant {
        binding: f.profiled.offer.binding.clone(),
        grant_id: Uuid::new_v4(),
        expires_at_ms: NOW + 5000,
    });
    f.profiled.stop = Stop::Running;
    insert(&f).await;
    sqlx::raw_sql(include_str!("../migrations/0014_observation_ledger.sql"))
        .execute(&f.db.pool().await)
        .await
        .unwrap();
    let source = OwnerSource {
        binding: aq_admission::transport::lifecycle_binding(&f.profiled.offer).unwrap(),
        source_id: Uuid::new_v4(),
        generation: 1,
        native_session_key: text("native-key"),
        account_claim: text(&f.selection.choice.account_reference),
        policy: Policy {
            reference: artifact("accounting-policy", b"policy"),
            account_scope: Digest::of(b"account"),
            call_namespace: Digest::of(b"native-key"),
            provider: text(&f.selection.choice.provider),
            layout: Layout {
                policy: artifact("layout", b"layout"),
                input_includes_cache_read: false,
                input_includes_cache_write: false,
                output_includes_reasoning: false,
            },
            parsers: vec![ParserAuthority {
                parser: artifact("parser", b"parser"),
                category_claim: artifact("parser", b"parser"),
                priority: 1,
                complete_accounting_evidence: Some(artifact(
                    "synthetic-qualification",
                    b"test-only",
                )),
            }],
        },
    };
    observations::register_source(&f.state, &source)
        .await
        .unwrap();
    (f, source)
}
fn event(source: &OwnerSource, sequence: u64, input: u64) -> ObservationRecord {
    ObservationRecord::seal(
        Uuid::new_v4(),
        Observation {
            binding: source.binding.clone(),
            provenance: Provenance {
                source_id: source.source_id,
                source_generation: source.generation,
                source_kind: SourceKind::NativeTranscript,
                native_session_key: source.native_session_key.clone(),
                cursor: SourceCursor::Record {
                    id: text(&format!("record-{sequence}")),
                },
                parser: source.policy.parsers[0].parser.clone(),
                raw_record: RawRecordEvidence::Available {
                    digest: Digest::of(b"source"),
                    byte_length: 6,
                },
            },
            sequence,
            observed_at_ms: NOW,
            kind: ObservationKind::Usage {
                usage: Box::new(Usage {
                    call: ProviderCall {
                        provider: source.policy.provider.clone(),
                        account_reference: source.account_claim.clone(),
                        model: text("model"),
                        native_call_id: text("native-call"),
                    },
                    basis: CounterBasis::PerCall,
                    counters: Counters {
                        input: Counter::Known { value: input },
                        output: Counter::Known { value: 4 },
                        cache_read: Counter::Known { value: 0 },
                        cache_write: Counter::Known { value: 0 },
                        reasoning: Counter::Known { value: 0 },
                    },
                    category_semantics: CategorySemantics::DeclaredDisjoint {
                        policy: source.policy.parsers[0].category_claim.clone(),
                    },
                    cost: Cost::Unknown {
                        reason: UnknownReason::NotReported,
                    },
                    completeness: Completeness::ReportedComplete,
                    correction_of: None,
                }),
            },
        },
    )
    .unwrap()
}
fn packet(f: &Fixture, events: Vec<ObservationRecord>) -> Packet {
    Packet::seal(
        Uuid::new_v4(),
        Message::Batch(ObservationBatch {
            node_id: f.node.node_id,
            boot_id: f.node.boot_id,
            first_sequence: events.first().unwrap().body.sequence,
            last_sequence: events.last().unwrap().body.sequence,
            events,
        }),
    )
    .unwrap()
}
async fn projection(f: &Fixture, s: &OwnerSource) -> Projection {
    let v: Value =
        sqlx::query_scalar("SELECT projection FROM observation_projections WHERE lineage_id=$1")
            .bind(s.binding.lineage_id)
            .fetch_one(&f.db.pool().await)
            .await
            .unwrap();
    serde_json::from_value(v).unwrap()
}
async fn counts(f: &Fixture) -> (i64, i64) {
    sqlx::query_as("SELECT through_sequence,event_count FROM observation_nodes WHERE node_id=$1")
        .bind(f.node.node_id)
        .fetch_one(&f.db.pool().await)
        .await
        .unwrap()
}
#[tokio::test]
async fn exact_retries_corrections_and_history_replay_never_double_charge() {
    let (f, s) = setup().await;
    let first = event(&s, 1, 10);
    let p = packet(&f, vec![first.clone()]);
    let receipt = observations::ingest(&f.state, f.node.node_id, &p)
        .await
        .unwrap();
    receipt.validate_against(&p).unwrap();
    assert_eq!(
        observations::ingest(&f.state, f.node.node_id, &p)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(counts(&f).await, (1, 1));
    assert_eq!(projection(&f, &s).await.known_subtotals[0], 10);
    let repeat = packet(&f, vec![event(&s, 2, 10)]);
    observations::ingest(&f.state, f.node.node_id, &repeat)
        .await
        .unwrap();
    assert_eq!(projection(&f, &s).await.canonical_calls, 1);
    let mut correction = event(&s, 3, 7).body;
    if let ObservationKind::Usage { usage } = &mut correction.kind {
        usage.correction_of = Some(EventRef {
            event_id: first.event_id,
            digest: first.body_digest.clone(),
        });
    }
    let corrected = ObservationRecord::seal(Uuid::new_v4(), correction).unwrap();
    observations::ingest(&f.state, f.node.node_id, &packet(&f, vec![corrected]))
        .await
        .unwrap();
    assert_eq!(projection(&f, &s).await.known_subtotals[0], 7);
    assert_eq!(counts(&f).await, (3, 3));
    let old: Value = sqlx::query_scalar("SELECT record FROM observation_events WHERE event_id=$1")
        .bind(first.event_id)
        .fetch_one(&f.db.pool().await)
        .await
        .unwrap();
    assert_eq!(old, json!(first));
    let changed = Packet::seal(p.body.message_id, repeat.body.message).unwrap();
    assert!(matches!(
        observations::ingest(&f.state, f.node.node_id, &changed).await,
        Err(observations::Error::Conflict)
    ));
    drop(f);
}
#[tokio::test]
async fn atomic_batch_refusal_does_not_advance_cursor_or_partial_totals() {
    let (f, s) = setup().await;
    let mut foreign = event(&s, 2, 12).body;
    foreign.provenance.source_id = Uuid::new_v4();
    let p = packet(
        &f,
        vec![
            event(&s, 1, 10),
            ObservationRecord::seal(Uuid::new_v4(), foreign).unwrap(),
        ],
    );
    assert!(matches!(
        observations::ingest(&f.state, f.node.node_id, &p).await,
        Err(observations::Error::Refused)
    ));
    assert_eq!(counts(&f).await, (0, 0));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM observation_events")
        .fetch_one(&f.db.pool().await)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let skipped = packet(&f, vec![event(&s, 2, 1)]);
    assert!(matches!(
        observations::ingest(&f.state, f.node.node_id, &skipped).await,
        Err(observations::Error::Conflict)
    ));
    let p = packet(&f, vec![event(&s, 1, 10)]);
    assert!(
        observations::ingest(&f.state, Uuid::new_v4(), &p)
            .await
            .is_err()
    );
    assert_eq!(counts(&f).await, (0, 0));
    drop(f);
}
#[tokio::test]
async fn late_known_source_is_observation_only_and_revocation_fences_even_replay() {
    let (mut f, s) = setup().await;
    f.profiled.status = Status::Cancelling;
    f.profiled.cancellation_id = Some(Uuid::new_v4());
    sqlx::query("UPDATE execution_attempts SET state=$2 WHERE attempt_id=$1")
        .bind(s.binding.attempt_id)
        .bind(json!(f.profiled))
        .execute(&f.db.pool().await)
        .await
        .unwrap();
    let p = packet(&f, vec![event(&s, 1, 10)]);
    observations::ingest(&f.state, f.node.node_id, &p)
        .await
        .unwrap();
    let mut fresh = s.clone();
    observations::register_source(&f.state, &s).await.unwrap();
    fresh.source_id = Uuid::new_v4();
    assert!(
        observations::register_source(&f.state, &fresh)
            .await
            .is_err()
    );
    let (held, state): (bool, Value) =
        sqlx::query_as("SELECT holds_task,state FROM execution_attempts WHERE attempt_id=$1")
            .bind(s.binding.attempt_id)
            .fetch_one(&f.db.pool().await)
            .await
            .unwrap();
    assert!(held);
    assert_eq!(state, json!(f.profiled));
    sqlx::query("UPDATE nodes SET revoked_at=now() WHERE id=$1")
        .bind(f.node.node_id)
        .execute(&f.db.pool().await)
        .await
        .unwrap();
    assert!(matches!(
        observations::ingest(&f.state, f.node.node_id, &p).await,
        Err(observations::Error::Refused)
    ));
    assert_eq!(counts(&f).await, (1, 1));
    drop(f);
}
#[tokio::test]
async fn missing_cumulative_baseline_and_account_mismatch_are_audited_as_unknown() {
    let (f, s) = setup().await;
    let mut a = event(&s, 1, 100).body;
    if let ObservationKind::Usage { usage } = &mut a.kind {
        usage.basis = CounterBasis::Cumulative {
            counter_id: text("total"),
            counter_generation: 1,
        };
    }
    let mut b = event(&s, 2, 10).body;
    if let ObservationKind::Usage { usage } = &mut b.kind {
        usage.call.account_reference = text("foreign-account");
    }
    let p = packet(
        &f,
        vec![
            ObservationRecord::seal(Uuid::new_v4(), a).unwrap(),
            ObservationRecord::seal(Uuid::new_v4(), b).unwrap(),
        ],
    );
    observations::ingest(&f.state, f.node.node_id, &p)
        .await
        .unwrap();
    let projection = projection(&f, &s).await;
    assert_eq!(projection.canonical_calls, 0);
    assert_eq!(projection.unresolved_events, 2);
    assert!(matches!(
        projection.counters().unwrap().input,
        Counter::Unknown { .. }
    ));
    assert_eq!(counts(&f).await, (2, 2));
    drop(f);
}
#[tokio::test]
async fn quota_and_database_failure_roll_back_all_accounting_effects() {
    let (f, s) = setup().await;
    let pool = f.db.pool().await;
    sqlx::query("UPDATE observation_nodes SET retained_bytes=134217728 WHERE node_id=$1")
        .bind(f.node.node_id)
        .execute(&pool)
        .await
        .unwrap();
    let p = packet(&f, vec![event(&s, 1, 10)]);
    assert!(matches!(
        observations::ingest(&f.state, f.node.node_id, &p).await,
        Err(observations::Error::Capacity)
    ));
    assert_eq!(counts(&f).await, (0, 0));
    sqlx::query("UPDATE observation_nodes SET retained_bytes=1000 WHERE node_id=$1")
        .bind(f.node.node_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION observation_test_failure() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected persistence failure'; END $$; CREATE TRIGGER fail_receipt BEFORE INSERT ON observation_packets FOR EACH ROW EXECUTE FUNCTION observation_test_failure();").execute(&pool).await.unwrap();
    assert!(
        observations::ingest(&f.state, f.node.node_id, &p)
            .await
            .is_err()
    );
    assert_eq!(counts(&f).await, (0, 0));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM observation_facts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER fail_receipt ON observation_packets")
        .execute(&pool)
        .await
        .unwrap();
    observations::ingest(&f.state, f.node.node_id, &p)
        .await
        .unwrap();
    assert!(
        sqlx::query("DELETE FROM observation_events")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE observation_sources SET generation=2")
            .execute(&pool)
            .await
            .is_err()
    );
    drop(f);
}

#[tokio::test]
async fn concurrent_exact_delivery_returns_one_receipt_and_one_charge() {
    let (f, s) = setup().await;
    let p = packet(&f, vec![event(&s, 1, 11)]);
    let (a, b) = tokio::join!(
        observations::ingest(&f.state, f.node.node_id, &p),
        observations::ingest(&f.state, f.node.node_id, &p)
    );
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(counts(&f).await, (1, 1));
    assert_eq!(projection(&f, &s).await.known_subtotals[0], 11);
    drop(f);
}
#[tokio::test]
async fn worker_route_requires_node_credentials_and_rejects_wrong_packet_scope() {
    let (f, s) = setup().await;
    let p = packet(&f, vec![event(&s, 1, 10)]);
    let router = observations::api::routes(f.state.clone()).with_state(f.state.clone());
    let request = |credential: &str, body: Vec<u8>| {
        Request::builder()
            .method("POST")
            .uri("/v1/worker/observations/batch")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {credential}"))
            .body(Body::from(body))
            .unwrap()
    };
    let bytes = serde_json::to_vec(&p).unwrap();
    let r = router
        .clone()
        .oneshot(request("aqa_agent-token", bytes.clone()))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let r = router
        .clone()
        .oneshot(request("aqn_wrong", bytes.clone()))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(counts(&f).await, (0, 0));
    let mut foreign = event(&s, 1, 10).body;
    let foreign_node = Uuid::new_v4();
    if let SessionIdentity::Remote { node_id, .. } = &mut foreign.binding.session {
        *node_id = foreign_node;
    }
    let foreign = ObservationRecord::seal(Uuid::new_v4(), foreign).unwrap();
    let foreign = Packet::seal(
        Uuid::new_v4(),
        Message::Batch(ObservationBatch {
            node_id: foreign_node,
            boot_id: f.node.boot_id,
            first_sequence: 1,
            last_sequence: 1,
            events: vec![foreign],
        }),
    )
    .unwrap();
    let r = router
        .clone()
        .oneshot(request(
            &f.credential,
            serde_json::to_vec(&foreign).unwrap(),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    assert_eq!(counts(&f).await, (0, 0));
    let r = router
        .clone()
        .oneshot(request(&f.credential, bytes.clone()))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(r.headers()["cache-control"], "no-store");
    let got: Receipt =
        serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 2048).await.unwrap()).unwrap();
    got.validate_against(&p).unwrap();
    let r = router
        .clone()
        .oneshot(request(&f.credential, vec![b' '; MAX_BATCH_BYTES + 1]))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::PAYLOAD_TOO_LARGE);
    // Cookie/human identities never authorize a node transport.
    let req = Request::builder()
        .method("POST")
        .uri("/v1/worker/observations/batch")
        .header("content-type", "application/json")
        .header("cookie", "session=human")
        .body(Body::from(bytes))
        .unwrap();
    assert_eq!(
        router.oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(counts(&f).await, (1, 1));
    drop(f);
}
