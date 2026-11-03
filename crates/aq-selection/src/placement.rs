//! Sandbox placement facts (C4): template pin, backend capabilities, node
//! ceilings and placement preference. The server derives them from the node's
//! authenticated B1 advertisement and the catalog's activation-time template
//! resolution; selection only turns them into typed refusals.
use crate::candidate_core::{CandidateResult, profile_key};
use crate::{BackendKind, CandidateState, Digest, InstalledHarness};
use serde::{Deserialize, Serialize};

/// Present only for nodes that registered a sandbox backend advertisement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    /// Advertised backend capabilities (kebab-case B1 names).
    pub capabilities: Vec<String>,
    /// Capabilities the project requires; any missing one refuses.
    pub required_capabilities: Vec<String>,
    pub templates: Vec<AdvertisedTemplate>,
    /// Activation-time `Execution.environments` entry -> template digest.
    pub environment_templates: Vec<EnvironmentTemplate>,
    pub ceiling: Option<NodeCeiling>,
    /// A higher-preference eligible backend is online with free capacity.
    pub preferred_peer: Option<BackendKind>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdvertisedTemplate {
    pub digest: Digest,
    pub harnesses: Vec<AdvertisedHarness>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AdvertisedHarness {
    pub harness: String,
    pub version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentTemplate {
    pub environment: String,
    pub template: Digest,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeCeiling {
    Concurrency,
    Spend,
    /// A spend ceiling is configured but no comparable spend is accounted.
    SpendUnaccounted,
}
impl NodeCeiling {
    pub fn reason(self) -> &'static str {
        match self {
            NodeCeiling::Concurrency => "node_concurrency_ceiling_reached",
            NodeCeiling::Spend => "node_spend_ceiling_reached",
            NodeCeiling::SpendUnaccounted => "node_spend_unaccounted",
        }
    }
}

pub(crate) fn check(placement: &Placement, harness: &InstalledHarness) -> CandidateResult {
    if let Some(ceiling) = placement.ceiling {
        return Err((CandidateState::Unavailable, ceiling.reason()));
    }
    if placement.preferred_peer.is_some() {
        return Err((
            CandidateState::Unavailable,
            "placement_preferred_backend_available",
        ));
    }
    let unsupported = |reason| Err((CandidateState::Unsupported, reason));
    if placement
        .required_capabilities
        .iter()
        .any(|c| !placement.capabilities.contains(c))
    {
        return unsupported("required_backend_capability_unsupported");
    }
    let key = profile_key(&harness.environment);
    let Some(resolved) = placement
        .environment_templates
        .iter()
        .find(|t| t.environment == key)
        .or_else(|| {
            placement
                .environment_templates
                .iter()
                .find(|t| t.environment == harness.environment.id.as_str())
        })
    else {
        return unsupported("environment_template_unresolved");
    };
    if resolved.template != harness.environment.digest {
        return unsupported("environment_template_digest_mismatch");
    }
    let Some(template) = placement
        .templates
        .iter()
        .find(|t| t.digest == resolved.template)
    else {
        return unsupported("template_not_advertised");
    };
    if !template
        .harnesses
        .iter()
        .any(|h| h.harness == harness.harness && h.version == harness.cli_version)
    {
        return unsupported("template_missing_selected_harness_version");
    }
    Ok(())
}
