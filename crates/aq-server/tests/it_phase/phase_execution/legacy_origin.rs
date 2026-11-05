//! An actually issued phase receipt must not become legacy renewal authority.
use super::*;
#[tokio::test]
async fn actual_phase_grant_remains_held_when_legacy_renewal_refuses_its_origin() {
    let f = prepared(true).await;
    let auth = auth(&f.c).await;
    let initial = phase_execution::prepare(&f.c.env.env.state, &auth, &f.prepared, Uuid::new_v4())
        .await
        .unwrap();
    let request = aq_admission::transport::Renew {
        schema_version: 1,
        operation_id: Uuid::new_v4(),
        identity: f.prepared.original.identity.clone(),
        binding: f.prepared.original.binding.clone(),
        grant_id: initial.receipt.grant.grant_id,
        reservation_id: f.prepared.original.reservation.reservation_id,
        guardian_id: f.prepared.original.guardian_id,
        launch_id: f.prepared.original.launch_id,
        journal_revision: f.prepared.original.journal_revision + 1,
    };
    aq_admission::transport::TransportDocument::Renew(request.clone())
        .validate()
        .unwrap();
    let before = snapshot(&f.c).await;
    let result = aq_server::profiled_execution::renewal::renew(
        &f.c.env.env.state,
        &auth,
        &request,
        Uuid::new_v4(),
    )
    .await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("phase receipt unexpectedly renewed through legacy owner"),
    };
    assert!(
        matches!(error, aq_server::error::AppError::Conflict(ref message) if message.contains("protocol origin is not supported")),
        "{error:?}"
    );
    assert_eq!(before, snapshot(&f.c).await);
    assert_eq!(before["held"], true);
    assert_eq!(before["phase_receipts"], 1);
    assert_eq!(before["renewals"], 0);
}
async fn snapshot(c: &Case) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('hot',(SELECT to_jsonb(h) FROM profiled_authority_hot h),'journal',(SELECT to_jsonb(j) FROM profiled_owner_journals j),'phase_receipts',(SELECT count(*) FROM profiled_phase_execution_grants),'renewals',(SELECT count(*) FROM profiled_execution_receipts WHERE kind='renew'),'held',(SELECT holds_task FROM execution_attempts))")
        .fetch_one(&c.env.env.pool).await.unwrap()
}
