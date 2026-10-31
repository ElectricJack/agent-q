# Agent Q v2 implementation audit

**Date:** 2026-10-30

**Scope:** Local implementation evidence for the ecosystem, execution/recovery, agent graph access and dashboard specifications. This is a planning audit, not release certification.

The graph, Git and playbook components already contain substantial implementations. The largest missing product slice is AQ's execution authority and domain adapters connected to an independently released worker runtime. Rebuilding those component engines would duplicate existing work. The first product milestone should use fake agents to prove durable assignment and recovery, then add real graph access, retained source and one real harness.

This audit preserves the pre-implementation baseline. Current implementation and qualification statuses are recorded in the [milestone graph](../plans/0001-v2-milestones.md); historical connector links below name the audited commit.

## 1. Evidence and interpretation

The audit inspected source boundaries, test definitions, local status documents, recorded validation artifacts and build/release definitions. No application test suite, container drill, model invocation or remote release/CI verification was run for this audit. Recorded passing counts below are attributed to their existing evidence; they do not certify the current working tree.

The [snapshot](2026-10-30-implementation-audit-snapshot.json) records repository HEADs, dirty-entry counts and SHA-256 hashes of 46 selected evidence anchors. This is not a complete source/build attestation. Several repositories contain uncommitted work and may be changing concurrently; implementation owners must select and qualify exact candidate revisions before integration.

A final drift check found aq-git's release-status report had advanced from 78 to 84 recorded tests, and aq-playbooks runner/storage files were changing. A targeted follow-up found new indexing, retention, inspection and optional protection/keyring code, including a second PostgreSQL migration. Their behavior was not validated by this audit, and the original acceptance report does not automatically cover them. The snapshot retains the initial hashes plus a follow-up observation. PB-01/PB-02 must reconcile this work before adding overlapping implementation; no active component work was modified.

Evidence labels mean:

- **Implemented locally:** relevant implementation was found; full acceptance is not implied.
- **Recorded validation:** an existing report records tests or measurements; this audit did not rerun them.
- **Partial:** some mechanisms exist, but the specified end-to-end behavior is absent or limited.
- **Design only:** specifications exist without the corresponding implementation in the inspected scope.
- **Unverified externally:** remote publication, CI or deployment was not checked.

## 2. Repository baseline

| Repository | Audited baseline | Assessment |
| --- | --- | --- |
| agent-q | HEAD `eed842362f29016ee508ca200f3bb3da2473dc08`, with local documentation changes | Foundation server, connector, updater and release tooling exist. Execution and agent graph adapters are design only. |
| aq-worker | Initialized `main`, no commit at snapshot | Standalone specs and protocol ownership note; no Cargo workspace/runtime here yet. Foundation connector still lives in agent-q. |
| aq-graph | HEAD `225329cd6acf5a775fd2a0b016dd1c45fb16c79b`, with integration-doc changes | Substantial standalone service and CLI; production AQ integration absent. |
| aq-git | Initialized `main`, no commit at snapshot; implementation files untracked | Substantial local implementation. An immutable integration candidate and release acceptance are still needed. |
| aq-playbooks | HEAD `30385b7b355f277ea38ac603d0820722739b578d`; implementation is local work beyond that design commit | Five-crate implementation, fake-host pilots and recorded validation. HEAD alone does not identify that implementation. |
| aq-dashboard | Local design directory, no Git repository | Design scaffold only; frontend framework, source and release pipeline are not established. |

This table describes the captured local checkout, not the current state of GitHub. The task plan includes baseline qualification rather than treating uncommitted implementations as released dependencies.

## 3. Capability audit and follow-up ownership

Task IDs refer to the [milestone task graph](../plans/0001-v2-milestones.md). Detailed task contracts and hard dependencies are in its [JSON source](../plans/0001-v2-milestones.json).

