//! Real SQL/router fences with explicitly synthetic provider material and runtime.
mod common;
use aq_admission::{Digest, Label, MaterialLease, transport::*};
use aq_selection::{AuthorizedCredential, DeliveryCapability};
use aq_server::{
    execution::{
        authority::hot::{HotState, LaunchOwner},
        profiled::Status,
    },
    intelligence::config::{CredentialConfig, IntelligenceConfig, MaterialDeliveryMode},
    profiled_execution::{
        config::{ProfiledConfig, QualifiedRuntime},
        material_api,
        registration::{self, NativeRegistration},
    },
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use common::profiled::{Fixture, NOW, artifact, fixture, insert};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    os::unix::fs::PermissionsExt,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tower::ServiceExt;
use uuid::Uuid;
const SECRET: &[u8] = b"synthetic-private-provider-material";
struct Case {
    f: Fixture,
    request: MaterialFetch,
    path: std::path::PathBuf,
    _dir: tempfile::TempDir,
}
async fn post(
    app: Router,
    path: &str,
    value: Value,
    credential: Option<&str>,
) -> axum::response::Response {
    let mut req = Request::post(path).header("content-type", "application/json");
    if let Some(credential) = credential {
        req = req.header("authorization", format!("Bearer {credential}"));
    } else {
        req = req
            .header("x-aq-dev-email", "owner@localhost")
            .header("x-aq-request", "1");
    }
    app.oneshot(
        req.body(Body::from(serde_json::to_vec(&value).unwrap()))
            .unwrap(),
    )
    .await
    .unwrap()
}
async fn setup() -> Case {
    let mut f = fixture().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("provider");
    std::fs::write(&path, SECRET).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let choice = &f.selection.choice;
    let mut cfg = (*f.state.config).clone();
    cfg.intelligence = Some(Arc::new(
        IntelligenceConfig {
            revision: 1,
            credentials: vec![CredentialConfig {
                credential: AuthorizedCredential {
                    profile: choice.credential_profile.clone(),
                    provider: choice.provider.clone(),
                    account_reference: choice.account_reference.clone(),
                    endpoint_reference: choice.endpoint_reference.clone(),
                    revision: f.selection.credential_revision,
                    enabled: true,
                    provisionable: false,
                    material_owner: None,
                },
                allowed_node_ids: vec![f.node.node_id],
                secret_file: Some(path.clone()),
                delivery_mode: Some(MaterialDeliveryMode::ServerMaterialV1),
            }],
            qualified_harnesses: vec![],
        }
        .load()
        .unwrap(),
    ));
    let registration = Register {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: WorkerIdentity {
            node_id: f.node.node_id,
            boot_id: f.node.boot_id,
        },
        runtime: artifact("synthetic-runtime", b"code"),
        capabilities: vec![],
        features: vec![Feature::ServerMaterialV1],
    };
    let qualification = artifact("synthetic-qualification", b"qualified-synthetic-runtime");
    cfg.profiled_execution = Some(Arc::new(ProfiledConfig {
        revision: 1,
        qualified_runtimes: vec![QualifiedRuntime {
            runtime: registration.runtime.clone(),
            qualification: qualification.clone(),
            capabilities: vec![],
            features: registration.features.clone(),
            material_delivery: Some(DeliveryCapability {
                adapter: artifact("synthetic-material-adapter", b"code"),
                version: 1,
            }),
        }],
        projects: vec![],
    }));
    f.state.config = Arc::new(cfg);
    aq_server::profiled_execution::policies::install(&f.state)
        .await
        .unwrap();
    let native = NativeRegistration {
        schema_version: 1,
        semantic_digest: registration::semantic_digest(&registration).unwrap(),
        request: registration,
        qualification,
        configuration_revision: 1,
        registered_at_ms: NOW,
    };
    let pool = f.db.pool().await;
    sqlx::query("UPDATE execution_nodes SET state=jsonb_set(state,'{native}',$2) WHERE node_id=$1")
        .bind(f.node.node_id)
        .bind(json!(native))
        .execute(&pool)
        .await
        .unwrap();
    let installed = post(
        aq_server::build_admin_router(f.state.clone()),
        "/api/intelligence/register",
        json!({}),
        None,
    )
    .await;
    assert_eq!(installed.status(), StatusCode::OK);
    let attempt = insert(&f).await;
    let lease = MaterialLease {
        schema_version: 1,
        lease_id: Uuid::new_v4(),
        binding: f.profiled.offer.binding.clone(),
        credential_profile: f.selection.choice.credential_profile.clone(),
        credential_revision: f.selection.credential_revision,
        provider: Label::new(&f.selection.choice.provider).unwrap(),
        account_reference: Label::new(&f.selection.choice.account_reference).unwrap(),
        endpoint_reference: Label::new(&f.selection.choice.endpoint_reference).unwrap(),
        purpose: Label::new("model-authentication").unwrap(),
        material_digest: Digest::of(SECRET),
        material_bytes: SECRET.len() as u64,
        issued_at_ms: NOW,
        expires_at_ms: NOW + 1000,
    };
    let owner = LaunchOwner {
        guardian_id: Uuid::new_v4(),
        launch_id: Uuid::new_v4(),
        reservation_id: Uuid::new_v4(),
    };
    let op = Uuid::new_v4();
    let plan = PreparationPlan {
        schema_version: 1,
        operation_id: op,
        binding: lease.binding.clone(),
        reservation: NoLaunchReservation {
            reservation_id: owner.reservation_id,
            reservation_operation_id: Uuid::new_v4(),
            reservation_journal_digest: Digest::of(b"synthetic-reservation"),
        },
        guardian_id: owner.guardian_id,
        launch_id: owner.launch_id,
        clock: DeliveryClock::new(NOW, NOW + 1000).unwrap(),
        materials: vec![lease.clone()],
    };
    let doc = TransportDocument::PreparationPlan(plan);
    doc.validate().unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE")
        .bind(f.node.node_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SELECT attempt_id FROM execution_attempts WHERE attempt_id=$1 FOR UPDATE")
        .bind(attempt)
        .execute(&mut *tx)
        .await
        .unwrap();
    let mut hot = HotState::from_compat(&f.profiled).unwrap();
    sqlx::query("INSERT INTO profiled_authority_hot(attempt_id,state,record_revision,authority_revision,grant_revision) VALUES($1,$2,1,1,0)").bind(attempt).bind(json!(hot)).execute(&mut *tx).await.unwrap();
    hot.status = Status::Preparing;
    hot.owner = Some(owner);
    hot.preparation_operation_id = Some(op);
    sqlx::query("INSERT INTO profiled_execution_receipts(node_id,operation_id,kind,request_digest,response) VALUES($1,$2,'accept',$3,$4)").bind(f.node.node_id).bind(op).bind(Digest::of(b"synthetic-accept").as_str()).bind(json!(doc)).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE profiled_authority_hot SET state=$2 WHERE attempt_id=$1")
        .bind(attempt)
        .bind(json!(hot))
        .execute(&mut *tx)
        .await
        .unwrap();
    let identity = format!(
        "{}@{}",
        lease.credential_profile.id.as_str(),
        lease.credential_profile.version.as_str()
    );
    sqlx::query("INSERT INTO admission_material_leases(lease_id,operation_id,request_digest,node_id,boot_id,prospective_attempt_id,admitted_attempt_id,offer_id,credential_identity,credential_revision,purpose,lease,verifier,state,issued_at_ms,expires_at_ms) VALUES($1,$2,$3,$4,$5,$6,$6,$7,$8,$9,$10,$11,$12,'admitted',$13,$14)").bind(lease.lease_id).bind(Uuid::new_v4()).bind(Digest::of(b"synthetic-material-issuance").as_str()).bind(f.node.node_id).bind(f.node.boot_id).bind(attempt).bind(lease.binding.offer_id).bind(identity).bind(lease.credential_revision as i64).bind(lease.purpose.as_str()).bind(json!(lease)).bind(Digest::of(b"private-verifier-not-a-response").as_str()).bind(NOW as i64).bind((NOW+1000)as i64).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO profiled_launch_materials(attempt_id,lease_id) VALUES($1,$2)")
        .bind(attempt)
        .bind(lease.lease_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let request = MaterialFetch {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: WorkerIdentity {
            node_id: f.node.node_id,
            boot_id: f.node.boot_id,
        },
        binding: lease.binding,
        lease_id: lease.lease_id,
    };
    Case {
        f,
        request,
        path,
        _dir: dir,
    }
}
async fn fetch(c: &Case) -> axum::response::Response {
    post(
        aq_server::build_worker_router(c.f.state.clone()),
        "/v1/worker/profiled/material",
        json!(c.request),
        Some(&c.f.credential),
    )
    .await
}
#[tokio::test]
async fn private_delivery_retimes_original_expiry_without_grant_or_secret_receipt() {
    let c = setup().await;
    let pool = c.f.db.pool().await;
    let before: Value =
        sqlx::query_scalar("SELECT state FROM profiled_authority_hot WHERE attempt_id=$1")
            .bind(c.request.binding.attempt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    for elapsed in [0, 500] {
        c.f.time.store((NOW + elapsed) as i64, Ordering::SeqCst);
        let response = fetch(&c).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(
            response.headers()["content-type"],
            "application/octet-stream"
        );
        let metadata = response.headers()[material_api::METADATA_HEADER].as_bytes();
        assert!(metadata.len() <= material_api::MAX_METADATA_HEADER_BYTES);
        let metadata: MaterialDelivery =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(metadata).unwrap()).unwrap();
        assert_eq!(metadata.clock.expires_at_ms, NOW + 1000);
        assert_eq!(metadata.clock.valid_for_ms, 1000 - elapsed);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..], SECRET);
        metadata
            .verify_received(&c.request.binding, &bytes, 1_000_000, 2_000_000)
            .unwrap();
        assert!(
            metadata
                .verify_received(&c.request.binding, &bytes, 1_000_000, 1_001_000_000)
                .is_err()
        );
    }
    let after: Value =
        sqlx::query_scalar("SELECT state FROM profiled_authority_hot WHERE attempt_id=$1")
            .bind(c.request.binding.attempt_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    let receipts: i64 = sqlx::query_scalar("SELECT count(*) FROM admission_material_receipts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(receipts, 0);
    c.f.time.store((NOW + 1000) as i64, Ordering::SeqCst);
    let response = fetch(&c).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        !response
            .headers()
            .contains_key(material_api::METADATA_HEADER)
    );
    assert!(
        !response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .windows(SECRET.len())
            .any(|v| v == SECRET)
    );
    assert_eq!(
        aq_server::admission::classify("/v1/worker/profiled/material"),
        aq_server::admission::Class::Bulk
    );
}
#[tokio::test]
async fn missing_changed_and_revoked_material_never_falls_back_to_installed_credentials() {
    let c = setup().await;
    std::fs::write(&c.path, b"changed").unwrap();
    assert_eq!(fetch(&c).await.status(), StatusCode::CONFLICT);
    std::fs::remove_file(&c.path).unwrap();
    assert_eq!(fetch(&c).await.status(), StatusCode::CONFLICT);
    std::fs::write(&c.path, SECRET).unwrap();
    std::fs::set_permissions(&c.path, std::fs::Permissions::from_mode(0o600)).unwrap();
    sqlx::query("UPDATE admission_material_leases SET state='revoked' WHERE lease_id=$1")
        .bind(c.request.lease_id)
        .execute(&c.f.db.pool().await)
        .await
        .unwrap();
    assert_eq!(fetch(&c).await.status(), StatusCode::CONFLICT);
}
#[tokio::test]
async fn scope_current_registry_preparation_and_listener_boundaries_are_required() {
    let mut c = setup().await;
    let worker = aq_server::build_worker_router(c.f.state.clone());
    assert_eq!(
        post(
            worker.clone(),
            "/v1/worker/profiled/material",
            json!(c.request),
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let original = c.request.binding.clone();
    c.request.binding.offer_id = Uuid::new_v4();
    assert_eq!(fetch(&c).await.status(), StatusCode::CONFLICT);
    c.request.binding = original;
    let response = post(
        aq_server::build_admin_router(c.f.state.clone()),
        "/v1/worker/profiled/material",
        json!(c.request),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = worker
        .oneshot(
            Request::post("/v1/worker/profiled/material")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", c.f.credential))
                .body(Body::from(vec![b' '; material_api::MAX_REQUEST_BYTES + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    sqlx::query("UPDATE intelligence_registry SET config_digest=$1 WHERE singleton=TRUE")
        .bind("f".repeat(64))
        .execute(&c.f.db.pool().await)
        .await
        .unwrap();
    assert_eq!(fetch(&c).await.status(), StatusCode::CONFLICT);
}
#[tokio::test]
async fn actual_lease_lock_wait_charges_remaining_preparation_ttl() {
    let c = setup().await;
    let pool = c.f.db.pool().await;
    let mut blocked = pool.begin().await.unwrap();
    sqlx::query("SELECT lease_id FROM admission_material_leases WHERE lease_id=$1 FOR UPDATE")
        .bind(c.request.lease_id)
        .execute(&mut *blocked)
        .await
        .unwrap();
    let state = c.f.state.clone();
    let request = c.request.clone();
    let credential = c.f.credential.clone();
    let task = tokio::spawn(async move {
        post(
            aq_server::build_worker_router(state),
            "/v1/worker/profiled/material",
            json!(request),
            Some(&credential),
        )
        .await
    });
    let end = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'SELECT m.* FROM admission_material_leases m JOIN%')").fetch_one(&pool).await.unwrap();
        if waiting {
            break;
        }
        assert!(
            tokio::time::Instant::now() < end,
            "material fetch did not reach lease lock"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    c.f.time.store((NOW + 1000) as i64, Ordering::SeqCst);
    blocked.commit().await.unwrap();
    let response = task.await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        !response
            .headers()
            .contains_key(material_api::METADATA_HEADER)
    );
}

#[tokio::test]
async fn credential_rotation_during_observed_node_wait_denies_material() {
    let c = setup().await;
    let pool = c.f.db.pool().await;
    let mut blocked = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE")
        .bind(c.f.node.node_id)
        .execute(&mut *blocked)
        .await
        .unwrap();
    let blocker: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocked)
        .await
        .unwrap();
    let state = c.f.state.clone();
    let request = c.request.clone();
    let credential = c.f.credential.clone();
    let task = tokio::spawn(async move {
        post(
            aq_server::build_worker_router(state),
            "/v1/worker/profiled/material",
            json!(request),
            Some(&credential),
        )
        .await
    });
    let end = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND $1=ANY(pg_blocking_pids(pid)))").bind(blocker).fetch_one(&pool).await.unwrap();
        if waiting {
            break;
        }
        assert!(
            tokio::time::Instant::now() < end,
            "material fetch did not reach node lock"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    sqlx::query("UPDATE nodes SET credential_hash=$2 WHERE id=$1")
        .bind(c.f.node.node_id)
        .bind(aq_server::tokens::verifier("rotated-synthetic-credential"))
        .execute(&mut *blocked)
        .await
        .unwrap();
    blocked.commit().await.unwrap();
    let response = task.await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(
        !response
            .headers()
            .contains_key(material_api::METADATA_HEADER)
    );
    assert!(
        !response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .windows(SECRET.len())
            .any(|v| v == SECRET)
    );
}

#[tokio::test]
async fn revoked_preparation_and_missing_plan_refuse_even_when_lease_remains_admitted() {
    let c = setup().await;
    let pool = c.f.db.pool().await;
    sqlx::query("UPDATE profiled_authority_hot SET state=jsonb_set(state,'{revoked}','true') WHERE attempt_id=$1").bind(c.request.binding.attempt_id).execute(&pool).await.unwrap();
    assert_eq!(fetch(&c).await.status(), StatusCode::CONFLICT);
    let c = setup().await;
    let pool = c.f.db.pool().await;
    sqlx::query("UPDATE profiled_authority_hot SET state=jsonb_set(state,'{preparation_operation_id}','null') WHERE attempt_id=$1").bind(c.request.binding.attempt_id).execute(&pool).await.unwrap();
    assert_eq!(fetch(&c).await.status(), StatusCode::CONFLICT);
}
