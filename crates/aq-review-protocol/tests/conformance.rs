use aq_review_protocol::*;
use serde_json::{Value, json};
use uuid::Uuid;
#[path = "../examples/support/mod.rs"]
mod support;
fn fixtures() -> Value {
    serde_json::from_str(include_str!("../artifacts/fixtures.json")).unwrap()
}
fn subject() -> ReviewSubject {
    serde_json::from_value(fixtures()["subject"].clone()).unwrap()
}
fn request() -> DecisionRequest {
    serde_json::from_value(fixtures()["decision_request"].clone()).unwrap()
}
fn receipt() -> DecisionReceipt {
    serde_json::from_value(fixtures()["decision_receipt"].clone()).unwrap()
}
fn check() -> CheckReceipt {
    serde_json::from_value(fixtures()["check_receipt"].clone()).unwrap()
}
#[test]
fn generated_schemas_and_synthetic_fixture_bytes_do_not_drift() {
    assert_eq!(support::fixtures(), fixtures());
    for (expected, current) in [
        (
            include_str!("../artifacts/subject.schema.json"),
            subject_schema(),
        ),
        (
            include_str!("../artifacts/decision.schema.json"),
            decision_schema(),
        ),
        (
            include_str!("../artifacts/check.schema.json"),
            check_schema(),
        ),
    ] {
        assert_eq!(serde_json::from_str::<Value>(expected).unwrap(), current);
    }
    subject().validate().unwrap();
    request().validate().unwrap();
    receipt().validate_for(&request()).unwrap();
    check().validate_for_subject(&subject()).unwrap();
}
#[test]
fn result_record_v1_remains_a_recording_ack_not_human_approval() {
    let upstream: Value = serde_json::from_str(include_str!(
        "../artifacts/upstream-result-record-v1.fixture.json"
    ))
    .unwrap();
    let s = subject();
    assert_eq!(upstream["receipt"]["disposition"], "recorded");
    assert_eq!(json!(s.result.report_id), upstream["receipt"]["report_id"]);
    assert_eq!(
        s.result.record_receipt.digest,
        digest(&upstream["receipt"]).unwrap()
    );
    assert_eq!(s.result.report_digest, digest(&upstream["report"]).unwrap());
    assert!(serde_json::from_value::<DecisionReceipt>(upstream["receipt"].clone()).is_err());
}
#[test]
fn every_changed_task_source_report_and_policy_pin_changes_the_exact_subject() {
    let original = subject();
    let artifact = original.artifact().unwrap();
    for kind in 0..7 {
        let mut changed = original.clone();
        match kind {
            0 => changed.task.version += 1,
            1 => changed.task.content_revision += 1,
            2 => changed.source.resulting_object.hex = "3".repeat(40),
            3 => changed.result.report_digest = Digest::of(b"different report"),
            4 => changed.validation_policy.digest = Digest::of(b"different policy"),
            5 => changed.original_project_revision += 1,
            _ => changed.result.record_receipt.digest = Digest::of(b"different owner receipt"),
        };
        assert_ne!(changed.artifact().unwrap(), artifact);
        let mut new = request();
        new.subject = changed.artifact().unwrap();
        assert!(receipt().validate_for(&new).is_err());
        assert!(check().validate_for_subject(&changed).is_err());
    }
}
#[test]
fn binding_and_reference_contradictions_refuse_before_any_authority_claim() {
    let mut s = subject();
    if let SessionIdentity::Remote {
        assignment_generation,
        ..
    } = &mut s.binding.session
    {
        *assignment_generation += 1;
    }
    assert!(s.validate().is_err());
    let mut s = subject();
    s.execution_spec.digest = Digest::of(b"wrong spec");
    assert!(s.validate().is_err());
    let mut s = subject();
    s.result.record_receipt.id =
        Text::new(format!("aq.result-record/{}", Uuid::from_u128(99))).unwrap();
    assert!(s.validate().is_err());
    let mut s = subject();
    s.result.stopped_receipt.id = Text::new("aq.native-stopped/other-attempt").unwrap();
    assert!(s.validate().is_err());
    let mut s = subject();
    s.task.content_revision = 0;
    assert!(s.validate().is_err());
}
#[test]
fn source_requires_canonical_object_identity_and_explicit_owner_references() {
    let mut s = subject();
    s.source.resulting_object.hex = "A".repeat(40);
    assert!(s.validate().is_err());
    let mut s = subject();
    s.source.resulting_object = ObjectId {
        format: ObjectFormat::Sha256,
        hex: "a".repeat(64),
    };
    assert!(s.validate().is_err());
    let mut s = subject();
    s.source.source_receipt_id = Uuid::nil();
    assert!(s.validate().is_err());
    let mut value = json!(subject());
    value["source"]["confirmed"] = json!(true);
    assert!(serde_json::from_value::<ReviewSubject>(value).is_err());
}
#[test]
fn subject_and_answer_bounds_are_bytes_and_distinct_identities() {
    let mut s = subject();
    s.criteria = vec![Text::new("one").unwrap(); 2];
    assert!(s.validate().is_err());
    let mut s = subject();
    s.criteria = (0..33)
        .map(|n| Text::new(format!("criterion{n}")).unwrap())
        .collect();
    assert!(s.validate().is_err());
    let mut r = request();
    r.eligible_owners = vec![Uuid::from_u128(1); 17];
    assert!(r.validate().is_err());
    let mut r = request();
    r.eligible_owners.push(r.eligible_owners[0]);
    assert!(r.validate().is_err());
    let mut resolve: ResolveRequest =
        serde_json::from_value(fixtures()["resolve_request"].clone()).unwrap();
    resolve.comment = Some("é".repeat(1025));
    assert!(resolve.validate().is_err());
    assert!(decode::<ReviewSubject>(&vec![b' '; MAX_BYTES + 1]).is_err());
    assert!(canonical_bytes(&"x".repeat(MAX_BYTES + 1)).is_err());
}
#[test]
fn machine_or_expired_variants_cannot_smuggle_a_human_owner_or_approval() {
    let mut value = json!(receipt());
    value["finality"] = json!({"kind":"machine_assessment","choice":"approve","owner_user_id":Uuid::from_u128(903)});
    assert!(serde_json::from_value::<DecisionReceipt>(value).is_err());
    let mut value = json!(receipt());
    value["finality"] = json!({"kind":"expired","owner_user_id":Uuid::from_u128(903)});
    assert!(serde_json::from_value::<DecisionReceipt>(value).is_err());
    let mut r = receipt();
    if let DecisionFinality::HumanAnswer { owner_user_id, .. } = &mut r.finality {
        *owner_user_id = Uuid::nil();
    }
    assert!(r.validate().is_err());
    let mut value = json!(request());
    value["eligible"] = json!(true);
    assert!(serde_json::from_value::<DecisionRequest>(value).is_err());
}
#[test]
fn answers_pin_original_eligible_set_and_deadline_without_granting_current_role() {
    let q = request();
    let mut r = receipt();
    if let DecisionFinality::HumanAnswer { owner_user_id, .. } = &mut r.finality {
        *owner_user_id = Uuid::from_u128(999);
    }
    assert!(r.validate_for(&q).is_err());
    let mut r = receipt();
    r.recorded_at_ms = q.expires_at_ms;
    assert!(r.validate_for(&q).is_err());
    let mut r = receipt();
    r.finality = DecisionFinality::Expired {};
    r.recorded_at_ms = q.expires_at_ms - 1;
    assert!(r.validate_for(&q).is_err());
    r.recorded_at_ms = q.expires_at_ms;
    r.validate_for(&q).unwrap();
    let mut r = receipt();
    r.subject.digest = Digest::of(b"changed source subject");
    assert!(r.validate_for(&q).is_err());
}
#[test]
fn incomplete_or_wrong_validator_evidence_cannot_be_passed() {
    let mut r = check();
    r.disposition = CheckDisposition::Passed;
    assert!(r.validate().is_err());
    let mut r = check();
    r.validator.digest = Digest::of(b"different validator");
    assert!(r.validate().is_err());
    let mut r = check();
    r.executor = ExecutorEvidence::InProcessReadOnly {
        definition: r.validator.clone(),
        checked_artifacts: vec![],
        result_artifact: r.subject.clone(),
    };
    assert!(r.validate().is_err());
}
#[test]
fn passed_process_needs_zero_exit_and_exact_binding_shape() {
    let mut r = check();
    let s = subject();
    r.disposition = CheckDisposition::Passed;
    r.executor = ExecutorEvidence::StoppedProcess {
        definition: r.validator.clone(),
        binding: Box::new(s.binding.clone()),
        started_receipt: s.result.stopped_receipt.clone(),
        stopped_receipt: s.result.stopped_receipt.clone(),
        result_artifacts: vec![],
        exit_code: 1,
    };
    assert!(r.validate().is_err());
    if let ExecutorEvidence::StoppedProcess {
        exit_code, binding, ..
    } = &mut r.executor
    {
        *exit_code = 0;
        binding.assignment_generation = 0;
    }
    assert!(r.validate().is_err());
}
#[test]
fn check_identity_cannot_merge_two_revisions_of_the_same_artifact_slot() {
    let mut r = check();
    let a = r.subject.clone();
    let mut b = a.clone();
    b.digest = Digest::of(b"new bytes same ID/version");
    r.executor = ExecutorEvidence::InProcessReadOnly {
        definition: r.validator.clone(),
        checked_artifacts: vec![a, b],
        result_artifact: r.subject.clone(),
    };
    assert!(r.validate().is_err());
}
