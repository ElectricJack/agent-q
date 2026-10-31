use crate::validate::{fresh, validate};
use crate::*;
use aq_profiles::{FallbackReason, SelectionPolicy, UnknownCostPolicy};
use aq_session_protocol::require_capabilities;
use std::collections::BTreeMap;
fn profile_key(profile: &aq_session_protocol::ArtifactRef) -> String {
    format!("{}@{}", profile.id.as_str(), profile.version.as_str())
}
fn scope_matches(
    scope: &ObservationScope,
    credential: &AuthorizedCredential,
    model: &str,
    target: &ExecutionTarget,
) -> bool {
    scope.provider == credential.provider
        && scope.credential_profile == credential.profile
        && scope.credential_revision == credential.revision
        && scope.account_reference == credential.account_reference
        && scope.endpoint_reference == credential.endpoint_reference
        && scope.model.as_deref().is_none_or(|m| m == model)
        && match (&scope.node_id, target) {
            (None, _) => true,
            (Some(id), ExecutionTarget::Worker { node_id, .. }) => id == node_id,
            _ => false,
        }
}
fn availability(
    input: &SelectionInputs,
    credential: &AuthorizedCredential,
    model: &str,
    policy: &SelectionPolicy,
) -> Result<(CandidateState, Vec<String>, Vec<uuid::Uuid>)> {
    let mut groups = BTreeMap::<String, &AvailabilityObservation>::new();
    for observation in &input.observations {
        if let ObservationSource::Worker {
            node_id, boot_id, ..
        } = &observation.source
            && !matches!(&input.project.target,ExecutionTarget::Worker{node_id:current_node,boot_id:current_boot} if current_node==node_id && current_boot==boot_id)
        {
            continue;
        }
        if !scope_matches(&observation.scope, credential, model, &input.project.target) {
            continue;
        }
        let scope = digest(&(&observation.scope, &observation.source))?
            .as_str()
            .to_string();
        let rank = |o: &AvailabilityObservation| (o.sequence, o.received_at_ms, o.observation_id);
        if groups
            .get(&scope)
            .is_none_or(|old| rank(observation) > rank(old))
        {
            groups.insert(scope, observation);
        }
    }
    let mut positive = false;
    let mut ids = Vec::new();
    let mut blocked = None;
    for observation in groups.into_values() {
        ids.push(observation.observation_id);
        let timely = fresh(
            observation.received_at_ms,
            observation.expires_at_ms,
            input.now_ms,
        );
        let cooldown = timely && observation.cooldown_until_ms > input.now_ms;
        let observed =
            timely && input.now_ms - observation.received_at_ms <= policy.observation_max_age_ms;
        if !observed && !cooldown {
            continue;
        }
        let state = match observation.state {
            Availability::Available => {
                positive = true;
                continue;
            }
            Availability::AuthenticationFailed => CandidateState::MissingCredentials,
            Availability::ModelUnsupported => CandidateState::Unsupported,
            Availability::QuotaUnavailable | Availability::TemporaryUnavailable => {
                CandidateState::Unavailable
            }
        };
        // Stable, conservative precedence across independent matching scopes.
        let priority = |s: CandidateState| match s {
            CandidateState::MissingCredentials => 3,
            CandidateState::Unsupported => 2,
            _ => 1,
        };
        if blocked.is_none_or(|old| priority(state) > priority(old)) {
            blocked = Some(state);
        }
    }
    ids.sort();
    if let Some(state) = blocked {
        return Ok((
            state,
            vec![
                match state {
                    CandidateState::MissingCredentials => "provider_authentication_failed",
                    CandidateState::Unsupported => "provider_model_unsupported",
                    _ => "provider_cooldown_or_unavailable",
                }
                .into(),
            ],
            ids,
        ));
    }
    if positive {
        return Ok((
            CandidateState::Eligible,
            vec!["availability_observed".into()],
            ids,
        ));
    }
    if policy.allow_unknown_availability {
        Ok((
            CandidateState::Eligible,
            vec!["unknown_availability_bounded_attempt".into()],
            ids,
        ))
    } else {
        Ok((
            CandidateState::StaleUnknown,
            vec!["availability_stale_or_unknown".into()],
            ids,
        ))
    }
}
fn fallback(state: CandidateState) -> Option<FallbackReason> {
    match state {
        CandidateState::Eligible => None,
        CandidateState::MissingCredentials => Some(FallbackReason::MissingCredentials),
        CandidateState::Unavailable => Some(FallbackReason::Unavailable),
        CandidateState::Unsupported => Some(FallbackReason::Unsupported),
        CandidateState::StaleUnknown => Some(FallbackReason::StaleUnknown),
        CandidateState::PolicyDenied => Some(FallbackReason::PolicyDenied),
    }
}
fn global_budget(input: &SelectionInputs, policy: &SelectionPolicy) -> Option<&'static str> {
    let budget = &input.budget;
    if input.now_ms >= input.project.expires_at_ms {
        return Some("project_admission_snapshot_stale");
    }
    if !fresh(budget.observed_at_ms, budget.expires_at_ms, input.now_ms) {
        return Some("lineage_budget_stale");
    }
    if budget.attempts_started >= policy.max_attempts {
        return Some("lineage_retry_budget_exhausted");
    }
    if budget.elapsed_ms.is_none()
        || budget
            .elapsed_ms
            .is_some_and(|ms| ms >= policy.max_lineage_elapsed_ms)
    {
        return Some("lineage_time_budget_exhausted_or_unknown");
    }
    if budget.attempts_started > 0 && budget.last_attempt_id.is_none() {
        return Some("lineage_predecessor_unknown");
    }
    if budget.attempts_started > 0
        && input.replacement.as_ref().is_none_or(|e| {
            !fresh(e.observed_at_ms, e.expires_at_ms, input.now_ms)
                || Some(e.predecessor_attempt_id) != budget.last_attempt_id
        })
    {
        return Some("replacement_stop_retention_reconciliation_required");
    }
    None
}
fn cost(
    input: &SelectionInputs,
    policy: &SelectionPolicy,
    harness: &InstalledHarness,
    estimate: Option<&CandidateEstimate>,
) -> std::result::Result<(), (CandidateState, &'static str)> {
    let project = &input.project.ceilings;
    let p = &policy.budget;
    let budget = &input.budget;
    let hard = project.require_hard_cost_enforcement || p.require_hard_cost_enforcement;
    if hard
        && (harness.limits.hard_cost_enforcer.is_none()
            || budget.hard_budget_provider.is_none()
            || budget.accounting != AccountingCompleteness::Complete)
    {
        return Err((
            CandidateState::Unsupported,
            "hard_cost_enforcement_unqualified",
        ));
    }
    if p.require_complete_accounting && budget.accounting != AccountingCompleteness::Complete {
        return Err((CandidateState::PolicyDenied, "complete_accounting_required"));
    }
    if project.currency.is_some() && p.currency.is_some() && project.currency != p.currency {
        return Err((CandidateState::PolicyDenied, "budget_currency_mismatch"));
    }
    let limit = match (project.cost_limit_microunits, p.max_cost_microunits) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let currency = project.currency.as_ref().or(p.currency.as_ref());
    if let Some(limit) = limit {
        if budget
            .currency
            .as_ref()
            .is_some_and(|known| Some(known) != currency)
            || estimate.is_some_and(|e| Some(&e.currency) != currency)
        {
            return Err((CandidateState::PolicyDenied, "budget_currency_mismatch"));
        }
        if budget.spent_microunits.is_some_and(|spent| spent >= limit) {
            return Err((
                CandidateState::PolicyDenied,
                "lineage_cost_budget_exhausted",
            ));
        }
        if budget.spent_microunits.is_none()
            || estimate.is_none()
            || budget.accounting != AccountingCompleteness::Complete
        {
            if !hard
                && !p.require_complete_accounting
                && p.unknown_cost == UnknownCostPolicy::AllowBounded
            {
                return Ok(());
            }
            return Err((CandidateState::PolicyDenied, "cost_unknown"));
        }
        let estimate = estimate.unwrap();
        if budget.currency.as_ref() != currency || Some(&estimate.currency) != currency {
            return Err((CandidateState::PolicyDenied, "budget_currency_mismatch"));
        }
        if budget
            .spent_microunits
            .unwrap()
            .checked_add(estimate.microunits)
            .is_none_or(|sum| sum > limit)
        {
            return Err((
                CandidateState::PolicyDenied,
                "lineage_cost_budget_exhausted",
            ));
        }
    }
    Ok(())
}
fn trace(
    id: String,
    c: &aq_profiles::ModelCandidate,
    harness: &str,
    environment: &str,
    credential: Option<&AuthorizedCredential>,
    state: CandidateState,
    reason: &str,
) -> CandidateTrace {
    CandidateTrace {
        candidate_id: id,
        profile_candidate_id: c.id.clone(),
        provider: c.provider.clone(),
        model: c.model.clone(),
        harness: harness.into(),
        environment: environment.into(),
        credential_profile: credential.map(|c| c.profile.clone()),
        state,
        reason_codes: vec![reason.into()],
        observation_ids: vec![],
    }
}
pub fn select(
    catalog: &aq_profiles::ValidatedCatalog,
    input: &SelectionInputs,
) -> Result<SelectionOutcome> {
    for composition in &input.compositions {
        composition.composition.validate_native_v2().map_err(|_| {
            SelectionError::Invalid("native selection requires schema2 composition".into())
        })?;
    }
    select_internal(catalog, input)
}
/// Historical schema1 inspection/replay compatibility only. Never use this
/// entrypoint for a new native admission or offer fence.
pub fn select_legacy_for_inspection(
    catalog: &aq_profiles::ValidatedCatalog,
    input: &SelectionInputs,
) -> Result<SelectionOutcome> {
    ensure(
        input.compositions.iter().all(|c| {
            c.composition.schema_version == 1
                && c.composition.graph_access_policy_digest.is_none()
                && c.composition.graph_access_policy_kind.is_none()
                && c.composition.requested_permissions.is_none()
        }),
        "legacy composition shape",
    )?;
    select_internal(catalog, input)
}
fn select_internal(
    catalog: &aq_profiles::ValidatedCatalog,
    input: &SelectionInputs,
) -> Result<SelectionOutcome> {
    validate(catalog, input)?;
    let definitions = &catalog.artifact().catalog;
    let agent = definitions
        .agents
        .iter()
        .find(|a| aq_profiles::key(&a.id, a.version) == input.project.agent_profile)
        .ok_or_else(|| SelectionError::Invalid("agent absent".into()))?;
    ensure(
        agent.intelligence == input.project.intelligence_profile
            && agent.execution == input.project.execution_profile,
        "profile combination mismatch",
    )?;
    let intelligence = definitions
        .intelligence
        .iter()
        .find(|i| aq_profiles::key(&i.id, i.version) == agent.intelligence)
        .unwrap();
    let execution = definitions
        .execution
        .iter()
        .find(|e| aq_profiles::key(&e.id, e.version) == agent.execution)
        .unwrap();
    let Some(policy) = &intelligence.selection_policy else {
        return Ok(SelectionOutcome {
            resolved: None,
            traces: vec![],
            reason_codes: vec!["selection_policy_missing".into()],
        });
    };
    policy
        .validate()
        .map_err(|e| SelectionError::Invalid(e.to_string()))?;
    let global = global_budget(input, policy);
    let mut rows: Vec<(CandidateTrace, Option<ConcreteChoice>, u64)> = Vec::new();
    let mut harnesses: Vec<_> = input.node.harnesses.iter().collect();
    harnesses.sort_by_key(|h| {
        (
            &h.harness,
            h.environment.digest.clone(),
            h.adapter.digest.clone(),
            h.cli_version.clone(),
        )
    });
    for candidate in &intelligence.candidates {
        let before = rows.len();
        for name in &execution.harnesses {
            if !candidate.harnesses.contains(name) {
                continue;
            }
            for harness in harnesses.iter().filter(|h| {
                &h.harness == name
                    && (execution
                        .environments
                        .contains(&profile_key(&h.environment))
                        || execution
                            .environments
                            .iter()
                            .any(|e| e == h.environment.id.as_str()))
            }) {
                let Some(composition) = input.compositions.iter().find(|c| {
                    &c.composition.harness == name
                        && (c.composition.environment == harness.environment.id.as_str()
                            || c.composition.environment == profile_key(&harness.environment))
                }) else {
                    continue;
                };
                let references = if execution.credential_profile_references.is_empty() {
                    vec!["unconfigured".to_string()]
                } else {
                    execution.credential_profile_references.clone()
                };
                for reference in references {
                    if input.credentials.iter().any(|c| {
                        profile_key(&c.profile) == reference && c.provider != candidate.provider
                    }) {
                        continue;
                    }
                    ensure(rows.len() < 128, "candidate fanout exceeds published bound")?;
                    let credential = input.credentials.iter().find(|c| {
                        profile_key(&c.profile) == reference && c.provider == candidate.provider
                    });
                    let id = format!(
                        "choice:{}",
                        digest(&(
                            candidate.id.as_str(),
                            &harness.adapter,
                            &harness.environment,
                            &harness.cli_version,
                            &reference
                        ))?
                        .as_str()
                    );
                    let mut row = trace(
                        id,
                        candidate,
                        name,
                        &profile_key(&harness.environment),
                        credential,
                        CandidateState::Eligible,
                        "candidate_evaluated",
                    );
                    let mut choice = None;
                    let evaluated =
                        (|| -> std::result::Result<(), (CandidateState, &'static str)> {
                            if let Some(reason) = global {
                                return Err((CandidateState::PolicyDenied, reason));
                            }
                            if !input.project.allowed_environments.iter().any(|e| {
                                e == harness.environment.id.as_str()
                                    || e == &profile_key(&harness.environment)
                            }) {
                                return Err((
                                    CandidateState::PolicyDenied,
                                    "project_environment_denied",
                                ));
                            }
                            if !fresh(
                                input.node.observed_at_ms,
                                input.node.expires_at_ms,
                                input.now_ms,
                            ) {
                                return Err((
                                    CandidateState::StaleUnknown,
                                    "node_capability_snapshot_stale",
                                ));
                            }
                            let credential = credential.ok_or((
                                CandidateState::MissingCredentials,
                                "credential_profile_unregistered",
                            ))?;
                            if !credential.enabled {
                                return Err((
                                    CandidateState::PolicyDenied,
                                    "credential_profile_disabled",
                                ));
                            }
                            if !credential.provisionable {
                                return Err((
                                    CandidateState::MissingCredentials,
                                    "credential_not_provisionable",
                                ));
                            }
                            let missing = if credential.material_owner.is_some() {
                                "credential_preparation_missing_or_stale"
                            } else {
                                "installed_credential_missing_or_stale"
                            };
                            let delivery = credential_delivery(input, credential)
                                .ok_or((CandidateState::MissingCredentials, missing))?;
                            require_capabilities(
                                &policy.required_capabilities,
                                &harness.capabilities,
                            )
                            .map_err(|_| {
                                (
                                    CandidateState::Unsupported,
                                    "required_capability_unsupported",
                                )
                            })?;
                            let model = harness
                                .models
                                .iter()
                                .find(|m| {
                                    m.provider == candidate.provider
                                        && m.model == candidate.model
                                        && m.reasoning_configuration_reference
                                            == candidate.reasoning_configuration_reference
                                })
                                .ok_or((
                                    CandidateState::Unsupported,
                                    "model_or_reasoning_unsupported",
                                ))?;
                            let limits = &harness.limits;
                            if limits.context_bytes == 0
                                || limits.wall_time_ms == 0
                                || limits.memory_bytes == 0
                                || limits.cpu_millis == 0
                                || limits.process_limit == 0
                                || limits.max_output_tokens < policy.max_output_tokens
                            {
                                return Err((
                                    CandidateState::Unsupported,
                                    "required_native_limit_unsupported",
                                ));
                            }
                            let mut resources = input.project.ceilings.clone();
                            resources.context_bytes = resources
                                .context_bytes
                                .min(limits.context_bytes)
                                .min(composition.composition.context_limit_bytes);
                            resources.wall_time_ms =
                                resources.wall_time_ms.min(limits.wall_time_ms);
                            resources.memory_bytes =
                                resources.memory_bytes.min(limits.memory_bytes);
                            resources.cpu_millis = resources.cpu_millis.min(limits.cpu_millis);
                            resources.process_limit =
                                resources.process_limit.min(limits.process_limit);
                            let bytes: u64 = composition
                                .composition
                                .instruction_order
                                .iter()
                                .filter(|i| i.delivery == aq_profiles::Delivery::Startup)
                                .map(|i| i.text.len() as u64)
                                .sum();
                            if bytes > resources.context_bytes {
                                return Err((
                                    CandidateState::Unsupported,
                                    "required_context_limit_unsupported",
                                ));
                            }
                            let estimate = input.estimates.iter().find(|e| {
                                e.credential_profile == credential.profile
                                    && e.provider == candidate.provider
                                    && e.model == candidate.model
                            });
                            cost(input, policy, harness, estimate)?;
                            let (state, reasons, ids) =
                                availability(input, credential, &candidate.model, policy).map_err(
                                    |_| (CandidateState::PolicyDenied, "observation_invalid"),
                                )?;
                            row.state = state;
                            row.reason_codes = reasons;
                            row.observation_ids = ids;
                            if state != CandidateState::Eligible {
                                return Ok(());
                            }
                            if delivery.is_some() {
                                row.reason_codes
                                    .push("material_preparation_required".into());
                            }
                            choice = Some(ConcreteChoice {
                                candidate_id: row.candidate_id.clone(),
                                profile_candidate_id: candidate.id.clone(),
                                provider: candidate.provider.clone(),
                                model: candidate.model.clone(),
                                native_reasoning: model.native_reasoning.clone(),
                                reasoning_configuration_reference: candidate
                                    .reasoning_configuration_reference
                                    .clone(),
                                harness: harness.harness.clone(),
                                cli_version: harness.cli_version.clone(),
                                adapter: harness.adapter.clone(),
                                environment: harness.environment.clone(),
                                qualification: harness.qualification.clone(),
                                failure_classifier: harness.failure_classifier.clone(),
                                credential_profile: credential.profile.clone(),
                                credential_delivery: delivery,
                                account_reference: credential.account_reference.clone(),
                                endpoint_reference: credential.endpoint_reference.clone(),
                                composition_digest: composition.digest.clone(),
                                resources,
                                max_output_tokens: policy.max_output_tokens,
                                estimate: estimate.cloned(),
                            });
                            Ok(())
                        })();
                    if let Err((state, reason)) = evaluated {
                        row.state = state;
                        row.reason_codes = vec![reason.into()];
                    }
                    rows.push((row, choice, credential.map_or(0, |c| c.revision)));
                }
            }
        }
        if before == rows.len() {
            ensure(rows.len() < 128, "candidate fanout exceeds published bound")?;
            let id = format!(
                "choice:{}",
                digest(&(&candidate.id, "no-compatible-composition"))?.as_str()
            );
            rows.push((
                trace(
                    id,
                    candidate,
                    "unavailable",
                    "unavailable",
                    None,
                    CandidateState::Unsupported,
                    "compatible_installed_composition_unavailable",
                ),
                None,
                0,
            ));
        }
    }
    let mut blocked = false;
    let mut selected = None;
    for (index, (row, choice, _)) in rows.iter_mut().enumerate() {
        if selected.is_some() {
            continue;
        }
        if blocked {
            row.state = CandidateState::PolicyDenied;
            row.reason_codes.push("fallback_not_permitted".into());
            continue;
        }
        if row.state == CandidateState::Eligible && choice.is_some() {
            selected = Some(index);
        } else if fallback(row.state).is_some_and(|reason| !policy.fallback_on.contains(&reason)) {
            blocked = true;
        }
    }
    let traces: Vec<_> = rows.iter().map(|(trace, _, _)| trace.clone()).collect();
    let resolved = if let Some(index) = selected {
        let (_, choice, credential_revision) = &rows[index];
        let mut expires = input
            .node
            .expires_at_ms
            .min(input.budget.expires_at_ms)
            .min(input.project.expires_at_ms)
            .min(input.now_ms.saturating_add(2000));
        for binding in &input.node.credential_bindings {
            if binding.credential_profile == choice.as_ref().unwrap().credential_profile
                && binding.credential_revision == *credential_revision
            {
                expires = expires.min(binding.expires_at_ms);
                if choice.as_ref().unwrap().credential_delivery.is_some()
                    && let Some(preparation) = &binding.preparation
                {
                    expires = expires.min(preparation.expires_at_ms);
                }
            }
        }
        // Relevant availability changes are fenced by the server; expiration of
        // any influencing fresh observation cannot silently extend this decision.
        for id in traces
            .iter()
            .take(index + 1)
            .flat_map(|t| &t.observation_ids)
        {
            if let Some(o) = input.observations.iter().find(|o| &o.observation_id == id) {
                if o.expires_at_ms > input.now_ms {
                    expires = expires.min(o.expires_at_ms);
                }
                let age_deadline = o
                    .received_at_ms
                    .saturating_add(policy.observation_max_age_ms)
                    .saturating_add(1);
                if age_deadline > input.now_ms {
                    expires = expires.min(age_deadline);
                } else if o.cooldown_until_ms > input.now_ms {
                    expires = expires.min(o.cooldown_until_ms);
                }
            }
        }
        Some(ResolvedSelection {
            schema_version: 1,
            lineage: input.project.lineage.clone(),
            target: input.project.target.clone(),
            choice: choice.clone().unwrap(),
            snapshot_digest: digest(input)?,
            catalog_digest: input.project.catalog_digest.clone(),
            project_revision: input.project.revision,
            catalog_revision: input.project.catalog_revision,
            node_revision: input.node.revision,
            registry_revision: input.registry_revision,
            credential_revision: *credential_revision,
            budget_revision: input.budget.revision,
            selected_at_ms: input.now_ms,
            expires_at_ms: expires,
            traces: traces.clone(),
        })
    } else {
        None
    };
    Ok(SelectionOutcome {
        reason_codes: if resolved.is_some() {
            vec![]
        } else if blocked {
            vec!["no_permitted_fallback".into()]
        } else {
            vec!["no_eligible_candidate".into()]
        },
        resolved,
        traces,
    })
}

