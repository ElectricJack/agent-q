use crate::*;
use aq_session_api::{
    Candidate, ContractDocument, Eligibility, Omission, OverrideExplanation, Permission,
    ProfileExplanation, ProfileRequest, Selection, Source,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ceilings {
    pub installation: Vec<Permission>,
    pub project: Vec<Permission>,
    pub execution: Vec<Permission>,
    pub caller: Vec<Permission>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextInput {
    pub source: Source,
    pub text: String,
    pub delivery: Delivery,
    pub required: bool,
    /// Smaller values are omitted first; ties preserve input order.
    pub omission_priority: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub field: String,
    pub value: u64,
    pub source: Source,
    pub reason_code: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompileRequest {
    pub profile: ProfileRequest,
    pub ceilings: Ceilings,
    pub available_tools: Vec<String>,
    pub available_adapters: Vec<String>,
    pub harness: String,
    pub environment: String,
    pub platform: ContextInput,
    pub project: ContextInput,
    pub assignment: ContextInput,
    pub context: Vec<ContextInput>,
    pub overrides: Vec<Override>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instruction {
    pub source: Source,
    pub delivery: Delivery,
    pub required: bool,
    pub text: String,
}
/// Frozen pre-selection composition, not a runnable worker LaunchManifest.
/// AQ-20 must seal concrete model/observation/budget/authority bindings later.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenComposition {
    pub schema_version: u32,
    pub catalog_digest: String,
    pub agent_profile: Source,
    pub intelligence_profile: Source,
    pub execution_profile: Source,
    pub project_binding_id: uuid::Uuid,
    pub project_revision: u64,
    pub task_context_revision: String,
    pub graph_access_profile_digest: String,
    pub harness: String,
    pub environment: String,
    pub instruction_order: Vec<Instruction>,
    pub effective_permissions: Vec<Permission>,
    pub context_limit_bytes: u64,
    pub overrides: Vec<OverrideExplanation>,
    pub omissions: Vec<Omission>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compilation {
    pub catalog_valid: bool,
    pub composition_valid: bool,
    pub selection_pending: bool,
    pub blocking_reasons: Vec<String>,
    pub composition_digest: Option<String>,
    pub composition: Option<FrozenComposition>,
    pub explanation: ProfileExplanation,
}
fn same(a: &Permission, b: &Permission) -> bool {
    a.operation == b.operation && a.scope_reference == b.scope_reference
}
fn contains(list: &[Permission], p: &Permission) -> bool {
    list.iter().any(|q| same(p, q))
}
fn add(list: &mut Vec<Permission>, p: &Permission) {
    if !contains(list, p) {
        list.push(p.clone());
    }
}
fn source_valid(s: &Source) -> Result<()> {
    text(&s.identity)?;
    text(&s.revision)?;
    ensure(
        s.digest.len() == 64
            && s.digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "context source digest",
    )
}
fn context_valid(c: &ContextInput) -> Result<()> {
    source_valid(&c.source)?;
    ensure(
        !c.text.trim().is_empty() && c.text.len() <= MAX_INSTRUCTION_BYTES,
        "context byte bounds",
    )?;
    ensure(
        aq_session_protocol::Digest::of(c.text.as_bytes()).as_str() == c.source.digest,
        "context bytes do not match source digest",
    )?;
    ensure(
        !c.required || c.delivery == Delivery::Startup,
        "required context cannot be on-demand",
    )
}
pub fn compile(
    catalog: &ValidatedCatalog,
    request: &CompileRequest,
    graph_access_profile_digest: &str,
) -> Result<Compilation> {
    ensure(
        request.profile.catalog_digest == catalog.digest(),
        "request catalog digest mismatch",
    )?;
    aq_session_api::decode(
        &serde_json::to_vec(&ContractDocument::ProfileExplain(request.profile.clone())).unwrap(),
    )
    .map_err(Error::Invalid)?;
    ensure(
        request.context.len() <= 128
            && request.overrides.len() <= 16
            && request.available_tools.len() <= 128
            && request.available_adapters.len() <= 128,
        "compile request cardinality",
    )?;
    for list in [
        &request.ceilings.installation,
        &request.ceilings.project,
        &request.ceilings.execution,
        &request.ceilings.caller,
    ] {
        ensure(list.len() <= 128, "ceiling bound")?;
        let mut seen = BTreeSet::new();
        for p in list {
            text(&p.operation)?;
            text(&p.scope_reference)?;
            ensure(
                seen.insert((&p.operation, &p.scope_reference)),
                "duplicate ceiling",
            )?;
        }
    }
    for list in [&request.available_tools, &request.available_adapters] {
        let mut seen = BTreeSet::new();
        for value in list {
            text(value)?;
            ensure(seen.insert(value), "duplicate available capability")?;
        }
    }
    text(&request.harness)?;
    text(&request.environment)?;
    let a = &catalog.artifact().catalog;
    let agent = a
        .agents
        .iter()
        .find(|a| key(&a.id, a.version) == request.profile.agent_profile)
        .ok_or_else(|| Error::Invalid("unknown agent recipe".into()))?;
    let execution = a
        .execution
        .iter()
        .find(|e| key(&e.id, e.version) == agent.execution)
        .unwrap();
    let intelligence = a
        .intelligence
        .iter()
        .find(|i| key(&i.id, i.version) == agent.intelligence)
        .unwrap();
    let mut limit = execution.context_limit_bytes;
    let mut overrides = Vec::new();
    let mut override_fields = BTreeSet::new();
    for o in &request.overrides {
        source_valid(&o.source)?;
        text(&o.reason_code)?;
        ensure(override_fields.insert(&o.field), "duplicate override")?;
        ensure(
            agent.allowed_overrides.contains(&o.field) && o.field == "context_limit_bytes",
            "protected or undeclared override",
        )?;
        ensure(
            o.value > 0 && o.value <= limit,
            "override cannot broaden context ceiling",
        )?;
        limit = o.value;
        overrides.push(OverrideExplanation {
            field: o.field.clone(),
            source: o.source.clone(),
            reason_code: o.reason_code.clone(),
        });
    }
    let mut blocking = Vec::new();
    if !execution.harnesses.contains(&request.harness)
        || !execution.environments.contains(&request.environment)
    {
        blocking.push("unsupported harness/environment".into());
    }
    if !request.available_adapters.contains(&request.harness) {
        blocking.push(format!("unavailable adapter:{}", request.harness));
    }
    if !intelligence
        .candidates
        .iter()
        .any(|c| c.harnesses.contains(&request.harness))
    {
        blocking.push("no compatible intelligence candidate".into());
    }
    let mut roots = agent.behaviors.clone();
    roots.extend(agent.capabilities.clone());
    let topological = expand(catalog.artifact(), &roots)?;
    let modules: BTreeMap<_, _> = a
        .modules
        .iter()
        .map(|m| (key(&m.id, m.version), m))
        .collect();
    let closure = |roots: &[String]| -> Result<BTreeSet<String>> {
        Ok(expand(catalog.artifact(), roots)?.into_iter().collect())
    };
    let required_roots: Vec<_> = roots
        .iter()
        .filter(|r| modules[*r].required)
        .cloned()
        .collect();
    let required = closure(&required_roots)?;
    let allowed = |p: &Permission| {
        request.available_tools.contains(&p.operation)
            && [
                &request.ceilings.installation,
                &request.ceilings.project,
                &request.ceilings.execution,
                &request.ceilings.caller,
                &execution.permissions,
            ]
            .iter()
            .all(|c| contains(c, p))
    };
    let mut available = BTreeMap::<String, bool>::new();
    let mut reasons = BTreeMap::<String, String>::new();
    for name in &topological {
        let m = modules[name];
        let missing: Vec<_> = m
            .required_adapters
            .iter()
            .filter(|id| !request.available_adapters.contains(id))
            .collect();
        let reason = if !missing.is_empty() {
            Some(format!("adapter_unavailable:{missing:?}"))
        } else if m
            .requirements
            .iter()
            .any(|r| r.required && !allowed(&r.permission))
        {
            Some("required_operation_denied".into())
        } else if m.dependencies.iter().any(|d| !available[d]) {
            Some("dependency_unavailable".into())
        } else {
            None
        };
        available.insert(name.clone(), reason.is_none());
        if let Some(reason) = reason {
            reasons.insert(name.clone(), reason);
        }
    }
    let selected_roots: Vec<_> = roots.iter().filter(|r| available[*r]).cloned().collect();
    let selected = closure(&selected_roots)?;
    let mut order = topological.clone();
    order.sort_by_key(|name| usize::from(modules[name].kind == ModuleKind::Capability));
    let mut requested = Vec::new();
    let mut effective = Vec::new();
    let mut denied = Vec::new();
    let mut omissions = Vec::new();
    let mut module_context = Vec::new();
    let mut flattened = Vec::new();
    for name in &order {
        let m = modules[name];
        let source = catalog.source("module", name)?;
        flattened.push(source.clone());
        let is_required = required.contains(name);
        if is_required && !available[name] {
            blocking.push(format!("required module {name}:{}", reasons[name]));
        }
        for r in &m.requirements {
            add(&mut requested, &r.permission);
            if is_required && r.required && !allowed(&r.permission) {
                add(&mut denied, &r.permission);
            }
        }
        if !selected.contains(name) {
            omissions.push(Omission {
                source,
                reason_code: reasons
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| "dependent_module_omitted".into()),
                required: is_required,
            });
            continue;
        }
        ensure(
            !is_required || m.delivery == Delivery::Startup,
            "required dependency cannot be deferred",
        )?;
        for r in &m.requirements {
            if allowed(&r.permission) {
                add(&mut effective, &r.permission);
            } else {
                omissions.push(Omission {
                    source: source.clone(),
                    reason_code: format!("optional_permission_denied:{}", r.permission.operation),
                    required: false,
                });
            }
        }
        module_context.push(ContextInput {
            source,
            text: catalog.artifact().instructions[name].clone(),
            delivery: m.delivery.clone(),
            required: is_required,
            omission_priority: 0,
        });
    }
    for c in [&request.platform, &request.project, &request.assignment] {
        context_valid(c)?;
        ensure(
            c.required && c.delivery == Delivery::Startup,
            "platform/project/assignment must be startup required",
        )?;
    }
    for c in &request.context {
        context_valid(c)?;
    }
    let mut aggregate_input_bytes = 0usize;
    for context in [&request.platform, &request.project, &request.assignment]
        .into_iter()
        .chain(request.context.iter())
    {
        aggregate_input_bytes = aggregate_input_bytes
            .checked_add(context.text.len())
            .ok_or_else(|| Error::Invalid("context input overflow".into()))?;
        ensure(
            aggregate_input_bytes <= MAX_CATALOG_BYTES,
            "aggregate context input bound",
        )?;
    }
    let role_source = catalog.source("agent", &request.profile.agent_profile)?;
    let role = ContextInput {
        source: role_source.clone(),
        text: agent.role.clone(),
        delivery: Delivery::Startup,
        required: true,
        omission_priority: 0,
    };
    // Fixed prompt order: platform, role/behavior, packs, project, exact assignment, labelled context.
    let mut contexts = vec![request.platform.clone(), role];
    contexts.extend(module_context);
    contexts.extend([request.project.clone(), request.assignment.clone()]);
    contexts.extend(request.context.clone());
    ensure(contexts.len() <= 128, "instruction plan cardinality")?;
    let mut seen = BTreeSet::new();
    for c in &contexts {
        ensure(
            seen.insert((c.source.identity.clone(), c.source.revision.clone())),
            "duplicate context identity",
        )?;
    }
    let sum = |required: bool| -> Result<u64> {
        contexts
            .iter()
            .filter(|c| c.delivery == Delivery::Startup && c.required == required)
            .try_fold(0u64, |n, c| {
                n.checked_add(c.text.len() as u64)
                    .ok_or_else(|| Error::Invalid("context sum overflow".into()))
            })
    };
    let required_bytes = sum(true)?;
    let mut optional_bytes = sum(false)?;
    if required_bytes > limit {
        return Err(Error::Invalid(format!(
            "required context overflow: {required_bytes}>{limit}; sources:{}",
            contexts
                .iter()
                .filter(|c| c.required)
                .map(|c| c.source.identity.as_str())
                .collect::<Vec<_>>()
                .join(",")
        )));
    }
    let mut drop_order: Vec<_> = contexts
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.required && c.delivery == Delivery::Startup)
        .map(|(i, c)| (c.omission_priority, i))
        .collect();
    drop_order.sort();
    let mut dropped = BTreeSet::new();
    let mut dropped_modules = BTreeSet::new();
    for (_, index) in drop_order {
        if required_bytes + optional_bytes <= limit {
            break;
        }
        if dropped.contains(&index) {
            continue;
        }
        let mut group = BTreeSet::from([index]);
        if let Some(name) = contexts[index].source.identity.strip_prefix("module:") {
            let mut names = BTreeSet::from([name.to_string()]);
            loop {
                let before = names.len();
                for module in &order {
                    if modules[module]
                        .dependencies
                        .iter()
                        .any(|d| names.contains(d))
                    {
                        names.insert(module.clone());
                    }
                }
                if before == names.len() {
                    break;
                }
            }
            for (i, c) in contexts.iter().enumerate() {
                if c.source
                    .identity
                    .strip_prefix("module:")
                    .is_some_and(|n| names.contains(n))
                {
                    ensure(!c.required, "cannot omit required dependency closure")?;
                    group.insert(i);
                }
            }
            dropped_modules.extend(names);
        }
        for index in group {
            if dropped.insert(index) {
                if contexts[index].delivery == Delivery::Startup {
                    optional_bytes -= contexts[index].text.len() as u64;
                }
                omissions.push(Omission {
                    source: contexts[index].source.clone(),
                    reason_code: "optional_context_budget".into(),
                    required: false,
                });
            }
        }
    }
    // Omitted optional capability closures cannot leave their operations effective.
    effective.clear();
    for name in &order {
        if selected.contains(name) && !dropped_modules.contains(name) {
            for r in &modules[name].requirements {
                if allowed(&r.permission) {
                    add(&mut effective, &r.permission);
                }
            }
        }
    }
    let references = contexts
        .iter()
        .enumerate()
        .filter(|(index, c)| c.delivery == Delivery::OnDemand && !dropped.contains(index))
        .map(|(_, c)| c.source.clone())
        .collect();
    let instructions = contexts
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !dropped.contains(i))
        .map(|(_, c)| Instruction {
            source: c.source,
            delivery: c.delivery,
            required: c.required,
            text: c.text,
        })
        .collect();
    let explanation = ProfileExplanation {
        schema_version: 1,
        request: request.profile.clone(),
        valid: false,
        agent_profile: role_source.clone(),
        execution_profile: catalog.source("execution", &agent.execution)?,
        graph_access_profile_digest: graph_access_profile_digest.into(),
        flattened_modules: flattened,
        overrides: overrides.clone(),
        requested_permissions: requested,
        effective_permissions: effective.clone(),
        denied_requirements: denied,
        omissions: omissions.clone(),
        reference_material: references,
        required_bytes,
        optional_bytes,
        estimated_tokens: None,
        tokenizer_version: None,
        selection: Selection {
            intelligence_profile: catalog.source("intelligence", &agent.intelligence)?,
            selected_candidate_id: None,
            snapshot_digest: digest(&request)?,
            decision_trace: vec![
                "selection_pending:AQ-20 must authorize observations and choose concrete model"
                    .into(),
            ],
            candidates: intelligence
                .candidates
                .iter()
                .map(|c| Candidate {
                    candidate_id: c.id.clone(),
                    provider: c.provider.clone(),
                    model: c.model.clone(),
                    harness: request.harness.clone(),
                    reasoning_configuration_reference: c.reasoning_configuration_reference.clone(),
                    eligibility: if c.harnesses.contains(&request.harness) {
                        Eligibility::StaleUnknown
                    } else {
                        Eligibility::Unsupported
                    },
                    reason_codes: vec!["selection_pending".into()],
                    observations: vec![],
                })
                .collect(),
        },
    };
    aq_session_api::decode(
        &serde_json::to_vec(&ContractDocument::ProfileExplanation(Box::new(
            explanation.clone(),
        )))
        .unwrap(),
    )
    .map_err(Error::Invalid)?;
    let composition = blocking.is_empty().then(|| FrozenComposition {
        schema_version: 1,
        catalog_digest: catalog.digest().into(),
        agent_profile: role_source,
        intelligence_profile: explanation.selection.intelligence_profile.clone(),
        execution_profile: explanation.execution_profile.clone(),
        project_binding_id: request.profile.project_binding_id,
        project_revision: request.profile.expected_project_revision,
        task_context_revision: request.profile.task_context_revision.clone(),
        graph_access_profile_digest: graph_access_profile_digest.into(),
        harness: request.harness.clone(),
        environment: request.environment.clone(),
        instruction_order: instructions,
        effective_permissions: effective,
        context_limit_bytes: limit,
        overrides,
        omissions,
    });
    let composition_digest = composition.as_ref().map(digest).transpose()?;
    Ok(Compilation {
        catalog_valid: true,
        composition_valid: blocking.is_empty(),
        selection_pending: true,
        blocking_reasons: blocking,
        composition_digest,
        composition,
        explanation,
    })
}
