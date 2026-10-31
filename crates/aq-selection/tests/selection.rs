mod common;
use aq_profiles::{FallbackReason, UnknownCostPolicy};
use aq_selection::*;
use common::*;
use uuid::Uuid;
#[test]
fn stable_order_chooses_exact_native_configuration_without_equating_reasoning_controls() {
    let (catalog, input) = fixture();
    let first = select(&catalog, &input).unwrap();
    let selected = first.resolved.unwrap();
    assert_eq!(selected.choice.model, "model-a");
    assert_eq!(selected.choice.native_reasoning, "high");
    let mut shuffled = input.clone();
    shuffled.credentials.reverse();
    shuffled.observations.reverse();
    assert_eq!(
        select(&catalog, &shuffled)
            .unwrap()
            .resolved
            .unwrap()
            .choice
            .candidate_id,
        selected.choice.candidate_id
    );
}
#[test]
fn explicit_fallback_and_cooldown_do_not_implicitly_downgrade() {
    let (catalog, mut input) = fixture();
    input.observations[0].state = Availability::QuotaUnavailable;
    input.observations[0].cooldown_until_ms = 5000;
    let out = select(&catalog, &input).unwrap();
    assert_eq!(out.resolved.unwrap().choice.model, "model-b");
    assert_eq!(out.traces[0].state, CandidateState::Unavailable);
    let mut policy = policy();
    policy.fallback_on.clear();
    let changed = change_policy(&catalog, policy, &mut input);
    let out = select(&changed, &input).unwrap();
    assert!(out.resolved.is_none());
    assert!(out.reason_codes.contains(&"no_permitted_fallback".into()));
}
#[test]
fn unknown_availability_can_only_allow_bounded_attempt_with_present_credentials() {
    let (catalog, mut input) = fixture();
    input.observations.clear();
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    let mut policy = policy();
    policy.allow_unknown_availability = true;
    let catalog = change_policy(&catalog, policy, &mut input);
    assert!(select(&catalog, &input).unwrap().resolved.is_some());
    input
        .credentials
        .iter_mut()
        .for_each(|c| c.provisionable = false);
    let out = select(&catalog, &input).unwrap();
    assert!(out.resolved.is_none());
    assert!(
        out.traces
            .iter()
            .all(|t| t.state == CandidateState::MissingCredentials)
    );
}
#[test]
fn installed_credential_and_central_authorization_are_independent() {
    let (catalog, mut input) = fixture();
    input.node.credential_bindings.clear();
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    let (catalog, mut input) = fixture();
    input.credentials[0].enabled = false;
    let out = select(&catalog, &input).unwrap();
    assert_eq!(out.traces[0].state, CandidateState::PolicyDenied);
    assert_eq!(out.resolved.unwrap().choice.model, "model-b");
}
#[test]
fn stale_node_and_unsupported_native_settings_never_become_unknown_availability_attempts() {
    let (catalog, mut input) = fixture();
    let mut policy = policy();
    policy.allow_unknown_availability = true;
    let catalog = change_policy(&catalog, policy, &mut input);
    input.node.expires_at_ms = 1500;
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    let (catalog, mut input) = fixture();
    input.node.harnesses[0].models.clear();
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    let (catalog, mut input) = fixture();
    input.node.harnesses[0].limits.max_output_tokens = 10;
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
}
#[test]
fn observations_scope_account_endpoint_model_node_and_credential_revision_exactly() {
    let (catalog, mut input) = fixture();
    let mut unrelated = input.observations[0].clone();
    unrelated.observation_id = Uuid::from_u128(55);
    unrelated.state = Availability::QuotaUnavailable;
    unrelated.scope.account_reference = "unrelated-account".into();
    input.observations.push(unrelated);
    assert_eq!(
        select(&catalog, &input)
            .unwrap()
            .resolved
            .unwrap()
            .choice
            .model,
        "model-a"
    );
    for field in ["endpoint", "model", "credential_revision"] {
        let mut input = input.clone();
        let o = input.observations.last_mut().unwrap();
        o.scope.account_reference = "billing-a".into();
        match field {
            "endpoint" => o.scope.endpoint_reference = "other-endpoint".into(),
            "model" => o.scope.model = Some("other-model".into()),
            _ => o.scope.credential_revision = 2,
        };
        assert_eq!(
            select(&catalog, &input)
                .unwrap()
                .resolved
                .unwrap()
                .choice
                .model,
            "model-a"
        );
    }
}
#[test]
fn worker_failure_cannot_claim_account_wide_availability_or_survive_boot_reuse() {
    let (catalog, mut input) = fixture();
    let o = &mut input.observations[0];
    o.source = ObservationSource::Worker {
        node_id: Uuid::from_u128(4),
        boot_id: Uuid::from_u128(5),
        attempt_id: Uuid::from_u128(6),
        selection_digest: aq_session_protocol::Digest::of(b"sealed"),
    };
    o.state = Availability::QuotaUnavailable;
    assert!(select(&catalog, &input).is_err());
    input.observations[0].scope.node_id = Some(Uuid::from_u128(4));
    assert_eq!(
        select(&catalog, &input)
            .unwrap()
            .resolved
            .unwrap()
            .choice
            .model,
        "model-b"
    );
    input.node.target = ExecutionTarget::Worker {
        node_id: Uuid::from_u128(4),
        boot_id: Uuid::from_u128(7),
    };
    input.project.target = input.node.target.clone();
    let out = select(&catalog, &input).unwrap();
    assert_eq!(out.traces[0].state, CandidateState::StaleUnknown);
}
#[test]
fn lineage_retry_cost_and_replacement_budgets_cannot_reset_on_fallback() {
    let (catalog, mut input) = fixture();
    input.budget.attempts_started = 3;
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    input.budget.attempts_started = 1;
    input.budget.last_attempt_id = Some(Uuid::from_u128(6));
    let out = select(&catalog, &input).unwrap();
    assert!(out.resolved.is_none());
    assert!(
        out.traces[0]
            .reason_codes
            .contains(&"replacement_stop_retention_reconciliation_required".into())
    );
    input.replacement = Some(ReplacementEvidence {
        lineage: input.project.lineage.clone(),
        predecessor_attempt_id: Uuid::from_u128(6),
        stop_receipt: artifact("stop-receipt"),
        retained_source_receipt: artifact("retained-source"),
        effect_reconciliation_receipt: artifact("effects-reconciled"),
        observed_at_ms: 1000,
        expires_at_ms: 5000,
    });
    assert!(select(&catalog, &input).unwrap().resolved.is_some());
    input.budget.elapsed_ms = Some(60000);
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
}
#[test]
fn ledger_unknown_is_not_zero_and_hard_cost_requires_qualified_provider_and_adapter() {
    let (catalog, mut input) = fixture();
    input.project.ceilings.cost_limit_microunits = Some(100);
    input.project.ceilings.currency = Some("USD".into());
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    let mut policy = policy();
    policy.budget.unknown_cost = UnknownCostPolicy::AllowBounded;
    let catalog = change_policy(&catalog, policy, &mut input);
    assert!(select(&catalog, &input).unwrap().resolved.is_some());
    input.project.ceilings.require_hard_cost_enforcement = true;
    let out = select(&catalog, &input).unwrap();
    assert!(out.resolved.is_none());
    assert!(
        out.traces[0]
            .reason_codes
            .contains(&"hard_cost_enforcement_unqualified".into())
    );
}
#[test]
fn missing_policy_old_catalog_is_explicitly_ineligible_and_snapshots_are_bound() {
    let (catalog, mut input) = fixture();
    input.project.revision = 2;
    assert!(select(&catalog, &input).is_err());
    let (catalog, mut input) = fixture();
    let mut definitions = catalog.artifact().catalog.clone();
    definitions.intelligence[0].selection_policy = None;
    let old = aq_profiles::validate(definitions, catalog.artifact().instructions.clone()).unwrap();
    input.project.catalog_digest = serde_json::from_value(serde_json::json!(old.digest())).unwrap();
    for c in &mut input.compositions {
        c.composition.catalog_digest = old.digest().into();
        c.composition.intelligence_profile = old
            .source("intelligence", &input.project.intelligence_profile)
            .unwrap();
        c.digest = aq_selection::digest(&c.composition).unwrap();
    }
    assert!(
        select(&old, &input)
            .unwrap()
            .reason_codes
            .contains(&"selection_policy_missing".into())
    );
}
#[test]
fn fallback_unsupported_is_explicit_and_provider_specific_reasoning_remains_native() {
    let (catalog, mut input) = fixture();
    input.node.harnesses[0]
        .models
        .retain(|m| m.provider != "fixture-a");
    let mut policy = policy();
    policy.fallback_on = vec![FallbackReason::Unsupported];
    let catalog = change_policy(&catalog, policy, &mut input);
    let out = select(&catalog, &input).unwrap();
    assert_eq!(out.resolved.unwrap().choice.native_reasoning, "deliberate");
}
#[test]
fn unknown_cost_never_bypasses_known_exhaustion_or_incompatible_currency() {
    let (catalog, mut input) = fixture();
    input.project.ceilings.cost_limit_microunits = Some(100);
    input.project.ceilings.currency = Some("USD".into());
    let mut policy = policy();
    policy.budget.unknown_cost = UnknownCostPolicy::AllowBounded;
    let catalog = change_policy(&catalog, policy, &mut input);
    for spent in [100, 101] {
        input.budget.spent_microunits = Some(spent);
        input.budget.currency = Some("USD".into());
        input.budget.accounting = AccountingCompleteness::Incomplete;
        let out = select(&catalog, &input).unwrap();
        assert!(out.resolved.is_none());
        assert!(
            out.traces[0]
                .reason_codes
                .contains(&"lineage_cost_budget_exhausted".into())
        );
    }
    input.budget.spent_microunits = Some(1);
    input.budget.currency = Some("EUR".into());
    let out = select(&catalog, &input).unwrap();
    assert!(out.resolved.is_none());
    assert!(
        out.traces[0]
            .reason_codes
            .contains(&"budget_currency_mismatch".into())
    );
}

