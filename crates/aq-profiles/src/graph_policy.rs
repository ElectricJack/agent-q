//! Model-independent graph policy sealed from an instruction draft. A resolved
//! API01 profile is derived only after a concrete intelligence choice exists.
use crate::*;
use aq_session_api::{Permission, Source};
use aq_session_protocol::{ArtifactRef, Text};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphAccessPolicyKind {
    PreselectionGraphAccessPolicyV1,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeType {
    Blocks,
    #[serde(rename = "parent-child")]
    ParentChild,
    References,
    MotivatedBy,
    Produces,
    Supports,
    Contradicts,
    Supersedes,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphAccessScope {
    pub repository_id: Uuid,
    pub workspace_id: Uuid,
    pub task_id: Uuid,
    pub record_ids: Vec<Uuid>,
    pub edge_types: Vec<GraphEdgeType>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphAccessPolicy {
    pub schema_version: u32,
    pub kind: GraphAccessPolicyKind,
    pub project_binding_id: Uuid,
    pub project_revision: u64,
    pub source_context_revision: String,
    pub context_digest: String,
    pub instructions_digest: String,
    pub tool_policy_digest: String,
    pub effective_permissions: Vec<Permission>,
    pub scope: GraphAccessScope,
}
impl GraphAccessPolicy {
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.schema_version == 1
                && !self.project_binding_id.is_nil()
                && self.project_revision > 0,
            "graph policy identity",
        )?;
        text(&self.source_context_revision)?;
        for value in [
            &self.context_digest,
            &self.instructions_digest,
            &self.tool_policy_digest,
        ] {
            ensure(valid_digest(value), "graph policy digest")?;
        }
        ensure(
            !self.scope.repository_id.is_nil()
                && !self.scope.workspace_id.is_nil()
                && !self.scope.task_id.is_nil()
                && !self.scope.record_ids.is_empty()
                && self.scope.record_ids.len() <= 256
                && self.scope.record_ids.contains(&self.scope.task_id)
                && self.scope.record_ids.iter().all(|id| !id.is_nil())
                && self.scope.record_ids.iter().collect::<BTreeSet<_>>().len()
                    == self.scope.record_ids.len(),
            "graph policy record scope",
        )?;
        ensure(
            self.scope.edge_types.len() <= 8
                && self
                    .scope
                    .edge_types
                    .iter()
                    .map(|e| serde_json::to_string(e).unwrap())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == self.scope.edge_types.len(),
            "graph policy edge scope",
        )?;
        ensure(
            self.effective_permissions.len() <= 128,
            "graph policy permission bound",
        )?;
        let mut permissions = BTreeSet::new();
        for permission in &self.effective_permissions {
            text(&permission.scope_reference)?;
            let expected = if permission.operation == "source.write" {
                format!("repository:{}", self.scope.repository_id)
            } else {
                format!("project:{}", self.project_binding_id)
            };
            ensure(
                permission.scope_reference == expected,
                "graph policy permission scope does not name owned project/repository",
            )?;
            ensure(
                ["task.read", "knowledge.read", "source.write"]
                    .contains(&permission.operation.as_str())
                    && permissions.insert((&permission.operation, &permission.scope_reference)),
                "unsupported graph policy permission",
            )?;
        }
        Ok(())
    }
    pub fn artifact(&self) -> Result<ArtifactRef> {
        self.validate()?;
        let value = digest(self)?;
        Ok(ArtifactRef {
            id: Text::new(format!("aq.preselection-graph-access-policy/{value}"))
                .map_err(|_| Error::Invalid("graph policy artifact identity".into()))?,
            version: Text::new("1").unwrap(),
            digest: serde_json::from_value(serde_json::json!(value))
                .map_err(|_| Error::Invalid("graph policy artifact digest".into()))?,
        })
    }
}
pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionDraft {
    pub catalog_digest: String,
    pub agent_profile: Source,
    pub intelligence_profile: Source,
    pub execution_profile: Source,
    pub project_binding_id: Uuid,
    pub project_revision: u64,
    pub task_context_revision: String,
    pub harness: String,
    pub environment: String,
    pub instruction_order: Vec<Instruction>,
    pub requested_permissions: Vec<Permission>,
    pub effective_permissions: Vec<Permission>,
    pub context_limit_bytes: u64,
    pub overrides: Vec<aq_session_api::OverrideExplanation>,
    pub omissions: Vec<aq_session_api::Omission>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilationDraft {
    pub catalog_valid: bool,
    pub composition_valid: bool,
    pub blocking_reasons: Vec<String>,
    pub composition: Option<CompositionDraft>,
}
pub struct SealedComposition {
    pub composition: FrozenComposition,
    pub graph_access_policy: GraphAccessPolicy,
}
impl CompositionDraft {
    pub fn instructions_digest(&self) -> Result<String> {
        digest(&self.instruction_order)
    }
    pub fn tool_policy_digest(&self) -> Result<String> {
        digest(&(&self.requested_permissions, &self.effective_permissions))
    }
    /// Scope and context are owner-derived facts. This method provides no live
    /// project authorization, graph access or execution authority.
    pub fn seal(
        self,
        scope: GraphAccessScope,
        context: &serde_json::Value,
    ) -> Result<SealedComposition> {
        let policy = GraphAccessPolicy {
            schema_version: 1,
            kind: GraphAccessPolicyKind::PreselectionGraphAccessPolicyV1,
            project_binding_id: self.project_binding_id,
            project_revision: self.project_revision,
            source_context_revision: self.task_context_revision.clone(),
            context_digest: digest(context)?,
            instructions_digest: self.instructions_digest()?,
            tool_policy_digest: self.tool_policy_digest()?,
            effective_permissions: self.effective_permissions.clone(),
            scope,
        };
        policy.validate_context(context)?;
        let composition = FrozenComposition {
            schema_version: 2,
            catalog_digest: self.catalog_digest,
            agent_profile: self.agent_profile,
            intelligence_profile: self.intelligence_profile,
            execution_profile: self.execution_profile,
            project_binding_id: self.project_binding_id,
            project_revision: self.project_revision,
            task_context_revision: self.task_context_revision,
            graph_access_profile_digest: String::new(),
            requested_permissions: Some(self.requested_permissions),
            graph_access_policy_digest: Some(digest(&policy)?),
            graph_access_policy_kind: Some(policy.kind),
            harness: self.harness,
            environment: self.environment,
            instruction_order: self.instruction_order,
            effective_permissions: self.effective_permissions,
            context_limit_bytes: self.context_limit_bytes,
            overrides: self.overrides,
            omissions: self.omissions,
        };
        composition.validate_graph_policy(&policy)?;
        Ok(SealedComposition {
            composition,
            graph_access_policy: policy,
        })
    }
}
impl FrozenComposition {
    pub fn validate_native_v2(&self) -> Result<()> {
        ensure(
            self.schema_version == 2
                && self.graph_access_profile_digest.is_empty()
                && self
                    .requested_permissions
                    .as_ref()
                    .is_some_and(|p| p.len() <= 128)
                && self.graph_access_policy_kind
                    == Some(GraphAccessPolicyKind::PreselectionGraphAccessPolicyV1)
                && self
                    .graph_access_policy_digest
                    .as_deref()
                    .is_some_and(valid_digest),
            "native composition requires graph policy schema2",
        )?;
        let requested = self
            .requested_permissions
            .as_ref()
            .expect("validated requested permissions");
        let mut seen = BTreeSet::new();
        for permission in requested {
            text(&permission.operation)?;
            text(&permission.scope_reference)?;
            ensure(
                seen.insert((&permission.operation, &permission.scope_reference)),
                "duplicate requested permission",
            )?;
        }
        ensure(
            self.effective_permissions.len() <= 128 && self.instruction_order.len() <= 128,
            "native composition bounds",
        )?;
        seen.clear();
        for permission in &self.effective_permissions {
            ensure(
                requested.iter().any(|p| {
                    p.operation == permission.operation
                        && p.scope_reference == permission.scope_reference
                }) && seen.insert((&permission.operation, &permission.scope_reference)),
                "effective permission outside requested plan",
            )?;
        }
        let mut bytes = 0usize;
        for instruction in &self.instruction_order {
            text(&instruction.source.identity)?;
            text(&instruction.source.revision)?;
            bytes = bytes
                .checked_add(instruction.text.len())
                .ok_or_else(|| Error::Invalid("instruction aggregate overflow".into()))?;
            ensure(
                !instruction.text.trim().is_empty()
                    && instruction.text.len() <= 65536
                    && bytes <= 8_388_608
                    && valid_digest(&instruction.source.digest)
                    && ((instruction.source.identity.starts_with("module:")
                        || instruction.source.identity.starts_with("agent:"))
                        || instruction.source.digest
                            == aq_session_protocol::Digest::of(instruction.text.as_bytes())
                                .as_str())
                    && (!instruction.required || instruction.delivery == Delivery::Startup),
                "native instruction provenance/bounds",
            )?;
        }
        Ok(())
    }
    pub fn validate_graph_policy(&self, policy: &GraphAccessPolicy) -> Result<()> {
        self.validate_native_v2()?;
        policy.validate()?;
        ensure(
            self.graph_access_policy_digest.as_ref() == Some(&digest(policy)?)
                && policy.project_binding_id == self.project_binding_id
                && policy.project_revision == self.project_revision
                && policy.source_context_revision == self.task_context_revision
                && policy.instructions_digest == digest(&self.instruction_order)?
                && policy.tool_policy_digest
                    == digest(&(
                        self.requested_permissions.as_ref().ok_or_else(|| {
                            Error::Invalid("missing requested permissions".into())
                        })?,
                        &self.effective_permissions,
                    ))?
                && serde_json::to_value(&policy.effective_permissions).unwrap()
                    == serde_json::to_value(&self.effective_permissions).unwrap(),
            "composition graph policy binding",
        )
    }
}

/// Supplied only after an actual concrete intelligence choice exists. These
/// values are not availability, authentication or execution authority.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedGraphModel {
    pub provider: String,
    pub model: String,
    pub native_reasoning: String,
    pub reasoning_configuration_reference: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRetryBudget {
    pub max_attempts: u8,
    pub max_elapsed_ms: u64,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}
impl GraphAccessPolicy {
    /// Current native mode grants the finite observed record projection for
    /// task/knowledge reads. Source writes remain a separate repository grant.
    pub fn validate_context(&self, context: &serde_json::Value) -> Result<()> {
        self.validate()?;
        ensure(
            aq_agent_api::valid("ContextSnapshot", context)
                && digest(context)? == self.context_digest,
            "graph policy context digest/shape",
        )?;
        ensure(
            context["root_ids"] == serde_json::json!([self.scope.task_id])
                && context["coverage"]["status"] == "complete",
            "graph policy required context coverage",
        )?;
        let mut actual = BTreeSet::new();
        for key in ["nodes", "pinned_revisions"] {
            for node in context[key].as_array().expect("validated context") {
                let operation = if node["kind"] == "task" {
                    "task.read"
                } else {
                    "knowledge.read"
                };
                let resource = format!("project:{}", self.project_binding_id);
                ensure(
                    self.effective_permissions
                        .iter()
                        .any(|p| p.operation == operation && p.scope_reference == resource),
                    "required context record lacks typed read permission",
                )?;
                actual.insert(node["id"].as_str().expect("validated UUID").to_string());
            }
        }
        ensure(
            actual.contains(&self.scope.task_id.to_string())
                && actual == self.scope.record_ids.iter().map(Uuid::to_string).collect(),
            "graph policy exact context record scope",
        )?;
        let allowed = self
            .scope
            .edge_types
            .iter()
            .map(|e| serde_json::to_value(e).unwrap())
            .collect::<Vec<_>>();
        for edge in context["edges"].as_array().expect("validated context") {
            ensure(
                actual.contains(edge["source"].as_str().unwrap())
                    && actual.contains(edge["target"].as_str().unwrap())
                    && allowed.contains(&edge["edge_type"]),
                "graph policy context edge scope",
            )?;
        }
        Ok(())
    }
    pub fn resolve_profile(
        &self,
        model: &ResolvedGraphModel,
        context: &serde_json::Value,
        role: &str,
        retry: &GraphRetryBudget,
    ) -> Result<serde_json::Value> {
        self.validate_context(context)?;
        for value in [
            &model.provider,
            &model.model,
            &model.native_reasoning,
            &model.reasoning_configuration_reference,
        ] {
            text(value)?;
        }
        let readable = self
            .effective_permissions
            .iter()
            .any(|p| ["task.read", "knowledge.read"].contains(&p.operation.as_str()));
        let capabilities = if readable {
            vec![
                "read.get",
                "read.revision",
                "read.history",
                "read.search",
                "read.context",
                "read.relationships",
            ]
        } else {
            vec![]
        };
        let policy_digest = digest(self)?;
        let profile = serde_json::json!({
            "profile_id":format!("native/{}",digest(&(&policy_digest,model,role,retry))?),"revision":self.project_revision.to_string(),"role":role,
            "model":{"provider":model.provider,"model":model.model,"configuration_digest":digest(model)?},
            "instructions_digest":self.instructions_digest,"tool_policy_digest":self.tool_policy_digest,"capabilities":capabilities,
            "read_scope":{"mode":"records","record_ids":self.scope.record_ids,"include_created_by_attempt":false,"edge_types":self.scope.edge_types},
            "write_scope":{"record_ids":[],"relationship_types":[],"correct_fields":[],"max_created_records":0,"max_proposal_tasks":0},
            "retry_budget":retry,"max_response_bytes":1_048_576,
        });
        ensure(
            aq_agent_api::valid("ResolvedProfile", &profile),
            "resolved API01 profile bounds",
        )?;
        Ok(profile)
    }
}