| Area | Evidence found | Gap and required work | Tasks |
| --- | --- | --- | --- |
| Foundation enrollment/authentication | [Worker routes](../../crates/aq-server/src/worker_api.rs) implement enroll/heartbeat/probe; [lifecycle test](https://github.com/ElectricJack/agent-q/blob/eed842362f29016ee508ca200f3bb3da2473dc08/crates/aq-worker/tests/lifecycle.rs) covers enroll, heartbeat, server restart and revocation | Qualify the current baseline and preserve its wire/configuration behavior through extraction | BASE-01, WRK-01, AQ-01 |
| Central persistent state | [Foundation migration](../../crates/aq-server/migrations/0001_foundation.sql) stores users, enrollments and nodes | No attempt/grant/offer/result/outbox state in the inspected foundation; add explicit durable authority and replay | AQ-02, AQ-03, AQ-05 |
| Worker protocol | [Current wire types](https://github.com/ElectricJack/agent-q/blob/eed842362f29016ee508ca200f3bb3da2473dc08/crates/aq-protocol/src/lib.rs) cover connectivity; [standalone protocol directory](../../../aq-worker/protocol/README.md) is a design note | Publish execution schemas, behavioral fixtures and compatibility negotiation; keep foundation v1 semantics | CON-01 |
| Worker execution | [Connector](https://github.com/ElectricJack/agent-q/blob/eed842362f29016ee508ca200f3bb3da2473dc08/crates/aq-worker/src/client.rs) provides heartbeat, backoff and failure classification | Standalone journal, reservations, launch ownership, expiry, stop proof and result recovery are missing | WRK-02, WRK-03, WRK-04 |
| Scheduling | aq-graph exposes ready candidates; worker design defines eligibility and local admission | No central execution claim/placement loop found. First use explicitly test-only fixtures, then actual graph candidates and project eligibility | AQ-04, AQ-08 |
| Graph service | [HTTP service](../../../aq-graph/src/service.rs), [API schema](../../../aq-graph/docs/openapi.json), [storage invariant tests](../../../aq-graph/tests/storage_invariants.rs) | Reuse the existing API. Qualify the exact graph candidate and add AQ's scoped client boundary; do not add a worker replica | GRAPH-01, API-01, AQ-06 |
| Graph mutation delivery | [Python adapter example](../../../aq-graph/examples/agent_q_adapter.py) preserves version/idempotency concepts; graph stores receipts | Example is not production AQ wiring. Add durable delivery intent, authorization, receipt reconciliation and confirmed domain events | AQ-07, AQ-10 |
| Agent credentials and context | Worker/framework and daemon specs define the intended policy | No per-attempt credential provisioning or agent graph CLI path found in foundation | AQ-06, WRK-05 |
| Git/workspace mechanics | [Release status](../../../aq-git/docs/release-status.md) maps implemented protocol, journal, queries, workspaces, handoff, candidates and publication | Qualify/package existing implementation, preserve its supported-repository limits, then add worker and daemon adapters | GIT-01, WRK-06, AQ-09 |
| Central source handoff | [Standalone scenario](../../../aq-git/crates/aq-git-cli/tests/scenario.rs) imports source, removes worker storage, builds/publishes centrally | Uses local process fixtures, not authenticated AQ transport. Add upload/download authorization, resumable transfer and distinct retention/result receipts | AQ-09, WRK-07 |
| Real agents and messaging | Harness/messaging design exists; foundation routes contain no production agent mailbox | Select one supported harness, implement confinement and process proof, provision client access and add durable scoped messaging | WRK-08, AQ-11, WRK-09 |
| Result versus task completion | Graph supplies version checks; worker/playbook specs distinguish outcome and completion | Add evidence applicability, cancellation/result ordering and confirmed graph lifecycle delivery | AQ-10 |
| Playbook engine | [Runner Host boundary](../../../aq-playbooks/crates/aq-playbooks-runner/src/lib.rs), compiler/model, storage and [fake-host pilots](../../../aq-playbooks/crates/aq-playbooks-cli/tests/host_pilots.rs) | Reuse engine; qualify a library candidate and embed behind a bounded host integration. Synthetic twelve-command catalog is not production AQ adapters | PB-01, AQ-12, AQ-13, AQ-14, AQ-15, AQ-16 |
| Playbook runtime capacity | [Storage adapter](../../../aq-playbooks/crates/aq-playbooks-postgres/src/lib.rs) uses a synchronous client and whole-namespace JSONB snapshots | Keep blocking operations off async control handlers; measure growing state, fairness and retention before capacity acceptance | AQ-12, PB-02 |
| Dashboard | [Design](../../../aq-dashboard/docs/specs/0001-dashboard.md) only | Publish inspection/action API, create frontend repo/application, test mocks and package static bundle | API-02, UI-01, UI-02, REL-03 |
| Product release/update | [Manifest](../../crates/aq-release/src/manifest.rs), [updater](../../crates/aq-updater/src/engine.rs), [release workflow](../../.github/workflows/release.yml) and [lifecycle drill](../../scripts/update-lifecycle) exist | Format 1 pins server/worker; extend deliberately for graph and embedded components, migration ordering and exact-artifact acceptance | INT-01, REL-01, REL-02 |
| Product scale and hosted recovery | Standalone graph measurements and foundation drill scripts exist | No evidence for the complete 500-agent path or target-host recovery in this audit | PERF-01, OPS-01, GATE-06 |

## 4. Existing validation that should be reused

### Foundation

The [CI definition](../../.github/workflows/ci.yml) includes formatting/Clippy, workspace tests with PostgreSQL, updater tests, static updater build, container smoke and update lifecycle jobs. Tests and scripts are executable assets; their presence does not prove a current CI run passed. BASE-01 reruns the relevant baseline on the selected revision and records exact commands, environments and artifacts. Hosted ingress/access and off-host recovery remain separate from a local smoke pass.

### aq-graph

[Schema-8 validation evidence](../../../aq-graph/docs/artifacts/validation/2026-10-30-schema8-checks.json) records 125 Rust tests including 101 PostgreSQL tests, nine Python tests, and deployment recovery. [Schema-8 load evidence](../../../aq-graph/docs/performance/schema8-load-first/README.md) records 100,000 nodes and 500,000 relationships, all offered 200/s sustained and 1,000/s ten-second burst requests completed, and expected validation/conflict responses. Continuous 200-client saturation exceeded latency budgets. These are local component measurements, not daemon/WAN/product sizing.

Older schema performance failures remain historical evidence. GRAPH-01 must name the exact runtime and current evidence rather than applying a historical result to a new candidate. No new graph engine, offline replica or v1 importer is part of this plan.

### aq-git

[Release status](../../../aq-git/docs/release-status.md) initially recorded 78 local correctness tests; the follow-up report records 84 after additional I/O/admission changes, plus formatting, Clippy and build checks. Neither suite was rerun in this audit. The report explicitly leaves full performance targets, full compatibility/crash certification, host-power-loss rehearsal, hosted credential/transport verification and release packaging as gates. The current profile has repository restrictions documented in its [README](../../../aq-git/README.md), including submodules/LFS/attributes and shallow/partial histories; initial integration fixtures and pilot projects must fit the qualified profile or be explicitly declined.

GIT-01 creates an immutable, verified integration candidate and an honest limitations record. It does not require every future platform feature before a disposable local pilot, and it does not equate process-crash tests with host-power-loss durability. OPS-01 gates any stronger deployment claim on corresponding evidence.

### aq-playbooks

[Acceptance evidence](../../../aq-playbooks/docs/acceptance.md) records 76 database-free workspace tests and eight explicitly enabled PostgreSQL tests. Coding, routing, watcher and report pilots use synthetic host implementations. They do not execute real workers, human decisions, hosted source publication or production calendars.

The initially recorded [implementation limits](../../../aq-playbooks/docs/specs/0002-implementation.md) are material to integration: exact artifact dependency pins, serial foreach with parallelism through async children/joins, restricted typing for dynamic join-result keys, protected-input/output refusal without a provider, and conservative snapshot/receipt retention. Follow-up source introduces optional protection and indexing/retention mechanisms, whose acceptance must be established on a selected candidate rather than inferred from new type names. Whole-namespace writes and a synchronous client remain visible in the inspected storage path. The 100-admission PostgreSQL measurement is not a large-history throughput guarantee. PB-02 must measure the qualified implementation and add narrowly scoped missing work only where its operating envelope fails the product requirement.

## 5. Design gaps to settle in implementation tasks

| Decision | Owner/task | Must be settled before |
| --- | --- | --- |
| Exact execution envelopes, error categories, acknowledgement boundaries and capability negotiation | aq-worker / CON-01 | Implementing both sides of execution transport |
| Foundation wire-type ownership and compatibility-preserving source extraction | aq-worker / WRK-01, agent-q / AQ-01 | Removing the in-tree connector |
| Durable AQ authority model and cancellation/admission ordering | agent-q / AQ-02, AQ-05 | Live worker execution |
| Graph scope model for search/context/edges, service-principal provenance and receipt lifetime | agent-q / API-01, AQ-06, AQ-07 | Agent graph mutations |
| First harness, isolation capability, process ownership and provider-secret handling | aq-worker / WRK-08 | Real provider sessions |
| Synchronous playbook storage/host execution isolation and namespace admission budgets | agent-q / AQ-12; aq-playbooks / PB-02 | Policy-driven load and production capacity claims |
| Composite manifest version, migration sequence and permitted rollback states | agent-q / REL-01, REL-02 | Multi-component product upgrades |
| Frontend framework and bundle/API compatibility | aq-dashboard / UI-01 | Dashboard implementation |
| Target machine/network limits, protected-data policy and accepted component profile | agent-q / PERF-01, OPS-01 | Declaring the product deployment-ready |

The plan intentionally does not require generic offline merging, active/active daemon failover, worker-side graph databases, live agent migration, arbitrary parallel foreach, a private registry, Kubernetes or full v1 feature parity. Confidential playbook values remain refused unless the selected candidate's protection facility and host configuration are explicitly qualified; deployment qualification must validate that selected workflows respect the actual supported protection boundary.

## 6. Recommended starting work

After this audit, BASE-01, CON-01, GRAPH-01, GIT-01 and PB-01 have no outstanding task dependencies. These establish the foundation baseline, shared execution contract and three reusable integration candidates. Daemon authority and worker runtime implementation then proceed independently against the pinned contract. The first demonstrable gate is distributed execution with a deterministic fake agent, followed immediately by crash/replay/cancellation proof.

Do not execute tasks against moving sibling branches. Before starting, compare the snapshot with current code, identify overlapping work and update the selected candidate revision. A task completes only with its specified evidence; this planning artifact has not submitted tasks to a daemon, created issues, committed implementation, or deployed services.
