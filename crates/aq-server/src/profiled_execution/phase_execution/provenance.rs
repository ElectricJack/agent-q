#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Provenance {
    pub schema_version: u32,
    pub qualification: ArtifactRef,
    pub registration_id: uuid::Uuid,
    pub project: crate::project_bindings::domain::FrozenProjectBinding,
    pub health: crate::profiled_execution::renewal::HealthEvidence,
    pub graph: crate::profiled_execution::native_offer::observation::ReadProvenance,
    pub retention: crate::source_retention::profiled_base::RetentionObservation,
    pub inventory_request_digest: Digest,
    pub inventory_receipt_digest: Digest,
    pub inventory_compatibility_epoch: u64,
    pub availability: Vec<aq_selection::AvailabilityObservation>,
    pub private_digest: Digest,
    pub private_bytes: u64,
    pub private_started_at_ms: u64,
    pub private_received_at_ms: u64,
}
