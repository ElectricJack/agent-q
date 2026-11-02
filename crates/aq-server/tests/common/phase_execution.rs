//! Frozen phase fixture: no refresh DTO conversion or profile inference.
use aq_admission::preparation_v2 as phase;
use aq_server::profiled_execution::accepted_preparation as owner;
const POLICY_ID: &str = "aq.accepted-preparation-policy/synthetic-test";
const EXECUTION_ID: &str = "aq.phase-execution-qualification/test";
const ADAPTER_SET: &str =
    "current_project_registry_principal_inventory_health_unissued_graph_retention_private_v1";
const CONTROLLED_OWNER_CLOCK: bool = false;
include!("phase_execution_core.rs");
