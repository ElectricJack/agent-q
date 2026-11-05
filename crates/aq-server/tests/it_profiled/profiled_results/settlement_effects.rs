//! Actual ordered Store/B1/stopped authority reads with explicitly synthetic
//! agent-binding and ledger rows. These fixtures qualify inspection/locking and
//! refusal, NOT command admission, Graph execution, source retention or release.
use super::*;
use aq_server::profiled_execution::settlement::effects::{self, LedgerDisposition};
use serde_json::Value;
use sqlx::Row;

struct Case {
    f: Fixture,
    report: ResultReport,
    profile: Value,
    binding: Value,
}
async fn setup(message: bool) -> Case {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../protocol/agent-access/v1/fixtures/mutation-delivery.json"
    ))
    .unwrap();
    let mut profile = fixture["proposal_profiles"][0]["profile"].clone();
    // Use an actual frozen schema fixture before touching a disposable owner.
    assert!(profile.is_object());
    profile["capabilities"] = if message {
        json!(["read.get", "message.send"])
    } else {
        json!(["read.get", "read.context"])
    };
    let (f, _, report) = prepare_with_graph_profile(true, true, Some(profile.clone())).await;
    results::record(&f.state, &context(&f).await, &report)
        .await
        .unwrap();
    let pool = f.db.pool().await;
    let mut tx = pool.begin().await.unwrap();
    let recorded = results::load_locked(&f.state, &mut tx, report.report_id)
        .await
        .unwrap();
    let SessionIdentity::Remote {
        node_id,
        boot_id,
        session_id,
        ..
    } = report.binding.session
    else {
        unreachable!()
    };
    let binding = json!({"task_id":recorded.spec.task.task_id,"attempt_id":report.binding.attempt_id,"session_id":session_id,"generation":report.binding.assignment_generation,"node_id":node_id,"boot_id":boot_id,"project_id":recorded.spec.lifecycle.project_id,"workspace_id":recorded.spec.task.workspace_id,"resolved_spec_digest":report.binding.execution_spec_digest,"profile_digest":aq_agent_api::digest(&profile).unwrap(),"context_digest":f.profiled.graph.context_digest});
    assert!(aq_agent_api::valid("Binding", &binding));
    // Synthetic admission history only. Never called through an HTTP credential
    // issuer, and never presented as an actual command/dispatch qualification.
    sqlx::query("INSERT INTO agent_launch_bindings(attempt_id,node_id,binding,profile,context) VALUES($1,$2,$3,$4,'{}')").bind(report.binding.attempt_id).bind(node_id).bind(&binding).bind(&profile).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    Case {
        f,
        report,
        profile,
        binding,
    }
}
async fn inspect(
    case: &Case,
) -> std::result::Result<effects::AqEffectsSnapshot, aq_server::error::AppError> {
    let pool = case.f.db.pool().await;
    let mut tx = pool.begin().await.unwrap();
    let recorded = results::load_locked(&case.f.state, &mut tx, case.report.report_id)
        .await
        .unwrap();
    let snapshot = effects::inspect_aq_effects_locked(&case.f.state, &mut tx, &recorded).await;
    tx.rollback().await.unwrap();
    snapshot
}
async fn insert(case: &Case, state: &str) -> Uuid {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../protocol/agent-access/v1/fixtures/base.json"
    ))
    .unwrap();
    let op = Uuid::new_v4();
    let mut request = fixture["command_requests"]["correct_record"].clone();
    request["binding"] = case.binding.clone();
    request["operation_id"] = json!(op);
    let digest = aq_agent_api::digest(&request).unwrap();
    let mut status = fixture["operation_statuses"][state].clone();
    status["binding"] = case.binding.clone();
    status["operation_id"] = json!(op);
    status["payload_digest"] = json!(digest);
    if state == "confirmed" {
        status["receipt"]["workspace_id"] = case.binding["workspace_id"].clone();
    }
    assert!(aq_agent_api::valid("CommandRequest", &request));
    assert!(aq_agent_api::valid("OperationStatus", &status));
    let mapped = json!({"service":{"workspace_id":case.binding["workspace_id"]},"payload":{"idempotency_key":format!("aq-agent-v1:{op}")}});
    let pool = case.f.db.pool().await;
    sqlx::query("INSERT INTO agent_graph_operations(operation_id,attempt_id,node_id,request,payload_digest,mapped,policy,status,phase,created_at_ms,dispatch_count,retained_bytes) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,1,32768)")
        .bind(op).bind(case.report.binding.attempt_id).bind(case.f.node.node_id).bind(request).bind(digest).bind(mapped).bind(&case.profile["retry_budget"]).bind(status).bind(if state=="pending"{"queued"}else{state}).bind(NOW as i64).execute(&pool).await.unwrap();
    op
}
#[tokio::test]
async fn actual_stopped_ledger_scan_is_exhaustive_and_preserves_held_authority() {
    let case = setup(false).await;
    let before = inspect(&case).await.unwrap();
    assert_eq!(before.disposition(), LedgerDisposition::Settled);
    assert_eq!(before.operation_count(), 0);
    let op = insert(&case, "confirmed").await;
    let after = inspect(&case).await.unwrap();
    assert_eq!(after.disposition(), LedgerDisposition::Settled);
    assert_eq!(after.operation_count(), 1);
    assert_ne!(before.digest(), after.digest());
    let pool = case.f.db.pool().await;
    let row=sqlx::query("SELECT holds_task,(SELECT count(*) FROM profiled_verified_releases) AS releases FROM execution_attempts WHERE attempt_id=$1").bind(case.report.binding.attempt_id).fetch_one(&pool).await.unwrap();
    assert!(row.get::<bool, _>("holds_task"));
    assert_eq!(row.get::<i64, _>("releases"), 0);
    sqlx::query("UPDATE agent_graph_operations SET payload_digest=$2 WHERE operation_id=$1")
        .bind(op)
        .bind("a".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
    assert!(inspect(&case).await.is_err());
}
#[tokio::test]
async fn actual_pending_ledger_and_mailbox_history_cannot_disappear_from_pilot_scope() {
    let case = setup(false).await;
    insert(&case, "unknown").await;
    assert_eq!(
        inspect(&case).await.unwrap().disposition(),
        LedgerDisposition::NeedsReconciliation
    );
    let sender = aq_agent_api::digest(&json!({"kind":"session","binding":case.binding})).unwrap();
    let pool = case.f.db.pool().await;
    sqlx::query("INSERT INTO mailbox_operations(sender_key,operation_id,payload_digest,response) VALUES($1,$2,$3,'{}')").bind(sender).bind(Uuid::new_v4()).bind("b".repeat(64)).execute(&pool).await.unwrap();
    assert_eq!(
        inspect(&case).await.unwrap().disposition(),
        LedgerDisposition::UnsupportedMessaging
    );
    let case = setup(true).await;
    assert_eq!(
        inspect(&case).await.unwrap().disposition(),
        LedgerDisposition::UnsupportedMessaging
    );
}
#[tokio::test]
async fn actual_ledger_byte_bound_refuses_before_payload_fetch() {
    let case = setup(false).await;
    let op = insert(&case, "confirmed").await;
    let pool = case.f.db.pool().await;
    // Explicit corruption/oversize fixture, not accepted mutation admission.
    sqlx::query("UPDATE agent_graph_operations SET mapped=jsonb_set(mapped,'{synthetic_oversize}',to_jsonb(repeat('x',33554432))) WHERE operation_id=$1").bind(op).execute(&pool).await.unwrap();
    assert!(inspect(&case).await.is_err());
}
