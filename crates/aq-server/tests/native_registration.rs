//! Explicit admitted-metadata fixtures; no real runtime/host is qualified here.
mod common;
use aq_admission::transport::{Feature, Register, WorkerIdentity};
use aq_server::profiled_execution::{
    config::{ProfiledConfig, QualifiedRuntime},
    registration::{self, NativeRegistration},
};
use common::profiled::{NOW, artifact, fixture};
use std::sync::Arc;
use uuid::Uuid;
#[tokio::test]
async fn configured_allowlist_does_not_synthesize_registration_and_admitted_identity_is_exact() {
    let mut f = fixture().await;
    let request = Register {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: WorkerIdentity {
            node_id: f.node.node_id,
            boot_id: f.node.boot_id,
        },
        runtime: artifact("synthetic-native-runtime", b"synthetic-code"),
        capabilities: vec![],
        features: vec![Feature::ArtifactTransferV1, Feature::DurableHostJournalV1],
    };
    let qualification = artifact(
        "synthetic-runtime-qualification",
        b"synthetic-qualification",
    );
    let mut config = (*f.state.config).clone();
    config.profiled_execution = Some(Arc::new(ProfiledConfig {
        revision: 1,
        qualified_runtimes: vec![QualifiedRuntime {
            runtime: request.runtime.clone(),
            qualification: qualification.clone(),
            capabilities: vec![],
            features: request.features.clone(),
            material_delivery: None,
        }],
        projects: vec![],
    }));
    f.state.config = Arc::new(config);
    aq_server::profiled_execution::policies::install(&f.state)
        .await
        .unwrap();
    let pool = f.db.pool().await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE")
        .bind(f.node.node_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(
        registration::current_runtime_locked(&f.state, &mut tx, f.node.node_id, f.node.boot_id)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let legacy = aq_worker_protocol::RegisterRequest {
        identity: f.node.clone(),
        operation_id: Uuid::new_v4(),
        protocol_version: 2,
        worker_version: "0.1.0".into(),
        capabilities: vec![aq_worker_protocol::FAKE_HARNESS_CAPABILITY.into()],
    };
    let historical = f.state.store.execution_register(&legacy).await.unwrap();
    let metadata = NativeRegistration {
        schema_version: 1,
        semantic_digest: registration::semantic_digest(&request).unwrap(),
        request: request.clone(),
        qualification: qualification.clone(),
        configuration_revision: 1,
        registered_at_ms: NOW,
    };
    sqlx::query("UPDATE execution_nodes SET state=jsonb_set(state,'{native}',$2) WHERE node_id=$1")
        .bind(f.node.node_id)
        .bind(serde_json::json!(metadata))
        .execute(&pool)
        .await
        .unwrap();
    // Old exact acknowledgments remain readable, but cannot reinstall a
    // fixture incarnation or make the native runtime eligible for fixture offers.
    let before: serde_json::Value =
        sqlx::query_scalar("SELECT state FROM execution_nodes WHERE node_id=$1")
            .bind(f.node.node_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        serde_json::to_value(f.state.store.execution_register(&legacy).await.unwrap()).unwrap(),
        serde_json::to_value(historical).unwrap()
    );
    let mut changed = legacy.clone();
    changed.operation_id = Uuid::new_v4();
    assert!(f.state.store.execution_register(&changed).await.is_err());
    assert!(
        f.state
            .store
            .execution_replace_incarnation(&changed)
            .await
            .is_err()
    );
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT state FROM execution_nodes WHERE node_id=$1")
            .bind(f.node.node_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    assert!(
        f.state
            .store
            .execution_reconcile(
                &aq_worker_protocol::ReconcileRequest {
                    identity: f.node.clone(),
                    operation_id: Uuid::new_v4(),
                    attempts: vec![],
                    cursor: None,
                    max_items: 16,
                },
                NOW
            )
            .await
            .is_err()
    );
    assert!(
        f.state
            .store
            .execution_poll(
                &aq_worker_protocol::PollRequest {
                    identity: f.node.clone(),
                    operation_id: Uuid::new_v4(),
                    inventory_sequence: 1,
                    available: aq_worker_protocol::Resources {
                        slots: 1,
                        cpu_millis: 100,
                        memory_bytes: 1024,
                        disk_bytes: 1024
                    },
                    max_items: 1,
                },
                NOW
            )
            .await
            .is_err()
    );

    let mut tx = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM nodes WHERE id=$1 FOR UPDATE")
        .bind(f.node.node_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let actual =
        registration::current_runtime_locked(&f.state, &mut tx, f.node.node_id, f.node.boot_id)
            .await
            .unwrap();
    assert_eq!(actual.runtime, request.runtime);
    assert_eq!(actual.qualification, qualification);
    assert_eq!(actual.boot_id, f.node.boot_id);
    assert!(
        registration::current_runtime_locked(&f.state, &mut tx, f.node.node_id, Uuid::new_v4())
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
    let mut corrupted = metadata.clone();
    corrupted.semantic_digest = aq_admission::Digest::of(b"changed");
    sqlx::query("UPDATE execution_nodes SET state=jsonb_set(state,'{native}',$2) WHERE node_id=$1")
        .bind(f.node.node_id)
        .bind(serde_json::json!(corrupted))
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(
        registration::current_runtime_locked(&f.state, &mut tx, f.node.node_id, f.node.boot_id)
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();
}
#[test]
fn native_semantics_ignore_only_operation_id_not_runtime_or_capability_change() {
    let first = Register {
        schema_version: 1,
        operation_id: Uuid::from_u128(1),
        identity: WorkerIdentity {
            node_id: Uuid::from_u128(2),
            boot_id: Uuid::from_u128(3),
        },
        runtime: artifact("synthetic-native-runtime", b"runtime"),
        capabilities: vec![],
        features: vec![Feature::ArtifactTransferV1],
    };
    let mut retry = first.clone();
    retry.operation_id = Uuid::from_u128(4);
    assert_eq!(
        registration::semantic_digest(&first).unwrap(),
        registration::semantic_digest(&retry).unwrap()
    );
    retry.features.push(Feature::DurableHostJournalV1);
    assert_ne!(
        registration::semantic_digest(&first).unwrap(),
        registration::semantic_digest(&retry).unwrap()
    );
}

#[tokio::test]
async fn native_writer_replays_without_reinstall_and_fences_held_claims_and_credentials() {
    let mut f = fixture().await;
    let pool = f.db.pool().await;
    // 019 remains explicitly installed until root's ordered 018 checkpoint.
    sqlx::raw_sql(include_str!("../migrations/0019_native_registration.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let request = Register {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: WorkerIdentity {
            node_id: f.node.node_id,
            boot_id: f.node.boot_id,
        },
        runtime: artifact("synthetic-native-runtime", b"code"),
        capabilities: vec![],
        features: vec![Feature::ArtifactTransferV1, Feature::DurableHostJournalV1],
    };
    let mut config = (*f.state.config).clone();
    config.profiled_execution = Some(Arc::new(ProfiledConfig {
        revision: 1,
        qualified_runtimes: vec![QualifiedRuntime {
            runtime: request.runtime.clone(),
            qualification: artifact("synthetic-qualification", b"qualification"),
            capabilities: vec![],
            features: request.features.clone(),
            material_delivery: None,
        }],
        projects: vec![],
    }));
    f.state.config = Arc::new(config);
    aq_server::profiled_execution::policies::install(&f.state)
        .await
        .unwrap();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::AUTHORIZATION,
        format!("Bearer {}", f.credential).parse().unwrap(),
    );
    let auth = aq_server::worker_api::authenticate_context(&f.state, &headers)
        .await
        .unwrap();
    registration::register(&f.state, &auth, &request)
        .await
        .unwrap();
    let attempt = common::profiled::insert(&f).await;
    sqlx::query("INSERT INTO profiled_authority_hot(attempt_id,state,record_revision,authority_revision,grant_revision) VALUES($1,$2,1,1,0)").bind(attempt).bind(serde_json::json!(aq_server::execution::authority::hot::HotState::from_compat(&f.profiled).unwrap())).execute(&pool).await.unwrap();
    assert!(sqlx::query("UPDATE execution_nodes SET state=jsonb_set(state,'{worker_version}','\"changed\"') WHERE node_id=$1")
        .bind(f.node.node_id).execute(&pool).await.is_err());
    let mut changed = request.clone();
    changed.operation_id = Uuid::new_v4();
    changed.identity.boot_id = Uuid::new_v4();
    assert!(
        registration::register(&f.state, &auth, &changed)
            .await
            .is_err()
    );
    let mut semantic_retry = request.clone();
    semantic_retry.operation_id = Uuid::new_v4();
    registration::register(&f.state, &auth, &semantic_retry)
        .await
        .unwrap();
    // Changed bytes under the same operation conflict, even while policy matches.
    let mut conflict = request.clone();
    conflict.identity.boot_id = Uuid::new_v4();
    assert!(
        registration::register(&f.state, &auth, &conflict)
            .await
            .is_err()
    );
    f.time
        .store((NOW + 30_001) as i64, std::sync::atomic::Ordering::SeqCst);
    aq_server::profiled_execution::no_grant::close_expired(&f.state, f.node.node_id, attempt)
        .await
        .unwrap()
        .unwrap();
    registration::register(&f.state, &auth, &changed)
        .await
        .unwrap();
    // Historical original replay never restores the old boot.
    registration::register(&f.state, &auth, &request)
        .await
        .unwrap();
    let current: serde_json::Value =
        sqlx::query_scalar("SELECT state FROM execution_nodes WHERE node_id=$1")
            .bind(f.node.node_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        current["identity"]["boot_id"],
        changed.identity.boot_id.to_string()
    );
    let mut newer = f.state.clone();
    let mut newer_config = (*f.state.config).clone();
    let mut newer_profiled = (**newer_config.profiled_execution.as_ref().unwrap()).clone();
    newer_profiled.revision = 2;
    newer_profiled.qualified_runtimes.clear();
    newer_config.profiled_execution = Some(Arc::new(newer_profiled));
    newer.config = Arc::new(newer_config);
    aq_server::profiled_execution::policies::install(&newer)
        .await
        .unwrap();
    let mut stale_retry = changed.clone();
    stale_retry.operation_id = Uuid::new_v4();
    assert!(
        registration::register(&f.state, &auth, &stale_retry)
            .await
            .is_err()
    );
    let mut tx = pool.begin().await.unwrap();
    assert!(
        registration::current_runtime_locked(
            &f.state,
            &mut tx,
            f.node.node_id,
            changed.identity.boot_id
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    // Historical receipt readback survives policy retirement but remains
    // subject to current machine authentication.
    registration::register(&f.state, &auth, &request)
        .await
        .unwrap();
    sqlx::query("UPDATE nodes SET credential_hash=$2 WHERE id=$1")
        .bind(f.node.node_id)
        .bind(aq_server::tokens::verifier("rotated"))
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        registration::register(&f.state, &auth, &request)
            .await
            .is_err()
    );
}
