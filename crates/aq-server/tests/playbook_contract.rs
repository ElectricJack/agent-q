//! Independently retained public fixtures catch contract drift.
use aq_server::playbooks::domain::*;
use serde_json::{Value, json};
#[test]
fn registry_and_public_mailbox_dispositions_match_generated_profile() {
    let snapshot: Value = serde_json::from_str(include_str!(
        "../../../protocol/playbook-host/v1/registry.json"
    ))
    .unwrap();
    assert_eq!(snapshot["registry"], json!(registry()));
    assert_eq!(
        snapshot["source_revision"],
        "a463e5918f914c8365a04dfe1b6778831cfebb95"
    );
    let input: Value = serde_json::from_str(include_str!(
        "../../../protocol/playbook-host/v1/fixtures/message-send.json"
    ))
    .unwrap();
    message_contract().input.validate(&input).unwrap();
    serde_json::from_value::<MessageInput>(input.clone())
        .unwrap()
        .validate()
        .unwrap();
    let output: Value = serde_json::from_str(include_str!(
        "../../../protocol/playbook-host/v1/fixtures/queued.json"
    ))
    .unwrap();
    message_contract().outcomes["queued"]
        .validate(&output)
        .unwrap();
    assert_eq!(output["harness_delivery"], "not_attempted");
    let mut forged = input;
    forged["sender"] = json!({"kind":"owner"});
    assert!(message_contract().input.validate(&forged).is_err());
    assert!(serde_json::from_value::<MessageInput>(forged).is_err());
    let mut fabricated = output;
    fabricated["disposition"] = json!("delivered_to_harness");
    assert!(
        message_contract().outcomes["queued"]
            .validate(&fabricated)
            .is_err()
    );
}
#[test]
fn graph_read_capability_does_not_authorize_mail_and_exact_session_scope_does_not_retarget() {
    let input: MessageInput = serde_json::from_str(include_str!(
        "../../../protocol/playbook-host/v1/fixtures/message-send.json"
    ))
    .unwrap();
    let (w, p, t) = input.recipient.scope();
    let mut scope = GrantScope {
        workspace_id: w,
        project_id: p,
        recipients: vec![input.recipient.clone()],
        capabilities: std::collections::BTreeSet::from(["graph.read".into()]),
    };
    assert_eq!(scope.validate().unwrap_err(), Error::Invalid);
    scope.capabilities = std::collections::BTreeSet::from(["message.send".into()]);
    scope.validate().unwrap();
    assert!(!scope.permits(&Recipient::Session {
        workspace_id: w,
        project_id: p,
        task_id: t,
        session_id: uuid::Uuid::new_v4(),
        attempt_id: uuid::Uuid::new_v4(),
        generation: 1
    }));
}
