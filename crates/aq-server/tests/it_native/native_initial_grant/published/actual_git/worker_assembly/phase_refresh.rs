//! Refresh nominal phase helper; no old DTO facade or history conversion.
pub(super) use aq_admission::preparation_refresh_v1 as wire;
use aq_server::profiled_execution::{
    accepted_refresh::PolicyDefinition, phase_refresh_execution as phase_owner,
};
const POLICY_ID: &str = "aq.accepted-preparation-refresh-policy/actual-worker0.4.0-test";
const EXECUTION_ID: &str = "aq.phase-refresh-execution-qualification/actual-worker0.4.0-test";
const ADAPTER_SET: &str = "registry_observation_continuity_current_project_principal_inventory_health_unissued_graph_retention_private_v1";
pub(super) const ACCEPTANCE: &str = "refresh_acceptance";
pub(super) const PREPARATION: &str = "refresh_preparation";
pub(super) const PREPARED: &str = "refresh_prepared";
pub(super) const EXECUTION: &str = "refresh_execution";
pub(super) const HOST_TERMINAL: &str = "refresh_terminal";
include!("phase_core.rs");
