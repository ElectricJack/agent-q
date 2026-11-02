# Agent Queue v1 to Agent Q v2 gap analysis

**Date:** 2026-10-30

**Scope:** Feature comparison between the existing Agent Queue installation (v1, checked out at `../agent-queue2`) and Agent Q v2 (`agent-q` with `aq-git`, `aq-graph`, `aq-playbooks`, `aq-worker` and the `aq-dashboard` design). It identifies v1 capabilities that v2 has not implemented, separating gaps already covered by the [milestone task graph](../plans/0001-v2-milestones.md) from gaps with no planned owner.

This complements the [implementation audit](2026-10-30-v2-implementation-audit.md), which compares the v2 specifications to v2 code and explicitly excludes full v1 feature parity. Full parity remains a non-goal; this document lists the v1 capabilities that need an explicit keep, defer or drop decision.

## 1. Method and limits

The v1 inventory was built from v1 documentation, the generated CLI/OpenAPI/MCP inventories, source modules, configuration defaults and test files. Maturity labels are judgements from code size, tests, documentation and local configuration. The v2 side was checked against specifications, the milestone JSON, server route registrations, migrations and repository status. No test suite was run for this analysis.

"Not planned" means no v2 specification or milestone task covers the capability. It does not mean v2 must implement it.

## 2. Summary

v2 is a durable execution kernel with substantial components, but it cannot yet run a real agent. Completed milestone tasks cover M0 to M3 (contract, connector, fake-agent execution, recovery and cancellation) and qualification of the graph, Git and playbook candidates. None of the v1 surfaces that users and agents rely on daily is implemented in agent-q yet: real harnesses, routing, integration, messaging, reviews, a product CLI or MCP surface, and the dashboard.

Gaps fall into three groups:

- **A. Planned:** covered by remaining M4 to M6 tasks.
- **B. Not planned:** no owner in the specifications or plan; requires a keep/defer/drop decision.
- **C. Safe to drop:** vestigial or project-specific in v1.

## 3. Scale

| | v1 | v2 (2026-10-30) |
| --- | --- | --- |
| Surface | 648 CLI leaves, ~511 REST operations, ~510 MCP tools | agent-q exposes ~40 worker, admin, agent-access and source routes; aq-graph has its own CLI and HTTP API |
| Code | ~530k lines of Python, ~83k lines of dashboard TypeScript | agent-q server ~4.6k lines of Rust; component libraries ~8k to 25k lines each |
| Harnesses | Claude and Codex (core), Gemini (limited), OpenCode (operator-defined) | Deterministic fixture agent only |

## 4. Capability comparison

| Area | v1 capability | v2 state | Plan coverage |
| --- | --- | --- | --- |
| Task graph | 12 statuses; `blocks`, `parent-child`, `waits-for`, `conditional-blocks` and `delivery-of` dependencies; hierarchy; subtasks; deliverables; formulas; proposals; held batches; explain; archives | aq-graph provides `blocks`, parent-child, readiness, revisions, drafts/changesets and history. Waits, contingency and delivery edges require explicit adapter contracts | A: AQ-08, AQ-13 (`work.ensure`). B: subtask checklists, deliverable checks, formulas/templates, held ingestion batches, archives, human-readable IDs, attachments, comments |
| Scheduling | Fair-share push scheduler, pull-based worker pools, gates, merge slot | Central placement and local admission designed; test-only scheduler implemented | A: AQ-08 |
| Routing and models | Deterministic planner, intelligence classes, agent profiles, provider availability state machine, failover/reroute, quota probes | Routing exists as a playbook pilot only; intelligence classes and profiles have no v2 design | B: profile, harness and intelligence-class catalog; provider availability, failover and quota probes |
| Agent execution | Multiple harnesses in tmux; prime document; stall ladder (nudge, restart, quarantine); native question detection; transcript readers | Process ownership, guardian, journal, grants and recovery implemented; no real harness | A: WRK-08 (one harness). B: additional harnesses, prime/context composition, stall nudging, transcript and usage parsing, attach/peek/nudge |
| Git and integration | Integration train, CI evidence and attestation, PR flow, GitHub App credential broker, promotion, CI and conflict fixers, prerequisite branch stacking, hierarchy/epic delivery | aq-git provides workspaces, frozen candidates, fast-forward-only conditional publication and retention; rejects submodules, LFS, attributes and shallow/partial history | A: AQ-09, AQ-15 (disposable-remote pilot). B: GitHub PRs, CI check ingestion, GitHub credentials, repair fixers, hierarchy/epic delivery, promotion |
| Projects | Onboarding (link, init, clone, create GitHub repository), project roots, per-project policy and caps | No project records in agent-q migrations; worker spec defines project eligibility only | B (blocks real use) |
| Playbooks | V2 engine in production; LLM-compiled markdown sources; shipped routing, failover, usage-probe, pipeline, failure-triage and report playbooks plus reviewed bundles | aq-playbooks engine with twelve typed host commands and four fake-host pilots | A: AQ-12 to AQ-16, GATE-05 (routing, watcher, report, coding). B: porting failure triage, provider failover, blocked-task escalation, digest and promotion policies; in-product authoring/compile flow |
| Waits | Typed waits (timer, task, CI, PR, message, human, job), hibernation, wake handoffs | Runner waits and timers exist; no host-side typed waits | Partly A. B: session hibernation and wake |
| Knowledge | Versioned records, verification, sharing, redaction/erasure, prompt context delivery, extraction, memory plugin, notes, vault | aq-graph knowledge records, verification, search, history and restore | A: AQ-06, WRK-05 (scoped reads). B: context injection budgets, sharing/redaction, extraction, memory, notes; no equivalent of the vault as configuration store |
| Messaging | Durable messages, agent questions (`WAITING_INPUT`), human asks, escalations, digest, reports, Discord, Gmail inbox, collaboration threads | Design only | A: AQ-11, WRK-09, AQ-16 (report pilot). B: external notification transport, escalation lifecycle, agent-question flow, collaboration threads |
| Reviews | Document reviews (submit, comment, decide, revision tasks), fidelity review, PR approval | `decision.request` and review work templates designed | B: document-review product |
| Supervisor | Per-project and global supervisors, `aq chat`, Discord conversations, failure triage | Central supervisor sessions designed | A: AQ-17. B: chat/conversation surface |
| Human and agent interfaces | `aq` CLI, REST, embedded MCP server injected into sessions, generated Python/TypeScript clients, plugins, shipped skills | Admin API and aq-graph CLI; no MCP design | B: product CLI (API-02 implies one but no task builds it), MCP, clients, plugin model (capability packs are only a concept), skills |
| Dashboard | ~25 pages: task graph, agents and terminals, inbox, waiting, metrics, reviews, knowledge, playbooks, settings | Specification only; no repository or framework | A: UI-01, UI-02 (task/attempt inspection, worker status, one action). B: remaining surfaces |
| Operations | Install wizard (macOS, WSL2), update/rollback, doctor (~76 checks), config editor and tuning, metrics, token usage and costs, durable job presets, test semaphore, smart test selection | Signed release manifest, updater, VPS/Compose/Cloudflare target | A: REL-01 to REL-03, OPS-01, PERF-01. B: doctor, token/cost accounting, local installation, job presets, resource gating, test selection |

