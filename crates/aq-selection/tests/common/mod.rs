#![allow(dead_code)]
use aq_profiles::*;
use aq_selection::*;
use aq_session_api::{ProfileRequest, Source};
use aq_session_protocol::{ArtifactRef, Digest, Text};
use uuid::Uuid;
pub fn artifact(id: &str) -> ArtifactRef {
    ArtifactRef {
        id: Text::new(id).unwrap(),
        version: Text::new("1").unwrap(),
        digest: Digest::of(id.as_bytes()),
    }
}
pub fn policy() -> SelectionPolicy {
    SelectionPolicy {
        schema_version: 1,
        fallback_on: vec![
            FallbackReason::Unavailable,
            FallbackReason::MissingCredentials,
            FallbackReason::Unsupported,
            FallbackReason::StaleUnknown,
            FallbackReason::PolicyDenied,
        ],
        allow_unknown_availability: false,
        observation_max_age_ms: 5000,
        max_output_tokens: 128,
        max_attempts: 3,
        max_lineage_elapsed_ms: 60_000,
        required_capabilities: vec![],
        budget: IntelligenceBudgetPolicy {
            max_cost_microunits: None,
            currency: None,
            require_complete_accounting: false,
            require_hard_cost_enforcement: false,
            unknown_cost: UnknownCostPolicy::Deny,
        },
    }
}
fn context(id: &str, body: &str) -> ContextInput {
    ContextInput {
        source: Source {
            identity: id.into(),
            revision: "1".into(),
            digest: Digest::of(body.as_bytes()).as_str().into(),
        },
        text: body.into(),
        delivery: Delivery::Startup,
        required: true,
        omission_priority: 0,
    }
}
pub fn fixture() -> (ValidatedCatalog, SelectionInputs) {
    let old = ValidatedCatalog::from_artifact(include_bytes!(
        "../fixtures/pre-selection-policy-catalog.json"
    ))
    .unwrap();
    let mut definitions = old.artifact().catalog.clone();
    for agent in &mut definitions.agents {
        agent.version = 2;
        agent.intelligence = "engineering@2".into();
        agent.execution = "confined-development@2".into();
    }
    let i = &mut definitions.intelligence[0];
    i.version = 2;
    i.selection_policy = Some(policy());
    i.candidates = vec![
        ModelCandidate {
            id: "primary".into(),
            provider: "fixture-a".into(),
            model: "model-a".into(),
            reasoning_configuration_reference: "reasoning-a@1".into(),
            harnesses: vec!["codex".into()],
        },
        ModelCandidate {
            id: "fallback".into(),
            provider: "fixture-b".into(),
            model: "model-b".into(),
            reasoning_configuration_reference: "reasoning-b@1".into(),
            harnesses: vec!["codex".into()],
        },
    ];
    definitions.execution[0].version = 2;
    definitions.execution[0].credential_profile_references =
        vec!["account-a@1".into(), "account-b@1".into()];
    let catalog = aq_profiles::validate(definitions, old.artifact().instructions.clone()).unwrap();
    let permissions = catalog.artifact().catalog.execution[0].permissions.clone();
    let request = CompileRequest {
        profile: ProfileRequest {
            catalog_digest: catalog.digest().into(),
            agent_profile: "implementer@2".into(),
            project_binding_id: Uuid::from_u128(1),
            expected_project_revision: 1,
            task_context_revision: "task:1".into(),
        },
        ceilings: Ceilings {
            installation: permissions.clone(),
            project: permissions.clone(),
            execution: permissions.clone(),
            caller: permissions.clone(),
        },
        available_tools: permissions.iter().map(|p| p.operation.clone()).collect(),
        available_adapters: vec!["codex".into()],
        harness: "codex".into(),
        environment: "confined-worktree".into(),
        platform: context("platform:1", "Platform rules"),
        project: context("project:1", "Project rules"),
        assignment: context("assignment:1", "Exact assignment"),
        context: vec![],
        overrides: vec![],
    };
    let composition = compile(&catalog, &request, &"a".repeat(64))
        .unwrap()
        .composition
        .unwrap();
    let composition = PreparedComposition {
        digest: aq_selection::digest(&composition).unwrap(),
        composition,
    };
    let lineage = TaskLineage {
        project_id: Uuid::from_u128(1),
        workspace_id: Uuid::from_u128(2),
        task_id: Uuid::from_u128(3),
    };
    let target = ExecutionTarget::Worker {
        node_id: Uuid::from_u128(4),
        boot_id: Uuid::from_u128(5),
    };
    let credentials: Vec<_> = [("a", "fixture-a"), ("b", "fixture-b")]
        .iter()
        .map(|(suffix, provider)| AuthorizedCredential {
            profile: artifact(&format!("account-{suffix}")),
            provider: (*provider).into(),
            account_reference: format!("billing-{suffix}"),
            endpoint_reference: format!("endpoint-{suffix}"),
            revision: 1,
            enabled: true,
            provisionable: true,
            material_owner: None,
        })
        .collect();
    let models = vec![
        NativeModelSupport {
            provider: "fixture-a".into(),
            model: "model-a".into(),
            reasoning_configuration_reference: "reasoning-a@1".into(),
            native_reasoning: "high".into(),
        },
        NativeModelSupport {
            provider: "fixture-b".into(),
            model: "model-b".into(),
            reasoning_configuration_reference: "reasoning-b@1".into(),
            native_reasoning: "deliberate".into(),
        },
    ];
    let node = CapabilitySnapshot {
        material_delivery: None,
        target: target.clone(),
        revision: 1,
        observed_at_ms: 1000,
        expires_at_ms: 20000,
        harnesses: vec![InstalledHarness {
            harness: "codex".into(),
            cli_version: "fixture-1".into(),
            adapter: artifact("codex-adapter"),
            environment: artifact("confined-worktree"),
            qualification: artifact("synthetic-qualification"),
            failure_classifier: Some(artifact("synthetic-error-classifier")),
            models,
            capabilities: vec![],
            limits: NativeLimits {
                context_bytes: 8192,
                max_output_tokens: 512,
                wall_time_ms: 60000,
                memory_bytes: 268435456,
                cpu_millis: 60000,
                process_limit: 8,
                hard_cost_enforcer: None,
            },
        }],
        credential_bindings: credentials
            .iter()
            .map(|c| CredentialBinding {
                credential_revision: 1,
                credential_profile: c.profile.clone(),
                available: true,
                preparation: None,
                observed_at_ms: 1000,
                expires_at_ms: 20000,
            })
            .collect(),
    };
    let observations = credentials
        .iter()
        .enumerate()
        .map(|(index, c)| AvailabilityObservation {
            observation_id: Uuid::from_u128(10 + index as u128),
            scope: ObservationScope {
                credential_revision: 1,
                provider: c.provider.clone(),
                credential_profile: c.profile.clone(),
                account_reference: c.account_reference.clone(),
                endpoint_reference: c.endpoint_reference.clone(),
                model: Some(format!("model-{}", if index == 0 { "a" } else { "b" })),
                node_id: None,
            },
            source: ObservationSource::Operator {
                principal_id: Uuid::from_u128(99),
            },
            sequence: 1,
            revision: 1,
            state: Availability::Available,
            received_at_ms: 1000,
            expires_at_ms: 10000,
            cooldown_until_ms: 0,
        })
        .collect();
    let input = SelectionInputs {
        project: ProjectSnapshot {
            lineage: lineage.clone(),
            revision: 1,
            catalog_revision: 1,
            catalog_digest: serde_json::from_value(serde_json::json!(catalog.digest())).unwrap(),
            admission_snapshot_digest: Digest::of(b"project-admission"),
            expires_at_ms: 15000,
            target,
            ceilings: ResourceCeilings {
                context_bytes: 8192,
                wall_time_ms: 60000,
                memory_bytes: 268435456,
                cpu_millis: 60000,
                process_limit: 8,
                cost_limit_microunits: None,
                currency: None,
                require_hard_cost_enforcement: false,
            },
            allowed_environments: vec!["confined-worktree".into()],
            permission_ceiling: permissions
                .iter()
                .map(|p| aq_session_protocol::Scope {
                    operation: Text::new(&p.operation).unwrap(),
                    resource: Text::new(&p.scope_reference).unwrap(),
                })
                .collect(),
            agent_profile: "implementer@2".into(),
            intelligence_profile: "engineering@2".into(),
            execution_profile: "confined-development@2".into(),
        },
        node,
        credentials,
        observations,
        registry_revision: 1,
        budget: BudgetSnapshot {
            lineage,
            revision: 1,
            attempts_started: 0,
            last_attempt_id: None,
            elapsed_ms: Some(0),
            spent_microunits: None,
            currency: None,
            accounting: AccountingCompleteness::Unavailable,
            accounting_source: artifact("unavailable-ledger"),
            hard_budget_provider: None,
            observed_at_ms: 1000,
            expires_at_ms: 20000,
        },
        replacement: None,
        estimates: vec![],
        compositions: vec![composition],
        now_ms: 2000,
    };
    (catalog, input)
}
pub fn change_policy(
    catalog: &ValidatedCatalog,
    policy: SelectionPolicy,
    input: &mut SelectionInputs,
) -> ValidatedCatalog {
    let mut definitions = catalog.artifact().catalog.clone();
    definitions.intelligence[0].selection_policy = Some(policy);
    let updated =
        aq_profiles::validate(definitions, catalog.artifact().instructions.clone()).unwrap();
    input.project.catalog_digest =
        serde_json::from_value(serde_json::json!(updated.digest())).unwrap();
    for composition in &mut input.compositions {
        composition.composition.catalog_digest = updated.digest().into();
        composition.composition.intelligence_profile = updated
            .source("intelligence", &input.project.intelligence_profile)
            .unwrap();
        composition.digest = aq_selection::digest(&composition.composition).unwrap();
    }
    updated
}
