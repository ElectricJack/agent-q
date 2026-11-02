//! Synthetic pure input vectors only; these cannot construct an actual owner,
//! persisted eligibility, effect-disposition decision, release or Graph intent.
use super::*;
use aq_admission::{ArtifactKind, RetainedArtifact, Text};
use serde_json::{Value, json};
struct Synthetic {
    result: RecordedResult,
    source: RecordedSourceConfirmation,
    policy: PilotPolicy,
    review: VerifiedReviewEvidence,
    aq: AqEffectsSnapshot,
    external: VerifiedEffectDisposition,
    current: CurrentApplicability,
}
fn pin(id: &str, bytes: &[u8]) -> ArtifactRef {
    ArtifactRef {
        id: Text::new(id).unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(bytes),
    }
}
impl Synthetic {
    fn new() -> Self {
        let record: Value = serde_json::from_str(include_str!(
            "../../../../aq-review-protocol/artifacts/upstream-result-record-v1.fixture.json"
        ))
        .unwrap();
        let mut report: aq_admission::transport::ResultReport =
            serde_json::from_value(record["report"].clone()).unwrap();
        let mut receipt: aq_admission::result_record::Receipt =
            serde_json::from_value(record["receipt"].clone()).unwrap();
        let body =
            r#"{"schema_version":1,"kind":"source_change_reviewed_human_owner_no_checks_v1"}"#;
        let definition = crate::profiled_execution::definitions::Definition::new(
            pin("synthetic-pilot-policy", body.as_bytes()),
            ArtifactKind::Definition,
            body.into(),
        )
        .unwrap();
        let policy = PilotPolicy::from_definition(&definition).unwrap();
        let mut outcome: aq_admission::outcome::ExpectedOutcomeDefinition =
            serde_json::from_str(include_str!(
                "../../../../aq-review-protocol/artifacts/upstream-outcome-v1.fixture.json"
            ))
            .unwrap();
        outcome.required_checks.clear();
        outcome.validation_policy = policy.artifact.clone();
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../aq-admission/artifacts/execution-spec.fixture.json"
        ))
        .unwrap();
        let mut spec: aq_admission::execution_spec::ProfiledExecutionSpec =
            serde_json::from_value(fixture["spec"].clone()).unwrap();
        spec.lifecycle = aq_lifecycle_api::Binding {
            project_id: outcome.project_id,
            task_id: outcome.task.task_id,
            lineage_id: aq_admission::transport::task_lineage_id(
                outcome.project_id,
                outcome.task.workspace_id,
                outcome.task.task_id,
            )
            .unwrap(),
            attempt_id: report.binding.attempt_id,
            attempt_generation: report.binding.assignment_generation,
            session: report.binding.session.clone(),
        };
        spec.project_revision = outcome.project_revision;
        spec.task = outcome.task.clone();
        spec.source.project_id = outcome.project_id;
        spec.source.project_revision = outcome.project_revision;
        spec.source.repository_id = outcome.source.repository_id;
        spec.source.repository_binding_generation = outcome.source.binding_generation;
        spec.source.resolved_object = outcome.source.base.clone();
        spec.source_artifact = outcome.source.artifact.clone();
        spec.outcome_artifact = outcome.artifact().unwrap();
        spec.task_observation = outcome.task_observation.clone();
        spec.task_context = outcome.task_context.clone();
        outcome.validate_against(&spec).unwrap();
        report.binding.execution_spec_digest = spec.artifact().unwrap().digest;
        report.binding.graph_access_profile_digest = spec.graph_access_profile.digest.clone();
        report.stop.binding = report.binding.clone();
        aq_admission::result_record::validate_report(&report).unwrap();
        receipt.binding = report.binding.clone();
        receipt.expected_outcome = spec.outcome_artifact.clone();
        receipt.report_digest = aq_admission::result_record::report_digest(&report).unwrap();
        receipt.validate_for(&report).unwrap();
        let receipt_artifact = receipt.artifact(&report).unwrap();
        let outcome_descriptor = RetainedArtifact {
            artifact: spec.outcome_artifact.clone(),
            kind: ArtifactKind::Outcome,
            bytes: outcome.canonical_bytes().unwrap().len() as u64,
            retention_receipt: pin("synthetic-retained-outcome", b"fixture"),
            retained_at_ms: 1000,
            retain_until_ms: 2000,
        };
        let head_report_id = report.report_id;
        let result = RecordedResult {
            report,
            receipt,
            receipt_artifact,
            spec,
            outcome,
            outcome_descriptor,
            head_report_id,
            is_current_report_head: true,
        };
        let source_id = result.report.source_receipt_ids[0];
        let artifact = Uuid::from_u128(9001);
        let transfer = Uuid::from_u128(9002);
        let base = aq_worker_protocol::ObjectId {
            algorithm: aq_worker_protocol::ObjectAlgorithm::Sha1,
            hex: result.spec.source.resolved_object.hex.clone(),
        };
        let object = aq_worker_protocol::ObjectId {
            algorithm: aq_worker_protocol::ObjectAlgorithm::Sha1,
            hex: "2".repeat(40),
        };
        let aq_admission::SessionIdentity::Remote { session_id, .. } =
            result.report.binding.session
        else {
            unreachable!()
        };
        let manifest = aq_worker_protocol::TransferManifest {
            transfer_id: transfer,
            role: aq_worker_protocol::SourceRole::Checkpoint,
            checkpoint: object.clone(),
            attempt: aq_worker_protocol::AttemptRef {
                attempt_id: result.report.binding.attempt_id,
                session_id,
                generation: result.report.binding.assignment_generation,
            },
            resolved_spec_digest: result.report.binding.execution_spec_digest.as_str().into(),
            source: aq_worker_protocol::SourceBinding {
                repository_id: result.spec.source.repository_id,
                binding_generation: result.spec.source.repository_binding_generation,
                logical_identity: "synthetic-only".into(),
                base,
            },
            artifact_id: artifact,
            manifest_digest: "a".repeat(64),
            bundle_digest: None,
            manifest_bytes: 1,
            bundle_bytes: 0,
        };
        let source = RecordedSourceConfirmation {
            manifest,
            receipt: aq_worker_protocol::CentralRetentionReceipt {
                receipt_id: source_id,
                transfer_id: transfer,
                repository_id: result.spec.source.repository_id,
                binding_generation: result.spec.source.repository_binding_generation,
                source: object,
                retention_handle: artifact,
                manifest_digest: "b".repeat(64),
            },
            acknowledgement: json!({"synthetic":true}),
        };
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../aq-review-protocol/artifacts/fixtures.json"
        ))
        .unwrap();
        let mut subject: aq_review_protocol::ReviewSubject =
            serde_json::from_value(fixture["subject"].clone()).unwrap();
        subject.project_id = result.spec.lifecycle.project_id;
        subject.original_project_revision = result.spec.project_revision;
        subject.task = aq_review_protocol::TaskRevision {
            workspace_id: result.spec.task.workspace_id,
            task_id: result.spec.task.task_id,
            version: result.spec.task.version,
            content_revision: result.spec.task.content_revision,
        };
        subject.binding = result.report.binding.clone();
        subject.execution_spec = result.spec.artifact().unwrap();
        subject.expected_outcome = result.spec.outcome_artifact.clone();
        subject.task_observation = result.spec.task_observation.clone();
        subject.task_context = result.spec.task_context.clone();
        subject.validation_policy = policy.artifact.clone();
        subject.result = aq_review_protocol::ResultPin {
            report_id: result.report.report_id,
            report_digest: result.receipt.report_digest.clone(),
            record_receipt: result.receipt_artifact.clone(),
            stopped_receipt: result.receipt.stopped_receipt.clone(),
        };
        subject.source.repository_id = source.receipt.repository_id;
        subject.source.binding_generation = source.receipt.binding_generation;
        subject.source.base.hex = source.manifest.source.base.hex.clone();
        subject.source.resulting_object.hex = source.receipt.source.hex.clone();
        subject.source.input_artifact = result.spec.source_artifact.clone();
        subject.source.source_receipt_id = source_id;
        subject.source.resulting_artifact = ArtifactRef {
            id: Text::new(format!("aq.git-import-manifest/{artifact}")).unwrap(),
            version: Text::new("1").unwrap(),
            digest: serde_json::from_value(json!(source.receipt.manifest_digest)).unwrap(),
        };
        subject.source.retention_receipt = ArtifactRef {
            id: Text::new(format!("aq.git-import-receipt/{source_id}")).unwrap(),
            version: Text::new("1").unwrap(),
            digest: aq_review_protocol::digest(&source.acknowledgement).unwrap(),
        };
        subject.criteria = result
            .outcome
            .acceptance_criteria
            .iter()
            .map(|c| Text::new(c.id.as_str()).unwrap())
            .collect();
        subject.validate().unwrap();
        let review = VerifiedReviewEvidence {
            subject,
            receipt: pin("synthetic-code-review", b"approve"),
            approved: true,
        };
        let aq = AqEffectsSnapshot::synthetic(&result, LedgerDisposition::Settled);
        let external = VerifiedEffectDisposition {
            report: result.receipt_artifact.clone(),
            stopped: result.receipt.stopped_receipt.clone(),
            source: source_digest(&source).unwrap(),
            ledger: aq.digest().clone(),
            policy: policy.artifact.clone(),
            receipt: pin(
                "synthetic-separate-external-disposition",
                b"scoped human reconciliation",
            ),
        };
        let current = CurrentApplicability {
            report: result.receipt_artifact.clone(),
            source: source_digest(&source).unwrap(),
            cancelled: false,
            changed: false,
        };
        Self {
            result,
            source,
            policy,
            review,
            aq,
            external,
            current,
        }
    }
    fn evidence(&self) -> Evidence<'_> {
        Evidence {
            policy: Some(&self.policy),
            review: Some(&self.review),
            aq_effects: Some(&self.aq),
            external_effects: Some(&self.external),
            applicability: Some(&self.current),
        }
    }
    fn reason(&self) -> Option<Reason> {
        evaluate_pilot(&self.result, &self.source, &self.evidence())
            .unwrap()
            .reason
    }
}
#[test]
fn synthetic_pure_plan_requires_every_independent_owner_product() {
    let f = Synthetic::new();
    assert_eq!(f.reason(), None);
    assert!(f.result.report.pending_effect_ids.is_empty());
    let mut e = f.evidence();
    e.external_effects = None;
    assert_eq!(
        evaluate_pilot(&f.result, &f.source, &e).unwrap().reason,
        Some(Reason::ExternalDispositionUnavailable)
    );
    let mut e = f.evidence();
    e.applicability = None;
    assert_eq!(
        evaluate_pilot(&f.result, &f.source, &e).unwrap().reason,
        Some(Reason::ApplicabilityUnavailable)
    );
    let mut e = f.evidence();
    e.review = None;
    assert_eq!(
        evaluate_pilot(&f.result, &f.source, &e).unwrap().reason,
        Some(Reason::ReviewUnavailable)
    );
    let mut e = f.evidence();
    e.aq_effects = None;
    assert_eq!(
        evaluate_pilot(&f.result, &f.source, &e).unwrap().reason,
        Some(Reason::AqEffectsUnavailable)
    );
}
#[test]
fn synthetic_policy_and_revision_changes_never_inherit_approval() {
    let mut f = Synthetic::new();
    f.result.is_current_report_head = false;
    assert_eq!(f.reason(), Some(Reason::SupersededReport));
    let mut f = Synthetic::new();
    f.result
        .outcome
        .required_checks
        .push(aq_admission::outcome::RequiredCheck {
            id: aq_admission::Label::new("actual-tests-required").unwrap(),
            validator: pin("unsupported-check", b"not executed"),
        });
    assert_eq!(f.reason(), Some(Reason::RequiredChecksUnsupported));
    let mut f = Synthetic::new();
    f.result.outcome.review = ReviewRequirement::PolicyActor;
    assert_eq!(f.reason(), Some(Reason::UnsupportedOutcome));
    let mut f = Synthetic::new();
    f.policy.artifact.digest = Digest::of(b"new policy");
    assert_eq!(f.reason(), Some(Reason::UnsupportedPolicy));
    let mut f = Synthetic::new();
    f.review.subject.source.resulting_object.hex = "3".repeat(40);
    assert_eq!(f.reason(), Some(Reason::ReviewMismatch));
    let mut f = Synthetic::new();
    f.review.approved = false;
    assert_eq!(f.reason(), Some(Reason::ReviewRejected));
}
#[test]
fn synthetic_reconciliation_and_applicability_remain_bound_to_original_facts() {
    let mut f = Synthetic::new();
    f.aq = AqEffectsSnapshot::synthetic(&f.result, LedgerDisposition::NeedsReconciliation);
    assert_eq!(f.reason(), Some(Reason::AqEffectsUnsettled));
    let mut f = Synthetic::new();
    f.external.ledger = Digest::of(b"different exhaustive ledger");
    assert_eq!(f.reason(), Some(Reason::ExternalDispositionMismatch));
    let mut f = Synthetic::new();
    f.external.receipt = f.review.receipt.clone();
    assert_eq!(f.reason(), Some(Reason::ExternalDispositionMismatch));
    let mut f = Synthetic::new();
    f.current.cancelled = true;
    assert_eq!(f.reason(), Some(Reason::Cancelled));
    let mut f = Synthetic::new();
    f.current.changed = true;
    assert_eq!(f.reason(), Some(Reason::ChangedInputs));
}