## 5. Unplanned gaps to decide first

Ranked by how much each blocks day-to-day use:

1. **Project model and onboarding.** v2 has no registry binding projects to repositories, policy and eligibility; scheduling and integration assume one. Likely precedes AQ-08.
2. **Product CLI and MCP for agents.** v1 agents operate almost entirely through MCP and `aq` commands. The v2 agent API covers context and graph reads only, and no task produces an operator CLI.
3. **Profile, harness and intelligence-class catalog with provider availability.** WRK-08 commits to one harness; v1 depends on Claude and Codex failover, quota probes and class-based routing.
4. **GitHub delivery.** The plan ends at fast-forward publication to a disposable remote, with no PRs, CI checks, GitHub App credentials or repair sessions. This was v1's largest subsystem.
5. **Session observability and interaction.** Attach, peek, nudge, live terminals, stall handling, transcript parsing and token/cost accounting are absent.
6. **Notification transport.** `message.send` and the findings outbox are designed, but nothing delivers to a human outside the dashboard. Escalations and digests depend on it.
7. **Document reviews and agent questions/asks.**
8. **Doctor-style diagnostics.** API-02 explanation views cover part of this.

## 6. Where v2 already exceeds v1

- Outbound-only remote workers with fenced execution authority and guardian process ownership; v1 is single-host tmux.
- aq-git exact-revision caching, conditional publication and retained source handoff.
- Versioned, revision-checked task and knowledge graph with history.
- Typed playbook contracts with explicit recovery semantics.
- Signed composite release and updater.

## 7. Safe to drop

- Voxel level editor, Telegram, SQLite backend, facts/override indexing, glossary and agent-name modules.
- Legacy triage-task routing and coordination workflow-stage records.
- Most of the ~115 `integration` and `system integration-*` cutover commands, which are operator plumbing and largely untested at the CLI level.
- Object-loop and agent-queue-specific playbooks, unless those projects move to v2.

## 8. Baseline risks

- aq-git has no commits; its implementation is untracked.
- aq-playbooks' implementation is uncommitted beyond the design commit.
- agent-q and aq-worker contain uncommitted source-retention work, apparently AQ-09 and WRK-06.

Qualified-candidate evidence currently rests on working trees outside Git history. Commit or tag those candidates before further integration work.

## 9. Recommended next step

Turn the group B list into explicit keep, defer or drop decisions. Add each kept item to [`0001-v2-milestones.json`](../plans/0001-v2-milestones.json) with an owner and hard prerequisites, most hanging off GATE-04 or GATE-05, then regenerate the Markdown view with `python3 scripts/check-v2-plan.py --render`.
