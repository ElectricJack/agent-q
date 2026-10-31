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
