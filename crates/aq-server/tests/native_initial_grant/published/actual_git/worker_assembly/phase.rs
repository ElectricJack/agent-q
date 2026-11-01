//! Actual worker/HTTP/SQL acceptance; runtime qualification remains synthetic.
use super::*;
use aq_admission::{
    ArtifactKind,
    preparation_v2::{
        self as wire, AcceptRequest, AcceptedDelivery, FactKind, OwnerAuthority, PhasePolicy,
        Proposal,
    },
};
use aq_server::profiled_execution::{
    accepted_preparation::PolicyDefinition, definitions::Definition,
};
use sqlx::Row;
fn reference(id: &str) -> ArtifactRef {
    ArtifactRef {
        id: Text::new(id).unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(id.as_bytes()),
    }
}
pub(super) async fn configure(case: &mut Case) -> Value {
    let version: String =
        sqlx::query_scalar("SELECT value FROM meta WHERE key='state_format_version'")
            .fetch_one(&case.env.env.pool)
            .await
            .unwrap();
    assert_eq!(
        version.parse::<u32>().unwrap(),
        aq_server::STATE_FORMAT_VERSION
    );
    assert!(
        version.parse::<u32>().unwrap() >= 30,
        "actual ordered preparation owner required"
    );
    let mut config = (*case.env.env.state.config).clone();
    let mut p = (**config.profiled_execution.as_ref().unwrap()).clone();
    let runtime = &p.qualified_runtimes[0];
    let source_profile = reference("synthetic-qualification/actual-worker-default-git-profile");
    let policy = PolicyDefinition {
        schema_version: 1,
        contract: wire::contract_artifact().unwrap(),
        runtime: runtime.runtime.clone(),
        qualification: runtime.qualification.clone(),
        phase: PhasePolicy {
            schema_version: 2,
            maximum_preparation_ms: 30000,
            maximum_execution_lease_ms: 10000,
            maximum_observation_ms: 2000,
        },
        source_profile: source_profile.clone(),
        source_disk_bytes: 21 * 1024 * 1024 * 1024 + 1024 * 1024,
        owners: FactKind::all()
            .iter()
            .map(|kind| OwnerAuthority {
                kind: *kind,
                authority: reference(&format!("synthetic-owner/{kind:?}")),
            })
            .collect(),
    };
    let body = serde_json::to_string(&policy).unwrap();
    let pin = ArtifactRef {
        id: Text::new("aq.accepted-preparation-policy/actual-worker-test").unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(body.as_bytes()),
    };
    p.definition_bodies
        .push(Definition::new(pin, ArtifactKind::Definition, body).unwrap());
    p.revision += 1;
    p.validate().unwrap();
    config.profiled_execution = Some(Arc::new(p));
    case.env.env.state.config = Arc::new(config);
    aq_server::profiled_execution::policies::install(&case.env.env.state)
        .await
        .unwrap();
    json!({"negotiation":wire::Negotiation { contract:wire::contract_artifact().unwrap(),features:wire::required_features() },"source_profile":source_profile,"maximum_preparation_ms":30000})
}
pub(super) async fn verify(
    case: &Case,
    root: &std::path::Path,
    identity: &aq_admission::transport::WorkerIdentity,
    result: &Value,
) {
    let q: AcceptRequest = serde_json::from_value(result["request"].clone()).unwrap();
    let p: Proposal = serde_json::from_value(result["proposal"].clone()).unwrap();
    let initial: AcceptedDelivery =
        serde_json::from_value(result["initial_delivery"].clone()).unwrap();
    let replay: AcceptedDelivery =
        serde_json::from_value(result["replay_delivery"].clone()).unwrap();
    assert_eq!(&q.original.identity, identity);
    initial
        .validate_for(&q, &p, initial.delivery.nonce)
        .unwrap();
    replay.validate_for(&q, &p, replay.delivery.nonce).unwrap();
    assert_ne!(initial.delivery.nonce, replay.delivery.nonce);
    assert_eq!(
        serde_json::to_value(&initial.receipt).unwrap(),
        serde_json::to_value(&replay.receipt).unwrap()
    );
    assert!(replay.delivery.clock.server_time_ms >= p.original.offered.clock.expires_at_ms);
    assert!(
        result["checked_at_ns"].as_u64().unwrap()
            > result["original_deadline_ns"].as_u64().unwrap()
    );
    assert!(result["checked_at_ns"].as_u64().unwrap() < result["deadline_ns"].as_u64().unwrap());
    assert_eq!(result["first_history"], result["history"]);
    let snapshot: Value =
        serde_json::from_slice(&std::fs::read(root.join("journal/journal.json")).unwrap()).unwrap();
    assert_eq!(snapshot["state"]["format"], 7);
    assert_eq!(result["journal_format"], 7);
    let attempt =
        &snapshot["state"]["attempts"][q.original.acceptance.binding.attempt_id.to_string()];
    assert_eq!(attempt["phase_acceptance"]["request"], result["request"]);
    assert_eq!(attempt["phase_acceptance"]["proposal"], result["proposal"]);
    assert_eq!(attempt["phase_preparation"], result["history"]);
    assert_eq!(
        attempt["phase_preparation"]["delivery"],
        result["initial_delivery"]
    );
    assert_eq!(
        result["journal_revision"],
        attempt["phase_preparation"]["record_revision"]
    );
    if result["phase"] == "prepared_phase_inputs" {
        verify_inputs(root, result, attempt, &q, &p, &initial, &snapshot);
    } else {
        assert_eq!(snapshot["state"]["revision"], result["journal_revision"]);
        assert!(attempt["phase_prepared"].is_null());
    }
    assert!(
        attempt["phase_preparation"]["record_revision"]
            .as_u64()
            .unwrap()
            >= q.original.journal_revision + 2
    );
    assert!(
        attempt["preparation"].is_null()
            && attempt["prepared"].is_null()
            && attempt["initial_grant"].is_null()
    );
    let row = sqlx::query(
        "SELECT request,receipt,receipt_digest FROM profiled_v2_accepts WHERE attempt_id=$1",
    )
    .bind(q.original.acceptance.binding.attempt_id)
    .fetch_one(&case.env.env.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<Value, _>("request"), result["request"]);
    assert_eq!(
        row.get::<Value, _>("receipt"),
        serde_json::to_value(&initial.receipt).unwrap()
    );
    assert_eq!(
        row.get::<String, _>("receipt_digest"),
        initial.receipt.digest().unwrap().as_str()
    );
    let hot: Value =
        sqlx::query_scalar("SELECT state FROM profiled_authority_hot WHERE attempt_id=$1")
            .bind(q.original.acceptance.binding.attempt_id)
            .fetch_one(&case.env.env.pool)
            .await
            .unwrap();
    assert_eq!(hot["status"], "preparing");
    assert!(hot["grant"].is_null());
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT holds_task FROM execution_attempts WHERE attempt_id=$1"
        )
        .bind(q.original.acceptance.binding.attempt_id)
        .fetch_one(&case.env.env.pool)
        .await
        .unwrap()
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM profiled_execution_receipts WHERE kind IN ('accept','prepare','started')").fetch_one(&case.env.env.pool).await.unwrap(),0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM admission_material_receipts")
            .fetch_one(&case.env.env.pool)
            .await
            .unwrap(),
        0
    );
    eprintln!(
        "bounded phase clocks: {}",
        json!({"proposal_expiry_ms":p.original.offered.clock.expires_at_ms,"accepted_issued_ms":initial.receipt.lease.issued_at_ms,"accepted_expiry_ms":initial.receipt.lease.expires_at_ms,"replay_server_ms":replay.delivery.clock.server_time_ms,"journal_format":snapshot["state"]["format"],"journal_revision":snapshot["state"]["revision"]})
    );
}