#[test]
fn classified_worker_failure_is_bound_to_exact_sealed_choice_and_current_boot() {
    let (catalog, input) = fixture();
    let selected = select(&catalog, &input).unwrap().resolved.unwrap();
    let ExecutionTarget::Worker { node_id, boot_id } = selected.target else {
        panic!()
    };
    let attempt = Uuid::from_u128(9001);
    let mut report = StructuredFailure {
        schema_version: 1,
        operation_id: Uuid::from_u128(9002),
        attempt_id: attempt,
        selection_digest: digest(&selected).unwrap(),
        node_id,
        boot_id,
        classifier: selected.choice.failure_classifier.clone().unwrap(),
        sequence: 1,
        failure: ProviderFailure::Quota,
        lifecycle_event_id: None,
    };
    let observation = derive_worker_observation(
        &report,
        &aq_selection::FailureContext {
            observation_id: Uuid::from_u128(9005),
            authenticated_node: node_id,
            current_boot: boot_id,
            sealed_attempt: attempt,
            sealed: &selected,
            revision: 1,
            now_ms: input.now_ms,
            ttl_ms: 5000,
            cooldown_ms: 1000,
        },
    )
    .unwrap();
    assert_eq!(
        observation.scope.account_reference,
        selected.choice.account_reference
    );
    assert_eq!(observation.scope.model, Some(selected.choice.model.clone()));
    assert_eq!(observation.scope.node_id, Some(node_id));
    assert!(
        derive_worker_observation(
            &report,
            &aq_selection::FailureContext {
                observation_id: Uuid::from_u128(9005),
                authenticated_node: node_id,
                current_boot: Uuid::from_u128(9003),
                sealed_attempt: attempt,
                sealed: &selected,
                revision: 1,
                now_ms: input.now_ms,
                ttl_ms: 5000,
                cooldown_ms: 1000
            }
        )
        .is_err()
    );
    assert!(
        derive_worker_observation(
            &report,
            &aq_selection::FailureContext {
                observation_id: Uuid::from_u128(9005),
                authenticated_node: node_id,
                current_boot: boot_id,
                sealed_attempt: Uuid::from_u128(9004),
                sealed: &selected,
                revision: 1,
                now_ms: input.now_ms,
                ttl_ms: 5000,
                cooldown_ms: 1000
            }
        )
        .is_err()
    );
    report.classifier = common::artifact("unqualified-classifier");
    assert!(
        derive_worker_observation(
            &report,
            &aq_selection::FailureContext {
                observation_id: Uuid::from_u128(9005),
                authenticated_node: node_id,
                current_boot: boot_id,
                sealed_attempt: attempt,
                sealed: &selected,
                revision: 1,
                now_ms: input.now_ms,
                ttl_ms: 5000,
                cooldown_ms: 1000
            }
        )
        .is_err()
    );
    report.classifier = selected.choice.failure_classifier.clone().unwrap();
    report.selection_digest = aq_selection::Digest::of(b"foreign-choice");
    assert!(
        derive_worker_observation(
            &report,
            &aq_selection::FailureContext {
                observation_id: Uuid::from_u128(9005),
                authenticated_node: node_id,
                current_boot: boot_id,
                sealed_attempt: attempt,
                sealed: &selected,
                revision: 1,
                now_ms: input.now_ms,
                ttl_ms: 5000,
                cooldown_ms: 1000
            }
        )
        .is_err()
    );
}
#[test]
fn project_environment_allowlist_cannot_be_widened_by_installed_qualified_adapter() {
    let (catalog, mut input) = fixture();
    input.project.allowed_environments = vec!["another-qualified-environment".into()];
    let result = select(&catalog, &input).unwrap();
    assert!(result.resolved.is_none());
    assert!(result.traces.iter().any(|t| {
        t.reason_codes
            .iter()
            .any(|r| r == "project_environment_denied")
    }));
}
#[test]
fn published_human_selection_preserves_exact_provenance_and_refuses_changed_decisions() {
    let (catalog, input) = fixture();
    let mut outcome = select(&catalog, &input).unwrap();
    let human = human_selection(&catalog, &input, &outcome).unwrap();
    assert_eq!(
        human.selected_candidate_id.as_deref(),
        Some(
            outcome
                .resolved
                .as_ref()
                .unwrap()
                .choice
                .candidate_id
                .as_str()
        )
    );
    assert_eq!(
        human.candidates[0].observations[0].endpoint_reference,
        input.observations[0].scope.endpoint_reference
    );
    let encoded = serde_json::to_vec(&human).unwrap();
    let decoded: aq_session_api::Selection = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded.snapshot_digest, human.snapshot_digest);
    outcome.resolved.as_mut().unwrap().choice.account_reference = "foreign-account".into();
    assert!(human_selection(&catalog, &input, &outcome).is_err());
}
#[test]
fn independent_matching_source_cannot_clear_another_sources_live_restriction() {
    let (catalog, mut input) = fixture();
    let mut restricted = input.observations[0].clone();
    restricted.observation_id = Uuid::from_u128(9101);
    restricted.source = ObservationSource::Operator {
        principal_id: Uuid::from_u128(9102),
    };
    restricted.state = Availability::QuotaUnavailable;
    restricted.received_at_ms = input.now_ms - 10;
    restricted.expires_at_ms = input.now_ms + 1000;
    restricted.cooldown_until_ms = input.now_ms + 1000;
    let mut available = restricted.clone();
    available.observation_id = Uuid::from_u128(9103);
    available.source = ObservationSource::Operator {
        principal_id: Uuid::from_u128(9104),
    };
    available.state = Availability::Available;
    available.received_at_ms = input.now_ms - 1;
    input.observations.extend([restricted, available]);
    let result = select(&catalog, &input).unwrap();
    assert_eq!(result.resolved.unwrap().choice.provider, "fixture-b");
}
#[test]
fn source_sequence_precedes_wall_clock_when_history_contains_clock_correction() {
    let (catalog, mut input) = fixture();
    let mut later = input.observations[0].clone();
    later.observation_id = Uuid::from_u128(9201);
    later.sequence += 1;
    later.state = Availability::QuotaUnavailable;
    later.received_at_ms = input.observations[0].received_at_ms - 1;
    input.observations.push(later);
    assert_eq!(
        select(&catalog, &input)
            .unwrap()
            .resolved
            .unwrap()
            .choice
            .provider,
        "fixture-b"
    );
}
#[test]
fn installed_only_v010_snapshots_and_decisions_roundtrip_byte_semantics_unchanged() {
    let golden: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/installed-selection-v0.1.0.json")).unwrap();
    let catalog = aq_profiles::ValidatedCatalog::from_artifact(include_bytes!(
        "fixtures/installed-catalog-v0.1.0.json"
    ))
    .unwrap();
    let input: SelectionInputs = serde_json::from_value(golden["inputs"].clone()).unwrap();
    assert_eq!(serde_json::to_value(&input).unwrap(), golden["inputs"]);
    assert!(select(&catalog, &input).is_err());
    let outcome = select_legacy_for_inspection(&catalog, &input).unwrap();
    assert_eq!(serde_json::to_value(outcome).unwrap(), golden["outcome"]);
}
fn prepared_credentials(input: &mut SelectionInputs) {
    let delivery = DeliveryCapability {
        adapter: artifact("qualified-private-delivery-adapter"),
        version: 1,
    };
    input.node.material_delivery = Some(delivery.clone());
    for (credential, binding) in input
        .credentials
        .iter_mut()
        .zip(&mut input.node.credential_bindings)
    {
        let owner = artifact(&format!(
            "daemon-file-owner:{}",
            credential.profile.id.as_str()
        ));
        credential.material_owner = Some(owner.clone());
        binding.available = false;
        binding.preparation = Some(ProvisioningReadiness {
            owner,
            credential_profile: credential.profile.clone(),
            credential_revision: credential.revision,
            target: input.project.target.clone(),
            delivery: delivery.clone(),
            observed_at_ms: input.now_ms - 1,
            expires_at_ms: input.now_ms + 1000,
        });
    }
}
#[test]
fn cold_node_selects_only_explicit_owned_preparation_and_preserves_material_provenance() {
    let (catalog, mut input) = fixture();
    prepared_credentials(&mut input);
    let outcome = select(&catalog, &input).unwrap();
    let choice = outcome.resolved.unwrap();
    assert!(matches!(
        choice.choice.credential_delivery,
        Some(CredentialDelivery::ServerLease { version: 1, .. })
    ));
    assert!(
        choice.traces[0]
            .reason_codes
            .contains(&"material_preparation_required".into())
    );
    assert!(
        !input.node.credential_bindings[0].available,
        "readiness never claims installed bytes"
    );
    assert!(
        choice.expires_at_ms
            <= input.node.credential_bindings[0]
                .preparation
                .as_ref()
                .unwrap()
                .expires_at_ms
    );
}
#[test]
fn arbitrary_node_delivery_claims_cannot_manufacture_keys_owner_identity_or_current_boot() {
    let (catalog, mut input) = fixture();
    prepared_credentials(&mut input);
    for credential in &mut input.credentials {
        credential.provisionable = false;
    }
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    for credential in &mut input.credentials {
        credential.provisionable = true;
        credential.material_owner = None;
    }
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    prepared_credentials(&mut input);
    for binding in &mut input.node.credential_bindings {
        binding.preparation.as_mut().unwrap().target = ExecutionTarget::Worker {
            node_id: Uuid::from_u128(9301),
            boot_id: Uuid::from_u128(9302),
        };
    }
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    prepared_credentials(&mut input);
    input.node.material_delivery.as_mut().unwrap().adapter = artifact("unqualified-node-choice");
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
    prepared_credentials(&mut input);
    for binding in &mut input.node.credential_bindings {
        binding.preparation.as_mut().unwrap().expires_at_ms = input.now_ms;
    }
    assert!(select(&catalog, &input).unwrap().resolved.is_none());
}

