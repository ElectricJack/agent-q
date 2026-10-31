//! Retained API schemas share the standalone client validator.
use aq_agent_api::check;
pub(super) use aq_agent_api::{StrictValue, valid};
use serde_json::Value;
use std::sync::OnceLock;
pub(super) fn upstream_valid(name: &str, value: &Value) -> bool {
    static UPSTREAM: OnceLock<Value> = OnceLock::new();
    let root = UPSTREAM.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../protocol/agent-access/v1/upstream/aq-graph-openapi.json"
        ))
        .expect("retained upstream schema")
    });
    check(root, &root["components"]["schemas"][name], value)
}
