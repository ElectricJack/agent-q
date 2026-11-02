# AQ-10 first-pilot settlement owner

Status: proposal for Root review, after execution result-origin migration38.
Number39 is reserved; no DDL, route, release or Graph completion is implemented
by this document. The current source increment is finite eligibility planning and
read-only AQ effect-ledger inspection. It cannot emit an execution grant or clear
a task claim. Architecture0002 sections5–7 and AQ-10-result-settlement-plan remain
normative. The physical runner, B1 recording and this settlement owner are distinct.

## Small supported policy

Only `source_change_reviewed`, `human_owner`, and an explicitly empty
`required_checks` set qualify for this first owner. The installed, exact
validation-policy definition must declare
`source_change_reviewed_human_owner_no_checks_v1`; unknown policy bodies, policy
actors and nonempty check requirements refuse. No check execution is invented.

The complete admitted mutable Graph input set must be exactly the task itself:
one task node, no relationships/dependencies, no other mutable context records.
Do not trim context to satisfy this rule. Additional inputs must be actual
immutable content-addressed source/catalog artifacts. Broader tasks refuse until
Graph can atomically assert all their required input revisions. Completing the
task with its ORIGINAL expected CAS version then fences the whole supported
mutable set; no no-op writes or read-new-version overwrite are required.

The code review pins the exact retained resulting commit and import artifact,
report/stop/outcome/spec/context/task/project/policy references. A commit's Git
identity fixes its tree. The pilot conservatively refuses changed commits rather
than implementing v1's unchanged-tree approval reuse. Review is not merge,
publication, executed checks, process success, or task completion.

## Effect basis and one explicit review

Under actual stopped Node/attempt/hot locks, reload the actual B1 report head and
sealed agent profile, then inspect EVERY `agent_graph_operations` row for the
attempt. Existing maximum is4096 rows; inspect4097 to detect overflow, and bound
logical inspected bytes independently to32MiB. Lock metadata before fetching JSON.
Only consistent confirmed/conflict/rejected owner records with no active dispatch
lease settle. Pending/dispatching/unknown rows hold. Use the existing AQ-07
cancel/reconciliation owner and exact operation ID/bytes to resolve them. Never
take an operation lock and subsequently acquire a Node/attempt lock.

The first pilot's immutable agent profile permits only the six scoped Graph read
capabilities and disables messaging. Still query the actual indexed mailbox
sender namespace derived from its exact frozen binding. Any retained mail
operation or messaging capability refuses this pilot; it is not silently omitted.
The report's `pending_effect_ids` is a claim to reconcile, never a complete
inventory. Database reads, message delivery, and independent later owner actions
are not relabeled as Graph mutation absence.

This ledger establishes the scoped AQ owner basis, not absence of arbitrary
shell/network/external effects. Where the selected actual runtime and closed
effect owners independently qualify a machine-derived source-only disposition,
that actual owner can produce a distinct effect-evidence view. Do not claim such
prevention merely from worktree settings, process exit, an omitted list, or the
word "read-only" in a model instruction.

For the trusted-node coding pilot requiring explicit reconciliation, use ONE NEW
`source_and_effects_review_v1` subject/profile through the existing protected
decision service, current-owner authentication, exact-revision resolution and
idempotent receipt machinery. Do not create another generic approval framework.
The complete new subject contains:

- the exact code-review subject and its original report/stop/retained-source pins;
- the actual sealed runtime/tool/agent-profile reference and exhaustive AQ ledger
  digest/count, including explicit messaging-disabled disposition;
- a REQUIRED explicit external-effect declaration: inspected scope/resources,
  known effects and their reconciled evidence, reason and residual uncertainty;
- exact installed policy and current eligible human-owner/project revision.

The human sees and approves this combined scope explicitly. A missing declaration
is not an empty declaration. Unresolved AQ operations or conflicting external
effects cannot be overridden into success. Any permitted residual uncertainty is
recorded as a scoped reconciliation decision, not universal security evidence.
Old `code_review_v1` Approve remains code review only. Two private evidence views
may share a receipt ONLY when both pin this exact NEW complete combined subject.
Neither the worker nor a playbook can impersonate the human answer.

Existing code-review rows/AAD/wire remain unchanged. Add a protected subject
profile discriminator (default old profile) and dispatch the actual typed subject
inside the existing service; filter old inbox/read adapters to their profile.
The new detail/action must display the effect scope before approval. Historical
answers remain immutable; a changed report/source/ledger needs a new subject.

## Actual applicability and acceptance

Read actual Graph context/task and live result retention OUTSIDE SQL. Require the
complete supported task-only input set and original task CAS/logical revisions;
compare the immutable admitted context, catalog, source and input references.
Fresh current-Git retention remains an actual `ResultRetentionObservation`, not
a historical import receipt supplied by the caller.

