//! Synthetic actual profiled state, never a fake legacy ExecutionSpec. These
//! fixtures qualify authority/SQL fences, not a provider, secret lease or launch.
use aq_server::{
    AppState,
    config::Config,
    execution::{
        authority::{self, AuthorityError, Kind, Policy},
        profiled::{Effects, ProfiledState, Retention, Status, Stop},
    },
    identity::verifier_from_config,
    store::NewNode,
    test_support::TestDb,
    tokens,
};
use aq_session_protocol::{
    ArtifactRef, Digest, ProfiledGrant, ProfiledOffer, SecretRef, SessionIdentity, Text,
    canonical_digest,
};
use aq_worker_protocol::{
    AttemptRef, GraphBinding, NodeIdentity, ObjectAlgorithm, ObjectId, RegisterRequest,
    SourceBinding,
};
use chrono::{Duration, TimeZone, Utc};
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
use uuid::Uuid;
const NOW: u64 = 1_000_000;
fn artifact(id: &str, bytes: &[u8]) -> ArtifactRef {
    ArtifactRef {
        id: Text::new(id).unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(bytes),
    }
}
struct Fixture {
    db: TestDb,
    state: AppState,
    node: NodeIdentity,
    profiled: ProfiledState,
    selection: aq_selection::ResolvedSelection,
    decision: Uuid,
    actor: Uuid,
    credential: String,
    time: Arc<AtomicI64>,
}
async fn fixture() -> Fixture {
    let db = TestDb::create().await;
    let store = db.store().await;
    let actual = Utc.timestamp_millis_opt(NOW as i64).unwrap();
    let owner = store
        .bootstrap_owner("owner@localhost", "dev", "owner@localhost", actual)
        .await
        .unwrap()
        .unwrap();
    let credential = tokens::new_node_credential();
    store
        .create_enrollment(
            "profiled-fixture",
            owner.id,
            actual,
            actual + Duration::minutes(5),
        )
        .await
        .unwrap();
    let node = store
        .redeem_enrollment(
            "profiled-fixture",
            NewNode {
                name: "profiled-fixture".into(),
                credential_hash: tokens::verifier(&credential),
                worker_version: "0.1.0".into(),
                protocol_version: 1,
            },
            actual,
        )
        .await
        .unwrap()
        .unwrap();
    let identity = NodeIdentity {
        node_id: node.id,
        boot_id: Uuid::new_v4(),
    };
    store
        .execution_register(&RegisterRequest {
            identity: identity.clone(),
            operation_id: Uuid::new_v4(),
            protocol_version: 2,
            worker_version: "0.1.0".into(),
            capabilities: vec![aq_worker_protocol::FAKE_HARNESS_CAPABILITY.into()],
        })
        .await
        .unwrap();
    let raw: Value = serde_json::from_str(include_str!("fixtures/selection.json")).unwrap();
    let catalog = aq_profiles::ValidatedCatalog::from_artifact(
        raw["catalog_artifact_json"].as_str().unwrap().as_bytes(),
    )
    .unwrap();
    let mut input: aq_selection::SelectionInputs =
        serde_json::from_value(raw["inputs"].clone()).unwrap();
    input.project.target = aq_selection::ExecutionTarget::Worker {
        node_id: node.id,
        boot_id: identity.boot_id,
    };
    input.node.target = input.project.target.clone();
    input.project.expires_at_ms = NOW + 30_000;
    input.node.observed_at_ms = NOW - 10;
    input.node.expires_at_ms = NOW + 30_000;
    input.budget.observed_at_ms = NOW - 10;
    input.budget.expires_at_ms = NOW + 30_000;
    input.now_ms = NOW;
    for c in &mut input.node.credential_bindings {
        c.observed_at_ms = NOW - 10;
        c.expires_at_ms = NOW + 30_000;
    }
    for o in &mut input.observations {
        o.received_at_ms = NOW - 10;
        o.expires_at_ms = NOW + 30_000;
        o.cooldown_until_ms = NOW - 10;
    }
    let selected = aq_selection::select(&catalog, &input)
        .unwrap()
        .resolved
        .unwrap();
    let shared: Value = serde_json::from_str(include_str!(
        "../../../vendor/aq-session-protocol-0.1.0/artifacts/fixtures.json"
    ))
    .unwrap();
    let mut offer: ProfiledOffer = serde_json::from_value(
        shared["valid_documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["kind"] == "launch")
            .unwrap()["body"]
            .clone(),
    )
    .unwrap();
    let session = SessionIdentity::Remote {
        node_id: node.id,
        boot_id: identity.boot_id,
        attempt_id: Uuid::new_v4(),
        assignment_generation: 1,
        session_id: Uuid::new_v4(),
    };
    let SessionIdentity::Remote { attempt_id, .. } = session else {
        unreachable!()
    };
    offer.binding.session = session.clone();
    offer.binding.attempt_id = attempt_id;
    offer.binding.offer_id = Uuid::new_v4();
    offer.manifest.bindings.session = session;
    offer.manifest.bindings.attempt_id = attempt_id;
    offer.manifest.bindings.project_id = selected.lineage.project_id;
    offer.manifest.bindings.graph_id = selected.lineage.workspace_id;
    offer.manifest.bindings.task_id = selected.lineage.task_id;
    offer.manifest.catalog = ArtifactRef {
        id: Text::new("profile-catalog").unwrap(),
        version: Text::new("1").unwrap(),
        digest: selected.catalog_digest.clone(),
    };
    offer.manifest.bindings.project_revision = selected.project_revision;
    let choice = &selected.choice;
    offer.manifest.model.provider = Text::new(&choice.provider).unwrap();
    offer.manifest.model.model = Text::new(&choice.model).unwrap();
    offer.manifest.model.reasoning = Text::new(&choice.native_reasoning).unwrap();
    offer.manifest.model.max_output_tokens = choice.max_output_tokens;
    offer.manifest.harness.harness = Text::new(&choice.harness).unwrap();
    offer.manifest.harness.cli_version = Text::new(&choice.cli_version).unwrap();
    offer.manifest.harness.adapter = choice.adapter.clone();
    offer.manifest.harness.environment = choice.environment.clone();
    offer.manifest.resources.memory_bytes = choice.resources.memory_bytes;
    offer.manifest.resources.cpu_millis = choice.resources.cpu_millis;
    offer.manifest.resources.process_limit = choice.resources.process_limit;
    offer.manifest.resources.wall_time_ms = choice.resources.wall_time_ms;
    offer.manifest.context.reserved_output_tokens = choice.max_output_tokens;
    offer.manifest.credentials = vec![SecretRef {
        credential_profile: choice.credential_profile.clone(),
        purpose: Text::new("model-authentication").unwrap(),
    }];
    let graph_artifact = artifact("graph-profile", b"synthetic-frozen-graph-profile");
    offer.manifest.bindings.graph_access_profile = graph_artifact.clone();
    offer.binding.graph_access_profile_digest = graph_artifact.digest.clone();
    let spec = json!({"schema_version":1,"kind":"synthetic_profiled_authority_fixture","task_id":selected.lineage.task_id});
    offer.binding.execution_spec_digest = aq_selection::digest(&spec).unwrap();
    offer.binding.launch_manifest_digest = canonical_digest(&offer.manifest).unwrap();
    let source = SourceBinding {
        repository_id: Uuid::new_v4(),
        binding_generation: 1,
        logical_identity: "fixture/repository".into(),
        base: ObjectId {
            algorithm: ObjectAlgorithm::Sha1,
            hex: "a".repeat(40),
        },
    };
    let graph = GraphBinding {
        workspace_id: selected.lineage.workspace_id,
        profile_id: graph_artifact.id.as_str().into(),
        profile_revision: graph_artifact.version.as_str().into(),
        profile_digest: graph_artifact.digest.as_str().into(),
        context_digest: Digest::of(b"graph-context").as_str().into(),
    };
    let state_projection = ProfiledState {
        schema_version: 1,
        offer,
        status: Status::Offered,
        grant: None,
        execution_deadline_ms: None,
        source,
        graph,
        cancellation_id: None,
        revoked: false,
        stop: Stop::Unknown,
        retention: Retention::Pending,
        effects: Effects::Unknown,
    };
    state_projection.validate().unwrap();
    let cfg: Config = toml::from_str(
        r#"format_version=1
environment="development"
[server]
admin_bind="127.0.0.1:0"
worker_bind="127.0.0.1:0"
public_origin="http://127.0.0.1:8080"
[auth]
mode="dev"
dev_email="owner@localhost"
[bootstrap]
owner_email="owner@localhost"
enabled=true
"#,
    )
    .unwrap();
    let time = Arc::new(AtomicI64::new(NOW as i64));
    let clock = time.clone();
    let state = AppState {
        store,
        identity: verifier_from_config(&cfg.auth, reqwest::Client::new()),
        config: Arc::new(cfg),
        clock: Arc::new(move || {
            Utc.timestamp_millis_opt(clock.load(Ordering::SeqCst))
                .unwrap()
        }),
        agent_access: None,
        source_retention: None,
    };
    Fixture {
        db,
        state,
        node: identity,
        profiled: state_projection,
        selection: selected,
        decision: Uuid::new_v4(),
        actor: owner.id,
        credential,
        time,
    }
}
async fn insert(f: &Fixture) -> Uuid {
    let pool = f.db.pool().await;
    let p = &f.profiled;
    let binding = &p.offer.binding;
    let SessionIdentity::Remote { session_id, .. } = binding.session else {
        unreachable!()
    };
    sqlx::query("INSERT INTO intelligence_decisions(decision_id,actor_id,operation_id,payload_digest,decision) VALUES($1,$2,$3,$4,$5)").bind(f.decision).bind(f.actor).bind(Uuid::new_v4()).bind(Digest::of(b"synthetic-request").as_str()).bind(json!({"outcome":{"resolved":f.selection}})).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO execution_attempts(attempt_id,task_id,node_id,offer_id,session_id,kind,state,execution_deadline_ms) VALUES($1,$2,$3,$4,$5,'profiled_v1',$6,$7)").bind(binding.attempt_id).bind(f.selection.lineage.task_id).bind(f.node.node_id).bind(binding.offer_id).bind(session_id).bind(json!(p)).bind(p.execution_deadline_ms.map(|v|v as i64)).execute(&pool).await.unwrap();
    let spec = json!({"schema_version":1,"kind":"synthetic_profiled_authority_fixture","task_id":f.selection.lineage.task_id});
    // A real admission stores real owner observations. These synthetic table rows
    // exercise the same concrete decoder and manifest/selection digest checks.
    let project = json!({"schema_version":1,"project_id":f.selection.lineage.project_id,"name":"Synthetic authority fixture","enabled":true,"repository_id":p.source.repository_id,"repository_binding_generation":1,"repository_adapter_reference":"aq.git.configured","graph_adapter_reference":"aq.graph.configured","graph_workspace_id":f.selection.lineage.workspace_id,"catalog_digest":f.selection.catalog_digest,"allowed_agent_profiles":["implementer@2"],"allowed_intelligence_profiles":["engineering@2"],"allowed_execution_profiles":["confined-development@2"],"allowed_node_ids":[f.node.node_id],"allowed_worker_groups":[],"allowed_environments":["confined-worktree"],"policy":{"identity":"synthetic-policy","revision":"1","digest":Digest::of(b"synthetic-policy")},"context":[],"ceilings":{"permissions":[],"context_bytes":8192,"wall_time_ms":60000,"memory_bytes":268435456,"cpu_millis":60000,"process_limit":8,"cost_limit_microunits":null,"currency":null,"require_hard_cost_enforcement":false}});
    let typed: aq_server::project_bindings::domain::ProjectBinding =
        serde_json::from_value(project.clone()).unwrap();
    typed.validate(&Default::default()).unwrap();
    let observed = aq_server::project_bindings::domain::BindingObservation {
        repository_id: p.source.repository_id,
        graph_workspace_id: f.selection.lineage.workspace_id,
        repository_adapter_reference: "aq.git.configured".into(),
        graph_adapter_reference: "aq.graph.configured".into(),
        repository: aq_server::project_bindings::domain::ExternalStatus::Verified,
        graph: aq_server::project_bindings::domain::ExternalStatus::Verified,
        repository_generation: Some(1),
        observed_at_ms: NOW,
        expires_at_ms: NOW + 30_000,
    };
    sqlx::query("INSERT INTO project_execution_bindings(project_id,revision,binding,validated_at_ms,binding_observation) VALUES($1,1,$2,$3,$4)").bind(f.selection.lineage.project_id).bind(project).bind(NOW as i64).bind(json!(observed)).execute(&pool).await.unwrap();
    let observation = aq_admission::SourceAdmission {
        schema_version: 1,
        project_id: f.selection.lineage.project_id,
        project_revision: 1,
        repository_id: p.source.repository_id,
        repository_binding_generation: 1,
        policy: artifact("synthetic-source-policy", b"immutable-base-policy"),
        selection: aq_admission::SourceSelection::Immutable {
            object: aq_admission::ObjectId {
                format: aq_admission::ObjectFormat::Sha1,
                hex: p.source.base.hex.clone(),
            },
        },
        resolved_object: aq_admission::ObjectId {
            format: aq_admission::ObjectFormat::Sha1,
            hex: p.source.base.hex.clone(),
        },
        observed_at_ms: NOW,
        expires_at_ms: NOW + 30_000,
    };
    observation.validate().unwrap();
    let source_observation = Uuid::new_v4();
    sqlx::query("INSERT INTO admission_source_observations(observation_id,project_id,source_policy_revision,observation,observed_at_ms,expires_at_ms) VALUES($1,$2,1,$3,$4,$5)").bind(source_observation).bind(f.selection.lineage.project_id).bind(json!(observation)).bind(NOW as i64).bind((NOW+30_000) as i64).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO profiled_launches(attempt_id,decision_id,source_observation_id,launch_binding,launch_manifest,manifest_digest,execution_spec,execution_spec_digest,graph_task_observation,source_binding,offer_expires_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)").bind(binding.attempt_id).bind(f.decision).bind(source_observation).bind(json!(binding)).bind(json!(p.offer.manifest)).bind(binding.launch_manifest_digest.as_str()).bind(spec).bind(binding.execution_spec_digest.as_str()).bind(json!({"kind":"synthetic-readiness-observation","task_id":f.selection.lineage.task_id,"version":1,"content_revision":1,"observed_at_ms":NOW,"expires_at_ms":NOW+30_000})).bind(json!(p.source)).bind((NOW+30_000) as i64).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO intelligence_attempt_selections(attempt_id,selection_digest,selection) VALUES($1,$2,$3)").bind(binding.attempt_id).bind(aq_selection::digest(&f.selection).unwrap().as_str()).bind(json!(f.selection)).execute(&pool).await.unwrap();
    binding.attempt_id
}
#[tokio::test]
async fn common_task_claim_and_legacy_decoders_cannot_cross_profiled_kind() {
    let f = fixture().await;
    let attempt = insert(&f).await;
    let legacy = aq_worker_protocol::Offer {
        offer_id: Uuid::new_v4(),
        identity: f.node.clone(),
        attempt: AttemptRef {
            attempt_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            generation: 1,
        },
        spec: aq_worker_protocol::ExecutionSpec {
            project_id: f.selection.lineage.project_id,
            harness: aq_worker_protocol::FAKE_HARNESS_CAPABILITY.into(),
            resolved_spec_digest: String::new(),
            fixture_output: "legacy-compatible".into(),
            resources: aq_worker_protocol::Resources {
                slots: 1,
                memory_bytes: 100,
                cpu_millis: 100,
                disk_bytes: 100,
            },
            required_capabilities: vec![aq_worker_protocol::FAKE_HARNESS_CAPABILITY.into()],
            bindings: None,
        }
        .seal(),
        offer_expires_at_ms: NOW + 10000,
        reservation_ttl_ms: 10000,
    };
    assert!(matches!(
        f.state
            .store
            .execution_offer(f.selection.lineage.task_id, &legacy)
            .await,
        Err(aq_server::execution::Error::Refused(
            aq_worker_protocol::ErrorCode::IdConflict,
            _
        ))
    ));
    assert!(matches!(
        f.state.store.execution_get(attempt).await,
        Err(aq_server::execution::Error::Refused(
            aq_worker_protocol::ErrorCode::NotFound,
            _
        ))
    ));
    let poll = f
        .state
        .store
        .execution_poll(
            &aq_worker_protocol::PollRequest {
                identity: f.node.clone(),
                operation_id: Uuid::new_v4(),
                inventory_sequence: 1,
                available: legacy.spec.resources.clone(),
                max_items: 8,
            },
            NOW,
        )
        .await
        .unwrap();
    assert!(poll.offers.is_empty());
    let mut replacement = f.node.clone();
    replacement.boot_id = Uuid::new_v4();
    let registration = RegisterRequest {
        identity: replacement,
        operation_id: Uuid::new_v4(),
        protocol_version: 2,
        worker_version: "0.1.0".into(),
        capabilities: vec![aq_worker_protocol::FAKE_HARNESS_CAPABILITY.into()],
    };
    assert!(matches!(
        f.state
            .store
            .execution_replace_incarnation(&registration)
            .await,
        Err(aq_server::execution::Error::Refused(
            aq_worker_protocol::ErrorCode::CapacityUnavailable,
            _
        ))
    ));
    let pool = f.db.pool().await;
    let held: bool =
        sqlx::query_scalar("SELECT holds_task FROM execution_attempts WHERE attempt_id=$1")
            .bind(attempt)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(held);
}
#[tokio::test]
async fn shared_view_fences_exact_seal_identity_live_time_and_revision_semantics() {
    let mut f = fixture().await;
    f.profiled.status = Status::Granted;
    f.profiled.execution_deadline_ms = Some(NOW + 60_000);
    f.profiled.grant = Some(ProfiledGrant {
        binding: f.profiled.offer.binding.clone(),
        grant_id: Uuid::new_v4(),
        expires_at_ms: NOW + 5000,
    });
    f.profiled.stop = Stop::Running;
    let attempt = insert(&f).await;
    let pool = f.db.pool().await;
    let mut tx = pool.begin().await.unwrap();
    let first = authority::load_locked(
        &f.state,
        &mut tx,
        &f.profiled.offer.binding.session,
        Policy::TerminalActive,
    )
    .await
    .unwrap();
    assert_eq!(first.kind, Kind::ProfiledV1);
    assert_eq!(
        first.grant_id,
        f.profiled.grant.as_ref().map(|g| g.grant_id)
    );
    assert_eq!(first.execution_deadline_ms, Some(NOW + 60_000));
    tx.rollback().await.unwrap();
    f.profiled.grant.as_mut().unwrap().expires_at_ms = NOW + 10_000;
    sqlx::query("UPDATE execution_attempts SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(f.profiled))
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let renewed = authority::load_locked(
        &f.state,
        &mut tx,
        &f.profiled.offer.binding.session,
        Policy::TerminalActive,
    )
    .await
    .unwrap();
    assert_eq!(renewed.authority_revision, first.authority_revision);
    assert!(renewed.record_revision > first.record_revision);
    assert!(renewed.grant_revision > first.grant_revision);
    tx.rollback().await.unwrap();
    let mut uncertain = f.profiled.clone();
    uncertain.stop = Stop::Unknown;
    sqlx::query("UPDATE execution_attempts SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(uncertain))
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        authority::load_locked(
            &f.state,
            &mut tx,
            &f.profiled.offer.binding.session,
            Policy::TerminalActive
        )
        .await,
        Err(AuthorityError::Unsupported)
    ));
    tx.rollback().await.unwrap();
    sqlx::query("UPDATE execution_attempts SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(f.profiled))
        .execute(&pool)
        .await
        .unwrap();
    f.time.store((NOW + 10_000) as i64, Ordering::SeqCst);
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        authority::load_locked(
            &f.state,
            &mut tx,
            &f.profiled.offer.binding.session,
            Policy::TerminalActive
        )
        .await,
        Err(AuthorityError::Expired)
    ));
    tx.rollback().await.unwrap();
    f.time.store(NOW as i64, Ordering::SeqCst);
    let mut foreign = f.profiled.offer.binding.session.clone();
    if let SessionIdentity::Remote { boot_id, .. } = &mut foreign {
        *boot_id = Uuid::new_v4();
    }
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        authority::load_locked(&f.state, &mut tx, &foreign, Policy::TerminalActive).await,
        Err(AuthorityError::Identity)
    ));
    tx.rollback().await.unwrap();
    f.profiled.status = Status::Cancelling;
    f.profiled.cancellation_id = Some(Uuid::new_v4());
    sqlx::query("UPDATE execution_attempts SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(f.profiled))
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        authority::load_locked(
            &f.state,
            &mut tx,
            &f.profiled.offer.binding.session,
            Policy::TerminalActive
        )
        .await,
        Err(AuthorityError::Revoked)
    ));
    tx.rollback().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let historical = authority::load_locked(
        &f.state,
        &mut tx,
        &f.profiled.offer.binding.session,
        Policy::TerminalHistorical,
    )
    .await
    .unwrap();
    assert!(historical.authority_revision > renewed.authority_revision);
    tx.rollback().await.unwrap();
}
#[tokio::test]
async fn migration_freezes_common_identity_profiled_boot_generation_deadline_and_immutable_seal() {
    let f = fixture().await;
    let attempt = insert(&f).await;
    let pool = f.db.pool().await;
    for query in [
        "UPDATE execution_attempts SET kind='legacy_fixture' WHERE attempt_id=$1",
        "UPDATE execution_attempts SET session_id=gen_random_uuid() WHERE attempt_id=$1",
        "UPDATE execution_attempts SET state=jsonb_set(state,'{offer,binding,assignment_generation}','2') WHERE attempt_id=$1",
    ] {
        assert!(
            sqlx::query(query)
                .bind(attempt)
                .execute(&pool)
                .await
                .is_err()
        );
    }
    assert!(
        sqlx::query("UPDATE profiled_launches SET manifest_digest=$2 WHERE attempt_id=$1")
            .bind(attempt)
            .bind(Digest::of(b"changed").as_str())
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::query("UPDATE execution_attempts SET execution_deadline_ms=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind((NOW + 5000) as i64)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE execution_attempts SET execution_deadline_ms=$2 WHERE attempt_id=$1")
            .bind(attempt)
            .bind((NOW + 6000) as i64)
            .execute(&pool)
            .await
            .is_err()
    );
    let row = sqlx::query("SELECT kind,state FROM execution_attempts WHERE attempt_id=$1")
        .bind(attempt)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("kind"), "profiled_v1");
    assert_eq!(row.get::<Value, _>("state"), json!(f.profiled));
}
#[tokio::test]
async fn actual_profiled_source_adapter_accepts_scoped_late_retention_and_refuses_foreign_spec() {
    use aq_server::source_retention::{Runtime, SourceRetentionConfig};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let mut f = fixture().await;
    let attempt = insert(&f).await;
    let dir = tempfile::tempdir().unwrap();
    f.state.source_retention = Some(Arc::new(
        Runtime::new(SourceRetentionConfig {
            staging_root: dir.path().join("staging"),
            git_root: dir.path().join("git"),
            git_socket: dir.path().join("git.sock"),
            max_staging_bytes: 2 * 1024 * 1024,
            max_bundle_bytes: 4096,
            bulk_concurrency: 2,
            incomplete_ttl_secs: 1,
        })
        .unwrap(),
    ));
    let app = aq_server::build_worker_router(f.state.clone());
    let manifest = aq_worker_protocol::TransferManifest {
        transfer_id: Uuid::new_v4(),
        role: aq_worker_protocol::SourceRole::Checkpoint,
        checkpoint: ObjectId {
            algorithm: ObjectAlgorithm::Sha1,
            hex: "b".repeat(40),
        },
        attempt: match &f.profiled.offer.binding.session {
            SessionIdentity::Remote {
                attempt_id,
                session_id,
                assignment_generation,
                ..
            } => AttemptRef {
                attempt_id: *attempt_id,
                session_id: *session_id,
                generation: *assignment_generation,
            },
            _ => unreachable!(),
        },
        resolved_spec_digest: f
            .profiled
            .offer
            .binding
            .execution_spec_digest
            .as_str()
            .into(),
        source: f.profiled.source.clone(),
        artifact_id: Uuid::new_v4(),
        manifest_digest: Digest::of(b"synthetic-source-manifest").as_str().into(),
        bundle_digest: None,
        manifest_bytes: 128,
        bundle_bytes: 0,
    };
    let body = json!({"identity":f.node,"operation_id":Uuid::new_v4(),"manifest":manifest});
    let call = |body: Value| {
        let app = app.clone();
        let token = f.credential.clone();
        async move {
            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/execution/sources/begin")
                        .header("authorization", format!("Bearer {token}"))
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice::<Value>(&bytes).unwrap())
        }
    };
    let (status, first) = call(body.clone()).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let mut foreign = body.clone();
    foreign["operation_id"] = json!(Uuid::new_v4());
    foreign["manifest"]["resolved_spec_digest"] = json!(Digest::of(b"foreign-spec"));
    assert_eq!(call(foreign).await.0, StatusCode::FORBIDDEN);
    f.profiled.status = Status::Cancelling;
    f.profiled.cancellation_id = Some(Uuid::new_v4());
    sqlx::query("UPDATE execution_attempts SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(f.profiled))
        .execute(&f.db.pool().await)
        .await
        .unwrap();
    f.time.store((NOW + 60_000) as i64, Ordering::SeqCst);
    let (status, replay) = call(body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay, first);
}
#[tokio::test]
async fn authority_expiry_is_sampled_after_an_observed_node_lock_wait() {
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
    let pool = f.db.pool().await;
    let mut held = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE")
        .bind(f.node.node_id)
        .execute(&mut *held)
        .await
        .unwrap();
    let worker_pool = pool.clone();
    let state = f.state.clone();
    let expected = f.profiled.offer.binding.session.clone();
    let (pid_tx, pid_rx) = tokio::sync::oneshot::channel();
    let waiting = tokio::spawn(async move {
        let mut tx = worker_pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        pid_tx.send(pid).unwrap();
        let result =
            authority::load_locked(&state, &mut tx, &expected, Policy::TerminalActive).await;
        tx.rollback().await.unwrap();
        result
    });
    let pid = pid_rx.await.unwrap();
    let observer = f.db.pool().await;
    let mut blocked = false;
    for _ in 0..100 {
        let lock: bool = sqlx::query_scalar(
            "SELECT COALESCE(wait_event_type='Lock',false) FROM pg_stat_activity WHERE pid=$1",
        )
        .bind(pid)
        .fetch_one(&observer)
        .await
        .unwrap();
        if lock {
            blocked = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(
        blocked,
        "actual node lock wait must be observed before clock advance"
    );
    f.time.store((NOW + 5000) as i64, Ordering::SeqCst);
    held.rollback().await.unwrap();
    assert!(matches!(
        waiting.await.unwrap(),
        Err(AuthorityError::Expired)
    ));
}
#[tokio::test]
async fn actual_profiled_graph_adapter_issues_exact_api01_binding_and_revokes_on_current_authority()
{
    use aq_server::agent_access::{AgentAccessConfig, Runtime};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use std::os::unix::fs::PermissionsExt;
    use tower::ServiceExt;
    let mut f = fixture().await;
    let api: Value = serde_json::from_str(include_str!(
        "../../../protocol/agent-access/v1/fixtures/base.json"
    ))
    .unwrap();
    let profile = api["context"]["profile"].clone();
    let context = api["context"]["context"].clone();
    let profile_digest = aq_agent_api::digest(&profile).unwrap();
    let context_digest = aq_agent_api::digest(&context).unwrap();
    f.profiled.graph.profile_id = profile["profile_id"].as_str().unwrap().into();
    f.profiled.graph.profile_revision = profile["revision"].as_str().unwrap().into();
    f.profiled.graph.profile_digest = profile_digest.clone();
    f.profiled.graph.context_digest = context_digest.clone();
    f.profiled.offer.manifest.bindings.graph_access_profile = ArtifactRef {
        id: Text::new(&f.profiled.graph.profile_id).unwrap(),
        version: Text::new(&f.profiled.graph.profile_revision).unwrap(),
        digest: serde_json::from_value(json!(profile_digest)).unwrap(),
    };
    f.profiled.offer.binding.graph_access_profile_digest = f
        .profiled
        .offer
        .manifest
        .bindings
        .graph_access_profile
        .digest
        .clone();
    f.profiled.offer.binding.launch_manifest_digest =
        canonical_digest(&f.profiled.offer.manifest).unwrap();
    f.profiled.status = Status::Granted;
    f.profiled.stop = Stop::Running;
    f.profiled.execution_deadline_ms = Some(NOW + 60_000);
    f.profiled.grant = Some(ProfiledGrant {
        binding: f.profiled.offer.binding.clone(),
        grant_id: Uuid::new_v4(),
        expires_at_ms: NOW + 5000,
    });
    let attempt = insert(&f).await;
    let SessionIdentity::Remote {
        session_id,
        assignment_generation,
        ..
    } = f.profiled.offer.binding.session
    else {
        unreachable!()
    };
    let binding = json!({"task_id":f.selection.lineage.task_id,"attempt_id":attempt,"session_id":session_id,"generation":assignment_generation,"node_id":f.node.node_id,"boot_id":f.node.boot_id,"project_id":f.selection.lineage.project_id,"workspace_id":f.selection.lineage.workspace_id,"resolved_spec_digest":f.profiled.offer.binding.execution_spec_digest,"profile_digest":profile_digest,"context_digest":context_digest});
    assert!(aq_agent_api::valid("Binding", &binding));
    sqlx::query("INSERT INTO agent_launch_bindings(attempt_id,node_id,binding,profile,context) VALUES($1,$2,$3,$4,$5)").bind(attempt).bind(f.node.node_id).bind(&binding).bind(profile).bind(context).execute(&f.db.pool().await).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let key = dir.path().join("graph-key");
    std::fs::write(&key, b"synthetic-service-key-000000000000000000").unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    f.state.agent_access = Some(Arc::new(
        Runtime::new(&AgentAccessConfig {
            graph_url: "http://127.0.0.1:9/".into(),
            graph_token_file: key,
            mutation_actor_id: None,
            credential_ttl_ms: 1000,
        })
        .await
        .unwrap(),
    ));
    let app = aq_server::build_worker_router(f.state.clone());
    let issue = json!({"identity":f.node,"attempt":{"attempt_id":attempt,"session_id":session_id,"generation":assignment_generation},"expected_credential_version":0});
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v2/worker/agent-session")
                .header("authorization", format!("Bearer {}", f.credential))
                .header("content-type", "application/json")
                .header("X-AQ-Agent-API-Version", "1")
                .body(Body::from(issue.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let issued: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let token = issued["token"].as_str().unwrap();
    assert_eq!(issued["context"]["binding"], binding);
    let fetch = || {
        let app = app.clone();
        let token = token.to_owned();
        async move {
            app.oneshot(
                Request::builder()
                    .uri("/v1/agent/context")
                    .header("authorization", format!("Bearer {token}"))
                    .header("X-AQ-Agent-API-Version", "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
        }
    };
    assert_eq!(fetch().await.status(), StatusCode::OK);
    f.profiled.status = Status::Cancelling;
    f.profiled.cancellation_id = Some(Uuid::new_v4());
    sqlx::query("UPDATE execution_attempts SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(f.profiled))
        .execute(&f.db.pool().await)
        .await
        .unwrap();
    assert_eq!(fetch().await.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn database_derives_revisions_on_noop_and_bounds_uncompressed_state_and_material_verifier() {
    let f = fixture().await;
    let attempt = insert(&f).await;
    let pool = f.db.pool().await;
    let before:(i64,i64,i64)=sqlx::query_as("SELECT record_revision,authority_revision,grant_revision FROM execution_attempts WHERE attempt_id=$1").bind(attempt).fetch_one(&pool).await.unwrap();
    for value in [0i64, 999] {
        sqlx::query("UPDATE execution_attempts SET record_revision=$2,authority_revision=$2,grant_revision=$2 WHERE attempt_id=$1").bind(attempt).bind(value).execute(&pool).await.unwrap();
        let after:(i64,i64,i64)=sqlx::query_as("SELECT record_revision,authority_revision,grant_revision FROM execution_attempts WHERE attempt_id=$1").bind(attempt).fetch_one(&pool).await.unwrap();
        assert_eq!(
            before, after,
            "exact replay cannot alter server-derived revisions"
        );
    }
    let mut huge = json!(f.profiled);
    huge["padding"] = json!("x".repeat(4 * 1024 * 1024));
    assert!(
        sqlx::query("UPDATE execution_attempts SET state=$2 WHERE attempt_id=$1")
            .bind(attempt)
            .bind(huge)
            .execute(&pool)
            .await
            .is_err()
    );
    for verifier in ["a".repeat(63), "A".repeat(64), "a".repeat(65)] {
        assert!(sqlx::query("INSERT INTO admission_material_leases(lease_id,operation_id,request_digest,node_id,boot_id,prospective_attempt_id,offer_id,credential_identity,credential_revision,purpose,lease,verifier,state,issued_at_ms,expires_at_ms) VALUES($1,$2,$3,$4,$5,$6,$7,'synthetic-credential@1',1,'model-authentication',$8,$9,'pending',1000,2000)").bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(Digest::of(b"synthetic-request").as_str()).bind(f.node.node_id).bind(f.node.boot_id).bind(attempt).bind(f.profiled.offer.binding.offer_id).bind(json!({"qualification":"synthetic-column-bound-only"})).bind(verifier).execute(&pool).await.is_err());
    }
    let mut prepared = f.profiled;
    prepared.status = Status::Preparing;
    sqlx::query("UPDATE execution_attempts SET state=$2,record_revision=999,authority_revision=999,grant_revision=999 WHERE attempt_id=$1").bind(attempt).bind(json!(prepared)).execute(&pool).await.unwrap();
    let changed:(i64,i64,i64)=sqlx::query_as("SELECT record_revision,authority_revision,grant_revision FROM execution_attempts WHERE attempt_id=$1").bind(attempt).fetch_one(&pool).await.unwrap();
    assert_eq!(changed, (before.0 + 1, before.1 + 1, before.2));
}
