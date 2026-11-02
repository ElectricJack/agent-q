# First-pilot combined review and settlement HTTP ports

This is a thin protected adapter over the existing combined-review and settlement owners. It adds no schema, decision admission, automatic answer, scheduler or eligibility logic. A real human must choose the answer; synthetic answers in component tests are not physical execution or human-review qualification.

The existing optional `reviews::admin_routes` injection mounts these routes beneath the daemon's current identity, origin/CSRF, admission, maintenance and response-hardening layers. Default constructors without a configured reviews owner keep every new route absent. Handlers pass the complete authenticated `store::User` from `RequireOwner`; they never substitute a UUID, worker or agent credential. Settlement uses the injected reviews owner's actual AppState and Service.

| Method and path | Body | Actual owner call |
|---|---|---|
| `GET /api/reviews/combined/{decision_id}` | none | `inspect_combined_as(&User, decision_id)` |
| `POST /api/reviews/combined/{decision_id}/resolve` | `{"expected_subject": ArtifactRef, "answer": ResolveRequest}` | `resolve_combined_as(&User, &answer, &expected_subject)` |
| `POST /api/settlement/evaluations` | existing `EvaluateRequest` | `settlement::Service::evaluate(&User, &request)` |
| `GET /api/settlement/evaluations/{operation_id}` | none | `settlement::Service::lookup(&User, operation_id)` |
| `POST /api/settlement/completions/{operation_id}/reconcile` | exactly `{}` | `settlement::Service::complete(&User, operation_id)` |

The resolve envelope rejects unknown/duplicate fields, requires path ID equal to `answer.decision_id`, and requires the complete expected subject reference. The owner checks the combined profile and current retained subject, principal, decision revision and exact operation identity, including replay. Existing code-only endpoints and inbox keep their original wire and exclude combined subjects.

All mutation bodies use `ApiJson` and the existing 16KiB review route limit. Mutations require `X-AQ-Request: 1`; a supplied Origin must equal the configured public origin. Reconcile has a closed empty body to prevent introducing an unbound choice or dispatch parameter. It can send the already admitted Graph completion effect; GET never invokes it. Lookup returns the immutable evaluation receipt and does not refresh eligibility or dispatch work. Complete/reconcile uses the existing finite original operation, current actor and configured service identity; it does not invent a new completion ID or bypass the actual owner fences.

Responses are existing owner values: combined inspection JSON, `DecisionReceipt`, `EvaluationReceipt`, or `CompletionStatus`. Existing ApiError/status conventions apply: 400 malformed/path mismatch, 413 body bound, 403 unauthorized/ineligible/profile refusal, 409 subject/revision/operation conflict, 404 absent route or operation, and 503 unavailable current evidence/service. A failed mutation may have a possible effect; clients retain original IDs/body and use the owner's exact replay/lookup semantics, not a new approval or completion.

Focused qualification reuses `profiled_results/settlement_owner39` actual SQL owner fixtures with explicitly synthetic launch/Git/Graph peers. Tests cover default absence, Owner/Viewer/identity rebinding, origin and closed bodies, old/new profile separation, expected-subject mismatch, explicit answer replay, immutable evaluation lookup and exact completion recovery after a lost Graph response. No provider, canonical database, production source or external human message is involved. Parent execution and human-approval gates remain open until separately qualified.