fn credential_delivery(
    input: &SelectionInputs,
    credential: &AuthorizedCredential,
) -> Option<Option<CredentialDelivery>> {
    let mut installed = false;
    for binding in &input.node.credential_bindings {
        if binding.credential_profile != credential.profile
            || binding.credential_revision != credential.revision
            || !fresh(binding.observed_at_ms, binding.expires_at_ms, input.now_ms)
        {
            continue;
        }
        installed |= binding.available;
        if let (Some(preparation), Some(owner), Some(supported)) = (
            &binding.preparation,
            &credential.material_owner,
            &input.node.material_delivery,
        ) && preparation.owner == *owner
            && preparation.credential_profile == credential.profile
            && preparation.credential_revision == credential.revision
            && preparation.target == input.project.target
            && matches!(preparation.target, ExecutionTarget::Worker { .. })
            && preparation.delivery.adapter == supported.adapter
            && preparation.delivery.version == 1
            && supported.version == 1
            && fresh(
                preparation.observed_at_ms,
                preparation.expires_at_ms,
                input.now_ms,
            )
        {
            return Some(Some(CredentialDelivery::ServerLease {
                owner: owner.clone(),
                adapter: supported.adapter.clone(),
                version: 1,
            }));
        }
    }
    installed.then_some(None)
}
