//! Explicit new nominal fixture using the actual refresh owner APIs.
use aq_admission::preparation_refresh_v1 as phase;
use aq_server::profiled_execution::accepted_refresh as owner;
const POLICY_ID: &str = "aq.accepted-preparation-refresh-policy/synthetic-test";
const EXECUTION_ID: &str = "aq.phase-refresh-execution-qualification/test";
const ADAPTER_SET: &str = "registry_observation_continuity_current_project_principal_inventory_health_unissued_graph_retention_private_v1";
const CONTROLLED_OWNER_CLOCK: bool = true;
include!("phase_execution_core.rs");