#[test]
fn two_models_on_one_harness_share_acyclic_policy_but_resolve_distinct_actual_profiles() {
    use aq_profiles::{GraphRetryBudget, ResolvedGraphModel};
    let (catalog, mut input) = fixture();
    let composition = input.compositions[0].composition.clone();
    assert_eq!(composition.schema_version, 2);
    let serialized = serde_json::to_value(&composition).unwrap();
    assert!(serialized.get("graph_access_profile_digest").is_none());
    assert!(serialized.get("graph_access_policy_digest").is_some());
    let policy = graph_policy(&composition);
    composition.validate_graph_policy(&policy).unwrap();
    let first = select(&catalog, &input).unwrap().resolved.unwrap();
    input.observations[0].state = Availability::QuotaUnavailable;
    let second = select(&catalog, &input).unwrap().resolved.unwrap();
    assert_eq!(first.choice.harness, second.choice.harness);
    assert_ne!(first.choice.model, second.choice.model);
    assert_eq!(
        first.choice.composition_digest,
        second.choice.composition_digest
    );
    let retry = GraphRetryBudget {
        max_attempts: 3,
        max_elapsed_ms: 60000,
        initial_delay_ms: 100,
        max_delay_ms: 1000,
    };
    let resolve = |selection: &ResolvedSelection| {
        policy
            .resolve_profile(
                &ResolvedGraphModel {
                    provider: selection.choice.provider.clone(),
                    model: selection.choice.model.clone(),
                    native_reasoning: selection.choice.native_reasoning.clone(),
                    reasoning_configuration_reference: selection
                        .choice
                        .reasoning_configuration_reference
                        .clone(),
                },
                &graph_context(),
                "implementer",
                &retry,
            )
            .unwrap()
    };
    let first_profile = resolve(&first);
    let second_profile = resolve(&second);
    assert_eq!(first_profile["model"]["provider"], first.choice.provider);
    assert_eq!(second_profile["model"]["model"], second.choice.model);
    assert_ne!(first_profile["profile_id"], second_profile["profile_id"]);
    assert_ne!(
        digest(&first_profile).unwrap(),
        digest(&second_profile).unwrap()
    );
    assert_eq!(
        first_profile["instructions_digest"],
        second_profile["instructions_digest"]
    );
    assert_eq!(
        first_profile["tool_policy_digest"],
        second_profile["tool_policy_digest"]
    );
    assert_eq!(
        first_profile["read_scope"]["record_ids"],
        serde_json::json!(policy.scope.record_ids)
    );
    let mut changed = graph_context();
    changed["nodes"][0]["body"] = serde_json::json!("changed");
    assert!(policy.validate_context(&changed).is_err());
    let mut changed_policy = policy.clone();
    changed_policy.instructions_digest = "f".repeat(64);
    assert!(composition.validate_graph_policy(&changed_policy).is_err());
    let mut changed_composition = composition;
    changed_composition.graph_access_profile_digest = "a".repeat(64);
    assert!(changed_composition.validate_native_v2().is_err());
}

