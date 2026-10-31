# Completion scope: real execution, session operations and policy

Date: 2026-10-31

The user selected the first three areas in the remaining-work summary: real-agent
execution, session interaction/observability, and central supervisors/playbooks.
This map identifies existing specifications and the remaining implementation
plans to complete. It supplements the [canonical task graph](0001-v2-milestones.md);
it does not replace its dependencies or mark unfinished acceptance gates complete.
The starting evidence is the [overnight checkpoint](2026-10-31-overnight-checkpoint.md).

## 1. Real-agent execution

Existing design and implementation sources:

- [Shared execution and recovery](../architecture/0002-execution-and-recovery.md).
- [Daemon execution and agent access](../specs/0002-execution-and-agent-access.md).
- [Worker execution and agent access](../../../aq-worker/docs/specs/0002-execution-and-agent-access.md).
- [Profiles, harnesses and terminals](../architecture/0003-agent-profiles-harnesses-and-terminals.md).
- [Worker profiled tmux sessions](../../../aq-worker/docs/specs/0003-profiled-tmux-sessions.md).
- [AQ-08 staged plan](../implementation/AQ-08-plan.md), supplemented by the
  [acyclic composition correction](../implementation/AQ-08-acyclic-graph-policy.md),
  [Started owner](../implementation/AQ-08-started-owner.md),
  [material delivery](../implementation/AQ-08-material-delivery.md) and
  [artifact owner](../implementation/AQ-08-inline-artifacts.md).
- [Native producer completion](../implementation/AQ-08-native-completion-plan.md),
  [versioned control delivery](../implementation/AQ-08-native-control-017.md),
  [exact0.1.7 standalone contract](../implementation/aq-admission-0.1.7-qualification.md),
  [worker runtime](../../../aq-worker/docs/implementation/WRK-08-native-runtime-plan.md),
  [durable journal/restart boundary](../../../aq-worker/docs/implementation/WRK-08-native-journal-plan.md),
  [private material installation](../../../aq-worker/docs/implementation/WRK-08-private-material-plan.md)
  and [first Codex adapter](../../../aq-worker/docs/implementation/WRK-08-codex-adapter-plan.md).
- [Result settlement stages](../implementation/AQ-10-result-settlement-plan.md)
  and [validated historical result reads](../implementation/AQ-14-result-owner-read.md).

Canonical remaining work: AQ-08, WRK-08, AQ-10 and GATE-04. AQ-28 separately
records the completed B1 report ledger/owner-read prerequisite; it does not
complete AQ-10's eligibility and settlement stages.

Complete the actual producer/consumer sequence: authorized ready/context query,
current source and project policy, composition and intelligence selection,
retained immutable inputs, durable offer, worker reservation/preparation,
verified preparation receipts, current grant, guarded tmux launch, durable Started
reporting, source/result handoff and evidence-based graph completion.

The current producer/control plans specify transaction/lock order, operation
receipts and recovery against the ordered state format25 foundation. Migration025
now owns typed source observations and authenticated native source reads; its
focused component qualification is retained separately from the earlier full
state24 run. Candidate026 belongs to human/check owners and remains unregistered. The original
AQ-08 plan's migration numbers and stage status are historical; current owner
records and exact published contracts govern.
Context planning must use qualified model-capacity/counting facts, with unknown
values explicit. It cannot invent a context window or represent unknown usage as
zero. Implement the worker's matching durable journal transitions and connect the
qualified private material installer according to the native runtime plan, then implement
the pinned first Codex adapter configuration and standalone ownership gates.
AQ-10 still needs the concrete evidence-validation and graph-settlement sequence.

Completion evidence is a real graph task producing a confined source change in a
worker tmux session, retained source and a confirmed authorized graph outcome.
Include cancellation, stale inputs, lost replies and restart recovery. Synthetic
host observations or a standalone harness process cannot close GATE-04.

## 2. Session interaction and observability

Existing design and implementation sources:

- [Shared terminal design](../architecture/0003-agent-profiles-harnesses-and-terminals.md).
- [Session operations, waits and notifications](../architecture/0004-session-operations-waits-and-notifications.md).
- [Worker observation and suspension](../../../aq-worker/docs/specs/0004-session-observation-and-suspension.md).
- [AQ-21 staged relay design](../implementation/AQ-21-design.md) and
  [current relay runtime](../implementation/AQ-21-runtime.md).
- [WRK-14 stage1](../../../aq-worker/docs/implementation/WRK-14-stage1.md).
- [Reader lease](../../../aq-worker/docs/implementation/WRK-14-reader-lease-plan.md),
  [renderer/baseline plan](../../../aq-worker/docs/implementation/WRK-14-renderer-baseline-plan.md)
  and [combined actual host qualification](../../../aq-worker/docs/implementation/WRK-08-host-union-qualification.md).
- [AQ-22 plan](../implementation/AQ-22-plan.md),
  [WRK-15 plan](../../../aq-worker/docs/implementation/WRK-15-plan.md), and
  [qualified storage correction](../../../aq-worker/docs/implementation/WRK-15-reserved-control.md).

Canonical work: AQ-21, WRK-14, WRK-09, AQ-22, WRK-15 and AQ-23.

The terminal plans specify output acquisition, epoch/sequence and snapshot
barriers, bounded buffering, reconnect/gap handling and production worker adoption.
Reader expiry and dense incremental output decoding are now qualified components;
the pure bounded renderer now has16 passing component tests and a retained actual
tmux REP counterexample that causes conservative refusal. Physical baseline,
resize barriers, snapshots and runtime adoption remain open. Input/resize qualification does not establish renderer state or
live viewing. CLI attach/peek should use the same authorized relay; a dashboard
implementation is outside this batch.

