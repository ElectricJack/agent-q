//! Actual phase owner refusals preserve preparation; no physical worker/provider
//! qualification is inferred from these explicitly synthetic peer observations.
use super::*;
use aq_admission::scoped_presence::{InventoryReceipt, InventoryV2, decode_inventory};
use std::sync::atomic::Ordering;

async fn audit(f: &Prepared) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('hot',(SELECT to_jsonb(h) FROM profiled_authority_hot h WHERE attempt_id=$1),'journal',(SELECT to_jsonb(j) FROM profiled_owner_journals j WHERE attempt_id=$1),'holds_task',(SELECT holds_task FROM execution_attempts WHERE attempt_id=$1),'releases',(SELECT count(*) FROM profiled_verified_releases WHERE attempt_id=$1),'leases',(SELECT coalesce(jsonb_agg(jsonb_build_object('lease_id',lease_id,'state',state,'expires_at_ms',expires_at_ms) ORDER BY lease_id),'[]'::JSONB) FROM admission_material_leases WHERE admitted_attempt_id=$1))")
        .bind(f.prepared.original.binding.attempt_id)
        .fetch_one(&f.c.env.env.pool).await.unwrap()
}
async fn refused_without_partial_state(f: &Prepared, before: &Value) {
    let result = phase_execution::prepare(
        &f.c.env.env.state,
        &auth(&f.c).await,
        &f.prepared,
        Uuid::new_v4(),
    )
    .await;
    assert!(
        result.is_err(),
        "changed current fact unexpectedly issued execution"
    );
    assert_unissued(&f.c).await;
    assert_eq!(
        &audit(f).await,
        before,
        "refusal must not alter hot authority, owner journal, material state or release"
    );
}
#[tokio::test]
async fn changed_graph_cas_content_and_raw_metadata_refuse_without_partial_authority() {
    // Modes are actual bounded HTTP Graph peers: changed CAS, inconsistent
    // context body, consistently changed root body, and non-startup metadata.
    for mode in [1, 2, 4, 5] {
        let f = prepared(true).await;
        let before = audit(&f).await;
        f.c.mode.store(mode, Ordering::SeqCst);
        refused_without_partial_state(&f, &before).await;
    }
}
#[tokio::test]
async fn no_longer_in_bounded_ready_page_cannot_issue_execution() {
    let f = prepared(true).await;
    let before = audit(&f).await;
    f.c.mode.store(3, Ordering::SeqCst);
    refused_without_partial_state(&f, &before).await;
}
#[tokio::test]
async fn changed_actual_private_backing_refuses_without_installation_or_grant_receipt() {
    let f = prepared(true).await;
    let before = audit(&f).await;
    let registry = f.c.env.env.state.config.intelligence.as_ref().unwrap();
    let path = registry
        .config
        .credentials
        .iter()
        .find(|c| c.credential.profile == f.a.lease.materials[0].credential_profile)
        .unwrap()
        .secret_file
        .as_ref()
        .unwrap();
    std::fs::write(
        path,
        b"changed synthetic backing after actual material retrieval",
    )
    .unwrap();
    refused_without_partial_state(&f, &before).await;
}
#[tokio::test]
async fn retired_actual_project_policy_fences_an_old_daemon_configuration() {
    let f = prepared(true).await;
    let before = audit(&f).await;
    let mut installer = f.c.env.env.state.clone();
    let mut config = (*installer.config).clone();
    let mut profiled = (**config.profiled_execution.as_ref().unwrap()).clone();
    profiled.revision += 1;
    for policy in &mut profiled.projects {
        policy.enabled = false;
    }
    config.profiled_execution = Some(std::sync::Arc::new(profiled));
    installer.config = std::sync::Arc::new(config);
    policies::install(&installer).await.unwrap();
    let enabled: bool = sqlx::query_scalar(
        "SELECT (policy->>'enabled')::boolean FROM admission_source_policies WHERE project_id=$1",
    )
    .bind(f.p.spec.lifecycle.project_id)
    .fetch_one(&f.c.env.env.pool)
    .await
    .unwrap();
    assert!(!enabled);
    // Keep f's original config: stale daemon snapshots must fail closed.
    refused_without_partial_state(&f, &before).await;
}
async fn inventory(f: &Prepared) -> InventoryV2 {
    let bytes:Vec<u8>=sqlx::query_scalar("SELECT request_bytes FROM profiled_presence_operations WHERE node_id=$1 AND boot_id=$2 AND kind='inventory_v2' ORDER BY inventory_revision DESC LIMIT 1")
        .bind(f.prepared.original.identity.node_id).bind(f.prepared.original.identity.boot_id)
        .fetch_one(&f.c.env.env.pool).await.unwrap();
    decode_inventory(&bytes).unwrap()
}
async fn record_inventory(f: &Prepared, q: &InventoryV2) -> InventoryReceipt {
    aq_server::profiled_execution::presence::inventory::record(
        &f.c.env.env.state,
        &auth(&f.c).await,
        q,
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn newer_legacy_then_v2_restoration_cannot_reuse_pre_barrier_selection() {
    let f = prepared(true).await;
    let before = audit(&f).await;
    let mut current = inventory(&f).await;
    let legacy = serde_json::json!({"operation_id":Uuid::new_v4(),"boot_id":current.identity.boot_id,
        "sequence":current.sequence+1,"ttl_ms":30_000,"qualifications":current.qualifications,"credentials":[]});
    let response = native_offer::post(
        &f.c.env.env.state,
        &f.c.env.env.credential,
        "/v1/worker/intelligence/inventory",
        serde_json::to_vec(&legacy).unwrap(),
    )
    .await;
    let status = response.status();
    let body = to_bytes(response.into_body(), 65536).await.unwrap();
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    current.operation_id = Uuid::new_v4();
    current.sequence += 2;
    record_inventory(&f, &current).await;
    refused_without_partial_state(&f, &before).await;
}
#[tokio::test]
async fn compatible_inventory_refresh_retains_actual_sequence_in_execution_fact() {
    let f = prepared(true).await;
    let mut current = inventory(&f).await;
    current.operation_id = Uuid::new_v4();
    current.sequence += 7;
    let ack = record_inventory(&f, &current).await;
    assert_ne!(
        current.sequence, ack.inventory.revision,
        "fixture must distinguish worker sequence from normalized revision"
    );
    let result = phase_execution::prepare(
        &f.c.env.env.state,
        &auth(&f.c).await,
        &f.prepared,
        Uuid::new_v4(),
    )
    .await
    .unwrap();
    let recorded = result
        .receipt
        .confirmation
        .facts
        .iter()
        .find_map(|fact| {
            if let phase::FactValue::Inventory { sequence, .. } = &fact.body.value {
                Some(*sequence)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(recorded, current.sequence);
}