In the final transaction reload installed policy/project, common stopped
authority, current B1 head, exact source confirmation, verified review/effect
views and the exhaustive AQ ledger. Validate all original pins and observation
ages after waits. Serialize cancellation with acceptance. Cancellation before
acceptance refuses success; a later cancellation does not rewrite that immutable
acceptance and may block an undispatched completion under explicit policy.

Retain a bounded immutable evaluation operation/receipt. Same ID/bytes returns
the original result; changed evidence is a new explicit evaluation. Missing
adapters or stale evidence return unavailable/stale/needs-reconciliation, never
success-shaped defaults. The present source intentionally leaves positive
external-disposition/current-applicability constructors unavailable until these
actual owners exist.

## Minimal schema39 and completion owner

Reuse existing source/result/review/stop tables and `profiled_verified_releases`.
The new schema needs only protected subject-profile discrimination, immutable
evaluation/release links to those actual owners, and a dedicated bounded
completion intent/receipt/outbox. Preserve SQL13/33/38 historical definitions;
extend the phase release refusal ONLY for the exact new owner join. No arbitrary
DTO or completeness-shaped JSON authorizes the release row.

Freeze these first-owner envelopes before SQL qualification:

| Retained owner | Per-attempt maximum | Payload envelope |
| --- | --- | --- |
| New combined review subjects |16 |existing review64KiB canonical subject bound, including required external declaration; no larger legacy envelope |
| External declaration within combined subject |1 per subject |8KiB total, at most16 resource/effect entries, required reason/residual-uncertainty fields |
| Immutable evaluation operations |16; at most1 accepted |128KiB aggregate canonical request/receipt/evidence per operation; reserve final receipt before acceptance |
| Completion intent |1, bound to accepted evaluation |64KiB total frozen payload/service mapping, status, compact verified receipt and completion outbox reservation |
| Completion dispatch |8 attempts,60s automatic elapsed budget |3s network attempt;15s recorded lease; exhaustion holds for exact-ID manual reconciliation |

Existing review global/lineage quotas remain in force; this adds the smaller
combined-subject attempt cap. All histories remain retained at capacity. A
reevaluation consumes a new operation; unchanged replay consumes none. The
completion owner reads at most1MiB from Graph and retains a bounded verified
compact receipt plus exact response digest, as AQ-07 does, rather than copying
arbitrary upstream bodies into its64KiB envelope. No quota failure may discard
an already-possible operation or prevent recording its final receipt/outbox.

After an accepted evaluation, the winning report, source, review and ledger pins
are immutable. Reject further B1 head changes under the SAME common authority
lock and a schema39 head guard; old exact report replay remains available. This
applies even while Graph is only prepared/possible/unknown. The dispatcher reads
the accepted record, never the current report head to retarget an old intent.
Nonaccepted evaluations may be followed by new report heads/evaluation IDs.

Set actual retained-source and reconciled-effect hot projections and record the
verified release, but keep `holds_task=true` while Graph completion is pending.
Existing publication admits another task attempt when that claim is false. The
pilot therefore avoids a scheduler change and keeps this exclusion until actual
completion is confirmed. Retention-handle retirement is a separate later action.

The internal completion owner freezes one canonical `update_node` payload with
the ORIGINAL task `expected_version`, `task_state=completed`, stable operation /
idempotency ID, and exact configured Graph service identity. It reuses AQ-07's
fixed public transport and bounded response/replay validation. It does not expand
agent command authority or require GraphActive for the stopped execution.

Do NOT place this settlement operation in `agent_graph_operations`: it is a new
owner effect after the execution ledger was frozen. Use its dedicated finite
record with prepared/possible/confirmed/conflict disposition, bounded bytes,
attempts and lease. No remote I/O occurs under SQL authority locks. Unknown
delivery queries/replays exact original bytes/ID; version conflicts remain held
conflicts, never a read-new-version loop. Changed transport identity holds.

Validate the actual Graph change-set/task/version/logical-revision receipt. In
one final AQ transaction retain that receipt, publish one logical completion
outbox event, and clear the common claim using the verified release. Reply loss
and outbox replay cannot duplicate logical completion. Stale/unknown/conflicting
outcomes preserve source/evidence and the task exclusion.

## Required focused qualification

Keep owner-layer tests bounded: actual stopped/B1/ledger joins and lock order;
pending/unknown/malformed/oversized operation refusal; disabled messaging with
actual mailbox history; code-only approval cannot satisfy effects; exact new
combined approval/current-owner revocation; report/source/ledger/policy changes;
task-only context refusal for neighbors/edges/incomplete coverage; fresh retention
and cancellation races; response loss/exact replay/conflict; atomic Graph receipt /
outbox / claim-clear rollback; no reassignment while completion is pending.

Use disposable registered databases and actual Graph/Git owner fixtures. Clearly
label synthetic ledger/proof rows. Pure planning, raw SQL fixtures, process exit
and already-passing static output tests do not qualify settlement or a real
provider. Real qualified agent work, retained result, explicit human decision and
confirmed expected-version Graph completion remain the end-to-end gate.