For observation adoption, specify trusted source admission tied to the launched
harness, the selected parser/CLI capability matrix, raw transcript retention,
rate-card ownership and lineage accounting/budget integration. The experimental
store remains inactive until its production integration is qualified. Native
usage absence stays unknown. Normalizer fixtures do not qualify released CLIs.

WRK-09 and AQ-23 need concrete message/composer arbitration and persisted stall
transitions: current evidence, grace/backoff, safe nudge, stop/retention/replacement,
and escalation. Human control, cancellation and registered waits must fence each
action. No unsafe input replay or replacement without stopped-writer evidence.
Missing wait/continuation capabilities must be explicit; this batch cannot claim
full hibernation-aware operations acceptance before those owners exist.

Completion evidence covers actual first-harness CLI watch/peek/nudge, retained
transcript/usage with truthful gaps and costs, recovery races and a deterministic
stall ladder. Four-harness and browser qualification remain separately tracked.

## 3. Central supervisors and playbooks

Existing design and implementation sources:

- [aq-playbooks design](../../../aq-playbooks/docs/specs/0001-aq-playbooks.md).
- [aq-playbooks implementation decisions](../../../aq-playbooks/docs/specs/0002-implementation.md).
- [AQ host integration boundary](../../../aq-playbooks/docs/aq-host-integration.md).
- [Central host ownership](../architecture/0003-agent-profiles-harnesses-and-terminals.md).
- [Notification and policy extension](../architecture/0004-session-operations-waits-and-notifications.md).
- [Current messaging owner](../implementation/AQ-11.md) and
  [notification transport](../implementation/AQ-24.md).
- [Playbook persistence proposal](../../../aq-playbooks/docs/implementation/PB-04.proposal.md)
  and [latest measured experiment](../../../aq-playbooks/docs/implementation/PB-04.lookahead.md).
- [AQ-12 through AQ-17 implementation plan](../implementation/AQ-12-through-17-plan.md),
  [first durable bridge](../implementation/AQ-13-first-bridge.md) and
  [exact human/check ownership](../implementation/AQ-14-decision-check-owner-design.md),
  [standalone review protocol](../implementation/AQ-14-review-protocol.md) and
  [optional human/source owner](../implementation/AQ-14-human-owner-checkpoint.md).

Canonical work: AQ-12 through AQ-17, applicable escalation/digest portions of
AQ-27, WRK-09 and PB-04.

The engine and AQ host have concrete implementation plans. The explicitly mounted
first bridge now has durable keyed admission, message.send, cancellation and a
protected outbox. The candidate human/source owner has actual PG component
qualification for competing answers, current role/expiry fences and atomic
receipt/outbox writes; its optional admin routes are disabled by default and026
remains unregistered. Profile2 decision adapter, executed checks, completion
reconciliation and default startup remain open. Use current
messaging, graph, selection and notification services; dated foundation-only route
inventories must not cause duplicate owners to be implemented.

Follow the specified namespace ownership and blocking-I/O isolation, startup
registration, protection-key provisioning, event-outbox delivery, bounded driver
budgets, authorize/dispatch/reconcile/cancel mappings and revision-bound receipts.
Map the twelve existing commands to AQ-owned services; do not expand the command
surface to compensate for missing production adapters. Reuse the guarded host
for central supervisor sessions, with separate central authority and credentials.

Implement core work/context/operation/message adapters first. Then add bounded
assessments, genuine human decisions and executed checks, followed by candidate
preparation/publication and findings/schedule/report coordination. Wire external
notification acceptance/uncertainty through the existing transport owner.

Qualify coding/review/check/publication, routing, findings-watcher and supervisor-
report pilots with exact retained subjects and restart/lost-reply evidence.
The unchanged playbook persistence budget remains a separate acceptance condition;
previous query reductions did not meet it and must not be advertised as a pass.

## Sequencing and scope limits

The first execution milestone is the critical path. Terminal output and playbook
runtime integration/persistence can progress independently against published
contracts. Safe nudge, central supervisor execution, accounting and substantive
policy pilots converge on the qualified native harness and result path.

Before each remaining implementation slice, its owner should record the concrete
state/transaction changes, public contracts, recovery behavior and bounded tests
in a task-level implementation plan, with links back to these designs. Existing
component evidence remains valid for its stated scope; source changes require
appropriate new qualification.

Hibernation/cross-node wake, additional harnesses, the dashboard and product
release/deployment are outside these three areas. AQ-27's wait-source integration,
GATE-05 and INT-03 retain their existing wait/continuation prerequisites; this
batch cannot close those broader tasks by omitting the dependencies. Retain
separate evidence for completed notification/digest work. Live-provider and
external-destination qualification must record actual configured targets and
results; fake peers cannot substitute for those product claims.

## Reconciled designs and active implementation plans

On2026-10-31 the shared ecosystem/execution/profile/session designs and worker
framework/execution specifications were reconciled with current components.
Native acceptance now explicitly returns preparation authority before a separate
grant; schema2 composition precedes final selected-model access-profile derivation.
Historical checkpoint prose is identified rather than treated as current status.

Concrete current plans are now retained for the
[native producer](../implementation/AQ-08-native-completion-plan.md),
[stopped evidence/result settlement](../implementation/AQ-10-result-settlement-plan.md),
[terminal output](../../../aq-worker/docs/implementation/WRK-14-output-plan.md) and
[embedded policy/supervisors](../implementation/AQ-12-through-17-plan.md).
The playbook host integration boundary now names actual existing AQ owners.
These plans identify staged implementation and remaining acceptance requirements;
their publication alone does not complete the corresponding milestone.

Current implementation/evidence: [three-area progress checkpoint](2026-10-31-execution-sessions-policy-progress.md). This does not close the three-area product gates.
