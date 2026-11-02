# Minimum execution closure: retained work, review and Graph

This source audit supplements the B1 phase-origin plan. It is not a claim of a
completed task run. Main at `2a1428a` has schema37; the isolated B1 candidate
`440298f` proposes execution schema38. Canonical services and data were not read
or changed for this audit. The unrelated wait-owner38 candidate remains isolated.

## Shortest honest pilot

1. Existing `profiled_execution/producer/build.rs` creates a sealed execution
   specification and exact `SourceChangeReviewed` expected outcome with
   `HumanOwner` review and `required_checks=[]`.
2. The worker executes the exact offered task in its assigned workspace and
   accepts a bounded local result **claim** while running. The worker must retain
   that claim before transport, stop the actual host, and retain/export the exact
   changed checkpoint before assembling its final immutable report. Existing
   `native/runner/codex.rs` currently reaches phase Stopped; the new native result
   producer/local command integration is separate work, not supplied by B1.
3. Reuse worker `source.rs` checkpoint/export, immutable source manifests and
   source-transfer chunks through native journal ownership. Reuse server
   `source_retention.rs` begin/chunk/finalize and `load_confirmed_locked`; do not
   relabel old Runtime/source state or a mere checkpoint acknowledgment as central
   source retention.
4. B1 `profiled_execution/results.rs` records the unchanged `ResultReport` only
   against actual stopped/outcome owners. The phase bridge additionally verifies
   the exact nominal execution, registration and full outer Started/Stopped
   history. Recording means Recorded, not successful settlement. Same exact POST
   retries recover its immutable receipt after a lost response.
5. Existing `reviews::Service::{prepare,admit,resolve}` binds a real retained
   report/source confirmation and current source applicability to a separate human
   decision. Narrative claims about tests are not check receipts. A first pilot
   with no configured required checks is honest; nonempty requirements must refuse
   until a real check executor/result owner exists.
6. B2 must independently inspect the exhaustive actual effect ledger, account for
   mailbox effects or refuse unsupported messaging, obtain any separate external
   effect disposition, verify the review and current exact task/source basis.
   Source-authored `settlement/{eligibility,effects}.rs` is a separate candidate;
   missing production ports cannot be treated as success.
7. C must retain a dedicated immutable Graph completion intent, apply only the
   original expected revision through actual Graph authority, recover response
   loss, and atomically acknowledge the confirmed receipt/clear the task claim.
   Completion delivery must not mutate the already frozen execution-effect
   digest. This join and a verified-release producer remain absent. SQL38 keeps
   phase releases unconditionally blocked until they qualify.

Human Results metadata and supplemental PNG media retain their explicit
`recorded_legacy_native_only` scope. B1's broader internal historical loader does
not silently make those public projections phase-capable.

## Reusable v1 behavior

Pinned source: `agent-queue2` commit
`9a9b8e8ef992d1ff699755a9383ed3c91e2844ea`. Retrieve with `git show`, not the
current v1 working tree. The existing
[implementation inventory](../research/2026-10-30-v1-implementation-reference-map.md)
retains source hashes and symbols. No v1 tests were run in this audit.

| Pinned v1 source | Behavior worth preserving in v2 |
| --- | --- |
| `src/orchestrator/execution.py::complete_session_task` (1310), completion/source join around1830, committed event publication around2199 | Serialize completion against the actual task claim/epoch, bind retained published source, emit completion only after the real committed state. Do not reuse its broad close entry point as agent authority. |
| `src/orchestrator/stranded_work.py::save_wip_to_task_branch` (248) | Stop first; preserve unknown/failed retention, never overwrite divergence or reinterpret missing workspace as clean. |
| `src/integration/reviews.py` (`ReviewSubject`, `TreeReviews`, `ReviewEvidenceProducer`) | Bind review to the exact repository/tree/head and reviewer authority; changed work invalidates prior evidence. |
| `src/integration/close_checks.py::check_candidate` | Execute finite actual checks against the exact candidate; missing/unavailable checks never mean pass. |

Useful frozen regressions to translate, rather than importing v1 orchestration:

- `test_stranded_work.py::test_wip_save_never_forces_a_diverged_task_branch` and
  `test_missing_workspace_is_unknown_not_clean`.
- `test_integration_review_evidence.py::test_tree_review_cannot_be_inherited_by_changed_tree_with_the_same_paths`,
  `test_approval_transaction_crash_rolls_back_evidence_and_transition`, and
  `test_service_eligibility_and_ancestry_never_supply_a_tree_review`.
- `test_task_close_completion_recovery.py` and
  `test_integration_completion_recovery.py` for response-loss/tail recovery.

These are behavioral references. They do not establish physical host safety,
provider qualification, checked-source retention or v2 end-to-end completion.
