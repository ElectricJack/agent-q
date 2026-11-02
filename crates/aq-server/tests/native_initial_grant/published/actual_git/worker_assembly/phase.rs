//! Frozen nominal phase helper over actual old protocol records.
pub(super) use aq_admission::preparation_v2 as wire;
use aq_server::profiled_execution::{
    accepted_preparation::PolicyDefinition, phase_execution as phase_owner,
};
const POLICY_ID: &str = "aq.accepted-preparation-policy/actual-worker-test";
const EXECUTION_ID: &str = "aq.phase-execution-qualification/actual-worker-test";
const ADAPTER_SET: &str =
    "current_project_registry_principal_inventory_health_unissued_graph_retention_private_v1";
pub(super) const ACCEPTANCE: &str = "phase_acceptance";
pub(super) const PREPARATION: &str = "phase_preparation";
pub(super) const PREPARED: &str = "phase_prepared";
pub(super) const EXECUTION: &str = "phase_execution";
pub(super) const HOST_TERMINAL: &str = "terminal";
include!("phase_core.rs");
