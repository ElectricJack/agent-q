//! Explicit catalog2 addition. Catalog1 functions/fixture bytes remain fixed.
use super::domain::{Error, Recipient, Result};
use aq_playbook_host::{model::*, runner::Authority};
use aq_session_protocol::{ArtifactRef, Validate};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogProfile {
    Mail1,
    HumanReview2,
    /// D4 host catalog 1.0.0, vendored and fingerprint-pinned (D5).
    Host3,
}
pub const HOST3_VERSION: &str = "1.0.0";
pub const HOST3_FINGERPRINT: &str =
    "sha256:627125cc393d8274bfaa9ffc785c338e566cc69af362ac027a3daa355c3509e1";
pub const HOST3_CAPABILITIES: [&str; 4] = [
    "context.read",
    "work.ensure",
    "work.change",
    "operation.cancel",
];
fn host3_registry() -> Registry {
    static REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REGISTRY
        .get_or_init(|| {
            let registry: Registry = serde_json::from_str(include_str!(
                "../../../../vendor/aq-host-catalog-1.0.0/registry.json"
            ))
            .expect("vendored D4 host catalog parses");
            assert_eq!(
                aq_playbook_host::model::digest(&(HOST3_VERSION, &registry))
                    .ok()
                    .as_deref(),
                Some(HOST3_FINGERPRINT),
                "vendored D4 host catalog fingerprint"
            );
            registry
        })
        .clone()
}
impl CatalogProfile {
    pub fn number(self) -> i16 {
        match self {
            Self::Mail1 => 1,
            Self::HumanReview2 => 2,
            Self::Host3 => 3,
        }
    }
    pub fn registry(self) -> Registry {
        if self == Self::Host3 {
            return host3_registry();
        }
        let mut registry = super::domain::registry();
        if self == Self::HumanReview2 {
            let decision = decision_contract();
            let operation = operation_contract();
            let event = decision_event();
            registry.commands.insert(decision.name.clone(), decision);
            registry.commands.insert(operation.name.clone(), operation);
            registry.events.insert(event.name.clone(), event);
        }
        registry
    }
    pub fn validate_scope(self, scope: &super::domain::GrantScope) -> Result<()> {
        if self == Self::Mail1 {
            return scope.validate();
        }
        if self == Self::Host3 {
            let allowed: BTreeSet<String> =
                HOST3_CAPABILITIES.iter().map(|c| c.to_string()).collect();
            if scope.workspace_id.is_nil()
                || scope.project_id.is_nil()
                || !scope.recipients.is_empty()
                || scope.capabilities.is_empty()
                || !scope.capabilities.is_subset(&allowed)
                || aq_playbook_host::model::canonical_bytes(scope)
                    .map_err(|_| Error::Invalid)?
                    .len()
                    > 32768
            {
                return Err(Error::Invalid);
            }
            return Ok(());
        }
        let mut base = scope.clone();
        base.capabilities = BTreeSet::from(["message.send".into()]);
        base.validate()?;
        let allowed = BTreeSet::from([
            "message.send".into(),
            "decision.request".into(),
            "operation.get".into(),
        ]);
        if scope.capabilities.is_empty() || !scope.capabilities.is_subset(&allowed) {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn authority(
        self,
        id: Uuid,
        revision: i64,
        scope: &super::domain::GrantScope,
    ) -> Result<Authority> {
        self.validate_scope(scope)?;
        let mut authority = super::domain::authority(id, revision);
        authority.capabilities = scope.capabilities.clone();
        Ok(authority)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionInput {
    pub recipient: Recipient,
    pub report_id: Uuid,
    pub source_operation_id: Uuid,
    pub expected_subject: ArtifactRef,
    pub eligible_owners: Vec<Uuid>,
    pub eligibility_project_revision: u64,
    pub expires_at_ms: u64,
}
impl DecisionInput {
    pub fn validate(&self) -> Result<()> {
        self.recipient.validate()?;
        self.expected_subject
            .id
            .validate()
            .map_err(|_| Error::Invalid)?;
        self.expected_subject
            .version
            .validate()
            .map_err(|_| Error::Invalid)?;
        if self.report_id.is_nil()
            || self.source_operation_id.is_nil()
            || self.expected_subject.id.as_str() != format!("aq.review-subject/{}", self.report_id)
            || self.expected_subject.version.as_str() != "1"
            || self.eligibility_project_revision == 0
            || self.eligibility_project_revision > i64::MAX as u64
            || self.expires_at_ms == 0
            || self.expires_at_ms > i64::MAX as u64
            || self.eligible_owners.is_empty()
            || self.eligible_owners.len() > 16
            || self.eligible_owners.iter().any(Uuid::is_nil)
            || self.eligible_owners.iter().collect::<BTreeSet<_>>().len()
                != self.eligible_owners.len()
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
fn object(fields: impl IntoIterator<Item = (&'static str, Schema)>) -> Schema {
    Schema::object(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn fixed(value: &str) -> Schema {
    Schema::String {
        max_length: 64,
        values: BTreeSet::from([value.into()]),
    }
}
fn artifact() -> Schema {
    object([
        ("id", Schema::string(1024)),
        ("version", Schema::string(1024)),
        ("digest", Schema::string(64)),
    ])
}
fn output() -> Schema {
    object([("receipt", artifact()), ("subject", artifact())])
}
fn outcome() -> Schema {
    Schema::String {
        max_length: 16,
        values: BTreeSet::from([
            "approved".into(),
            "rejected".into(),
            "expired".into(),
            "cancelled".into(),
        ]),
    }
}
fn completion() -> Schema {
    object([("outcome", outcome()), ("output", output())])
}
pub fn decision_contract() -> CommandContract {
    CommandContract {
        name: "decision.request".into(),
        version: "1".into(),
        input: object([
            ("recipient", super::domain::recipient_schema()),
            ("report_id", Schema::string(36)),
            ("source_operation_id", Schema::string(36)),
            ("expected_subject", artifact()),
            (
                "eligible_owners",
                Schema::Array {
                    items: Box::new(Schema::string(36)),
                    max_items: 16,
                },
            ),
            (
                "eligibility_project_revision",
                Schema::Integer {
                    minimum: 1,
                    maximum: i64::MAX,
                },
            ),
            (
                "expires_at_ms",
                Schema::Integer {
                    minimum: 1,
                    maximum: i64::MAX,
                },
            ),
        ]),
        outcomes: BTreeMap::from([
            ("accepted".into(), object([("handle", Schema::string(36))])),
            ("approved".into(), output()),
            ("rejected".into(), output()),
            ("expired".into(), output()),
            ("cancelled".into(), output()),
        ]),
        capabilities: BTreeSet::from(["decision.request".into()]),
        scopes: BTreeSet::from(["project".into()]),
        effect: Effect::Mutation,
        recovery: Recovery::Reconcilable,
        asynchronous: Some(AsyncContract {
            completion_event: "aq.decision.completed".into(),
            completion: completion(),
            correlation_field: "decision".into(),
            reconciliation_command: "operation.get".into(),
        }),
        cancellation: true,
        public_result_fields: BTreeSet::new(),
        sensitive_input_fields: BTreeSet::from(["eligible_owners".into()]),
        sensitive_output_fields: BTreeSet::new(),
        max_payload_bytes: 65536,
        deadline_ms: 10000,
    }
}
pub fn operation_contract() -> CommandContract {
    CommandContract {
        name: "operation.get".into(),
        version: "1".into(),
        input: object([
            ("kind", fixed("human_decision")),
            ("handle", Schema::string(36)),
            ("recipient", super::domain::recipient_schema()),
        ]),
        outcomes: BTreeMap::from([
            (
                "pending".into(),
                object([("handle", Schema::string(36)), ("subject", artifact())]),
            ),
            ("approved".into(), output()),
            ("rejected".into(), output()),
            ("expired".into(), output()),
            ("cancelled".into(), output()),
        ]),
        capabilities: BTreeSet::from(["operation.get".into()]),
        scopes: BTreeSet::from(["project".into()]),
        effect: Effect::Observation,
        // Frozen host1.1 admits only reconcilable registrations. This is a
        // read-only retained owner lookup, conservatively reconciled by exact
        // handle rather than automatically retried as a generic observation.
        recovery: Recovery::Reconcilable,
        asynchronous: None,
        cancellation: false,
        public_result_fields: BTreeSet::new(),
        sensitive_input_fields: BTreeSet::new(),
        sensitive_output_fields: BTreeSet::new(),
        max_payload_bytes: 65536,
        deadline_ms: 5000,
    }
}
pub fn decision_event() -> EventContract {
    EventContract {
        name: "aq.decision.completed".into(),
        version: "1".into(),
        payload: completion(),
        trusted_producers: BTreeSet::from(["aq.human.decisions@1".into()]),
        scopes: BTreeSet::from(["project".into()]),
        correlation_fields: BTreeSet::from(["decision".into()]),
        subject_field: None,
        revision_field: None,
        max_payload_bytes: 16384,
        retention_ms: 86400000,
        sensitive_payload_fields: BTreeSet::new(),
    }
}