fn verify_inputs(
    root: &std::path::Path,
    result: &Value,
    attempt: &Value,
    acceptance: &AcceptRequest,
    proposal: &Proposal,
    accepted: &AcceptedDelivery,
    snapshot: &Value,
) {
    let prepare: wire::PrepareRequest =
        serde_json::from_value(result["prepare_request"].clone()).unwrap();
    prepare
        .validate_for(&accepted.receipt, acceptance, proposal)
        .unwrap();
    assert_eq!(
        attempt["phase_prepared"]["request"],
        result["prepare_request"]
    );
    assert_eq!(snapshot["state"]["revision"], result["prepared_revision"]);
    assert!(
        result["prepared_revision"].as_u64().unwrap()
            > result["journal_revision"].as_u64().unwrap()
    );
    assert!(
        result["prepared_at_ns"].as_u64().unwrap()
            > result["original_deadline_ns"].as_u64().unwrap()
    );
    assert!(result["prepared_at_ns"].as_u64().unwrap() < result["deadline_ns"].as_u64().unwrap());
    assert_eq!(
        serde_json::to_value(&prepare.original.materials).unwrap(),
        result["material_receipts"]
    );
    assert_eq!(
        prepare.original.artifacts.len(),
        proposal.original.offered.artifacts.len() + 2
    );
    let workspace = std::path::PathBuf::from(result["workspace_path"].as_str().unwrap());
    assert!(workspace.starts_with(root.join("git/workspaces")));
    assert_eq!(
        std::fs::read(workspace.join("source")).unwrap(),
        b"actual source bytes\n"
    );
    assert!(
        attempt["downloads"]
            .as_object()
            .unwrap()
            .values()
            .all(|d| !d["receipt"].is_null())
    );
    assert!(attempt["source"]["verified"].is_object());
    eprintln!(
        "bounded phase inputs: {}",
        json!({"artifacts":prepare.original.artifacts.len(),"materials":prepare.original.materials.len(),"prepared_revision":result["prepared_revision"],"prepared_at_ns":result["prepared_at_ns"],"original_deadline_ns":result["original_deadline_ns"],"deadline_ns":result["deadline_ns"]})
    );
}