#[test]
fn graph_policy_refuses_uncovered_mixed_context_and_wrong_owner_scope_before_selection() {
    use aq_profiles::{GraphAccessScope, compile_draft};
    let (catalog, input) = fixture();
    let mut context = graph_context();
    let mut knowledge = context["nodes"][0].clone();
    knowledge["id"] = serde_json::json!(Uuid::from_u128(9));
    knowledge["kind"] = serde_json::json!("knowledge");
    knowledge["task_state"] = serde_json::Value::Null;
    context["nodes"].as_array_mut().unwrap().push(knowledge);
    let scope = GraphAccessScope {
        repository_id: Uuid::from_u128(41),
        workspace_id: Uuid::from_u128(2),
        task_id: Uuid::from_u128(3),
        record_ids: vec![Uuid::from_u128(3), Uuid::from_u128(9)],
        edge_types: vec![],
    };
    let draft = compile_draft(&catalog, &compile_request(&catalog))
        .unwrap()
        .composition
        .unwrap();
    let sealed = draft.seal(scope.clone(), &context).unwrap();
    sealed
        .graph_access_policy
        .validate_context(&context)
        .unwrap();
    for missing in ["task.read", "knowledge.read"] {
        let mut denied = sealed.graph_access_policy.clone();
        denied
            .effective_permissions
            .retain(|p| p.operation != missing);
        assert!(denied.validate_context(&context).is_err(), "{missing}");
    }
    let mut wrong = sealed.graph_access_policy.clone();
    wrong.effective_permissions[0].scope_reference = format!("project:{}", Uuid::from_u128(8888));
    assert!(wrong.validate_context(&context).is_err());
    let mut alias = sealed.graph_access_policy.clone();
    alias.effective_permissions[0].scope_reference = "project:example".into();
    assert!(alias.validate_context(&context).is_err());
    let mut wrong_repository = sealed.graph_access_policy;
    wrong_repository.scope.repository_id = Uuid::from_u128(8888);
    assert!(wrong_repository.validate_context(&context).is_err());
    let mut request = compile_request(&catalog);
    request
        .available_tools
        .retain(|tool| tool != "knowledge.read");
    let task_only = compile_draft(&catalog, &request)
        .unwrap()
        .composition
        .unwrap();
    assert!(task_only.seal(scope, &context).is_err());
    assert_eq!(input.compositions[0].composition.schema_version, 2);
}

#[test]
fn native_policy_binding_does_not_allow_rehashed_instruction_or_permission_forgery() {
    let (catalog, mut input) = fixture();
    let prepared = &mut input.compositions[0];
    prepared
        .composition
        .instruction_order
        .iter_mut()
        .find(|i| i.source.identity.starts_with("module:"))
        .unwrap()
        .text
        .push_str(" unauthorized instruction");
    prepared.digest = digest(&prepared.composition).unwrap();
    assert!(select(&catalog, &input).is_err());
    let (_, mut input) = fixture();
    input.compositions[0].composition.requested_permissions = Some(vec![]);
    input.compositions[0].digest = digest(&input.compositions[0].composition).unwrap();
    assert!(select(&catalog, &input).is_err());
    let (_, mut input) = fixture();
    input.compositions[0].composition.graph_access_policy_kind = None;
    input.compositions[0].digest = digest(&input.compositions[0].composition).unwrap();
    assert!(select(&catalog, &input).is_err());
}
