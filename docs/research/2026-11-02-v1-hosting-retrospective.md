# Retrospective: building AQ v2 inside AQ v1

**Date:** 2026-11-02, first afternoon and evening of the build. This is a living record; append later findings below the same headings.

**Context:** the v2 program plan (91 tasks across six repositories) was imported into AQ v1 and run with Codex and Claude agents.

**Outcome at the time of writing:**
- 18 of 91 plan tasks (about 20%) and 47 agent-filed follow-ups completed.
- About 30 PRs were merged.
- Most of the first hours went into making v1 handle a multi-repository project.

Each finding gives what happened, how it was handled, and what v2 should do differently. It names the v2 plan task that owns the improvement, or marks it **new** where none exists yet.

## What went right

| # | Finding | Evidence | Keep in v2 |
| --- | --- | --- | --- |
| R1 | **The queue keeps agents moving.** One task per fresh session, readiness derived from the dependency graph, and enforced close rules (named-test deliverables, checklist) fixed the original "agent stops after one feature" failure. | 18 plan tasks plus 47 follow-ups closed without anyone prompting the next step. | The same model: D3/D9 (automatic settlement, default coding policy), B4 (retries). |
| R2 | **Self-contained task descriptions worked.** Goal, what to build, current state, interfaces and out-of-scope sections kept agents in scope. Agents surfaced real gaps instead of improvising. | B6a1 found a renewal contract gap and stopped. D0 was identified as missing. sharp-harbor-18 refused to fabricate qualification facts. | **New:** v2 task authoring (F2, the supervisor's enqueue commands) should produce this structure by default. Templates should carry "out of scope, owned by task X". |
| R3 | **Independent reviews before running were cheap and high value.** The cross-family Fable review, the builder check and the parallelization review caught sizing problems, wrong dependency edges, a missing GitHub credential path and a release gate that cut MS1's critical path from about 13 to 9 sessions. | Plan revision log in `docs/superpowers/plans/2026-11-02-v2-full-application-plan.md`. | D9 (cross-family review in the coding policy); keep adversarial review on contract and security tasks. |
| R4 | **Agents fixed baseline problems instead of working around them.** Follow-ups repaired formatting, Clippy, flaky fixtures and stale hashes. | About 30 follow-up PRs. | Keep "file emergent work, don't widen scope" (R2). |
| R5 | **Recovery holds carried good diagnoses.** The supervisor's hold text explained the cause and the safe next step, which made operator release decisions quick. | Holds on A4, C5a1, E2, A3a, B6a1. | H3 (stall ladder) and the recovery explanations in API-02 and G10. |
| R6 | **Cross-project dependencies and PR-merge detection work.** v1 kept edges across projects, and its sweep noticed when a superseding PR carried a PR's commits to main. | B2 waiting on B1 across projects; the gate for aq-worker #4 resolved through #12. | B4 and D3 must keep dependency and delivery truth across repositories. |
| R7 | **The project split was cheap.** Moving tasks between projects kept their IDs, descriptions and edges. | 47 tasks moved in minutes. | **New:** v2 task moves between projects and repositories should be first-class, with an audit record. |
| R8 | **The cross-repository follow-up comment convention works.** Agents record a complete, self-contained follow-up for another repository, and the operator files it in the right project. | G7 to aq-server (`keen-current-13`); I0 to aq-server, routed to existing tasks. | Prefer it until v2 has the emergent-work API (W4). |

## What went wrong, and what we changed

### Multi-repository work

| # | Problem | Fix applied | v2 improvement |
| --- | --- | --- | --- |
| W1 | **v1's pools only take `project-repo`/`vault` workspaces.** Tasks needing another repository's kind got `no_candidates / workspace_requirement`. | First added task-lifecycle Codex profiles, then split into one v1 project per repository. | Multi-repository projects are first-class. A project binding lists repositories, and the scheduler places a task by its declared repository (spec 0003, D2). Workers must not assume a single primary repository. |
| W2 | **Worktree creation authorizes only the project's repository URL.** It ignored the workspace kind's `repo_url`, failing with "GitHub repository reference did not match the authorized repository". | Per-repository projects. A v1 patch exists (scratchpad, not applied). | Authorization per repository binding; aq-git binding generations (D1, D2). |
| W3 | **Delivery bookkeeping falls back to the project's repository** when a task has no `repo_id`. | Per-repository projects. | Every task names its repository explicitly. Delivery receipts are keyed by repository (D1, D3). |
| W4 | **Worker-filed follow-ups land in the filing task's project**, so cross-repository follow-ups ran in the wrong repository. Workers cannot file into another project. | Footer rule: record `CROSS-REPO FOLLOW-UP (<repo>):` comments; the operator files them. | **New:** an emergent-work API that takes a target repository and project, with authorization and supervisor triage. |
| W5 | **A directory can be a workspace in only one project** (compared by resolved path), which blocked shared read-only references. | A per-project directory of symlinks. | Read-only reference mounts can be shared; sandbox mount aliases (0005 §5.1). |

### Routing, scheduling and capacity

| # | Problem | Fix applied | v2 improvement |
| --- | --- | --- | --- |
| W6 | **Routing failures were silent.** Routing playbook runs failed with `error: None`. Tasks sat READY as "awaiting route", and diagnosis needed a manual `aq system task-route-plan`. | Ran the plan command by hand. | Routing outcomes carry a typed reason on the task, show up in explanations, and raise an attention item when a task cannot be placed (W13, G4, API-02). |
| W7 | **Task-lifecycle profile capacity is the number of worker-agent rows.** A new profile with no agents reports "no slots". | Created 8 agent rows. | Capacity is an explicit, visible setting on the node or profile (0005 §3.2, C4). |
| W8 | **Concurrency caps were the real throughput limit,** for example aq-worker at 4 while holding the largest share of work. | Raised to 6. | Show utilisation against each cap in the Overview (G4); suggest a cap change when the ready backlog is deep. |
| W9 | **Postgres became CPU-bound on the admission query** (`stack_authorization`, about 1–2 s per run) at about 100 tasks across six projects. Scheduler cycles overran and the pool scaler stalled. `ANALYZE` did not help. | None; accepted. | Measure and bound the scheduling path as the graph grows (J3, PB-04/D10 for playbooks). Prefer incremental readiness over whole-graph recursion. |
| W10 | **Interim release gates and phases cut parallelism.** The MS0 release gate held back 14 starting tasks. | Removed it after the parallelization review. | Milestones are acceptance checkpoints, not scheduling barriers. Keep gates only for genuine prerequisites. |

### Integration and review

| # | Problem | Fix applied | v2 improvement |
| --- | --- | --- | --- |
| W11 | **Follow-ups inherited the global `pull_request` default.** 26 PRs waited for the owner, and v1 cannot set `direct` as a project default. | Merged 30 PRs and told agents to file follow-ups as `direct`. | Project integration policy includes `direct`. The PR backlog is visible, with an optional auto-merge rule when checks pass (D1, D7, G4). |
| W12 | **The aq-server CI baseline is red on `main`,** so PR checks carried no signal. | Merged on mergeability. Agents repairing the baseline. | Track baseline health, and judge a change by new failures, not absolute status (**new**, probably with D6 `check.request`). |
| W13 | **Direct-to-main concurrency caused merge conflicts in hot files** (Dockerfile, formatting sweeps). | Reopened tasks with exact rebase instructions. | Detect overlapping file sets before scheduling, or require a rebase-and-recheck step before close (D1, D3). |
| W14 | **Reviews defaulted to the owner, and delegation applied only to new reviews.** No project supervisor was running, so delegated reviews stalled until the global supervisor was asked directly. | Delegated four reviews explicitly and messaged the global supervisor. | Reviewer assignment with a service level, automatic dispatch and a visible review queue (D9, G4, F1). |
| W15 | **The review flow creates its own revision tasks.** Reopening the original task as well risks two agents on one branch. | Closed the duplicate; made originals wait on their revision tasks. | One owner per branch. A revision task supersedes the original's claim (D9). |

### Owner decisions and recovery

| # | Problem | Fix applied | v2 improvement |
| --- | --- | --- | --- |
| W16 | **Owner-decision gates looked like failures.** They appeared as recovery holds labelled `session_close_hard_failure`. Questions went to the dashboard inbox, sometimes in forms the monitor didn't read. | Read hold text and asked the owner in batches. | A typed "needs owner decision" state with the question, options and the work it blocks, in a single decision inbox with notifications (F4, F5, G4). |
| W17 | **Releasing a hold needs the exact incident ID and a float `decided_at`.** These were only recoverable by parsing explanation text. Retry budgets were exhausted by infrastructure churn. | Parsed `aq task explain`; raised `max_retries`. | Recovery operations take simple handles. Explanations return structured fields. Infrastructure failures do not consume the task's retry budget (B4, X-requirements, API-02). |
| W18 | **Owner facts were blocked behind "attestation".** Agents correctly refused to invent qualification inputs. | The owner authorized agents to record observed facts as owner-attested. | A first-class owner-attestation record, labelled distinctly from measured evidence (C4, I4a). |
| W19 | **Credential custody wasn't designed for a shared desktop login.** v1 agents and v2 containers share `~/.codex/auth.json`, so an unsaved refresh could sign v1 out. | Owner decision: the desktop worker is the only writer, with write-back through C5b. | Already in 0005 §7; fresh-delta-13, C5b and C6b implement it. |

### Operator tooling and configuration

| # | Problem | Fix applied | v2 improvement |
| --- | --- | --- | --- |
| W20 | **The live checkout is fragile.** The operator (Claude) patched v1's live checkout and briefly switched it from its deployed `dev` branch. v1's own rules forbid this. | Reverted within about 2 minutes; the patch is kept outside v1. | Deploy from immutable images (E1, E4); refuse or warn on edits to a deployed tree (**new**). |
| W21 | **Configuration needs restarts.** Workspace-kind files only load at daemon start. | Restarted with `aq restart --no-dashboard`. | Validated hot reload of configuration (A4, E1). |
| W22 | **CLI gaps forced database edits.** There's no way to edit deliverables or subtask text. `project create --from-policy` prompts on a non-terminal and silently skips the policy. `message send` without `--project` fails silently. `task get` omits `verification_type`. Onboarding swallowed an exception as "failed unexpectedly" with no log (a missing parent directory). The project ID is derived from the display name. | SQL updates; reruns. | API and CLI completeness for every field tasks carry. Non-interactive modes. Every failure logged with its cause (F7 `aqv2`, G1/G2). |
| W23 | **Hard-coded branch assumptions.** Agent instructions kept asking for `origin/dev`, which these repositories don't have. | Feedback text on each task. | Branch names come from the project binding, never from prompt defaults (D2, profile templates). |
| W24 | **v1 graph import can't set per-node workspace requirements.** | The importer creates tasks one at a time, then adds edges. | Bulk plan import with full per-task fields, as one atomic, human-gated batch (F2). |

### Planning process

| # | Problem | Fix applied | v2 improvement |
| --- | --- | --- | --- |
| W25 | **The first plan assumed API keys.** The owner uses CLI subscription logins. | Requalified I0, I4a and I4b on the ChatGPT login. | Confirm the credential model with the owner before writing tasks that depend on it. |
| W26 | **Agent instructions named a command that doesn't exist** (`aq question` for asking). | The footer now uses `aq message send`. | Validate every command named in task text against the CLI before import (**new**, an importer check). |
| W27 | **The derived plan index went stale and deliverables were generic.** | The index is generated from the task file; deliverables name tests. | Generate every derived view; reject generic test deliverables (already enforced by the importer). |
| W28 | **Monitoring needed a lot of manual operator work:** routing, holds, PRs, reviews and decisions all needed humans on the first day. | A 15-minute monitor loop. | The Overview "needs attention" list (G4/G5) should list exactly these: unroutable tasks, holds awaiting an operator, PR backlog, stalled reviews, owner decisions, and capacity at its cap. |
| W29 | **Agents block on decisions that existing policy already answers.** Examples: the base branch (main), whether to publish, and owner-attestable facts. Every one costs a hold, an owner round trip and a retry. | Owner answers recorded in task feedback; the footer and descriptions state main and direct integration. | Project policy facts (default branch, integration mode, release authority, attestation rules) are queryable by agents, and the supervisor answers from policy before escalating (F1, F5, **new**). |
| W30 | **The operator and the supervisor both acted on the same cross-repository follow-up** and filed duplicate tasks (`nimble-dune-30` and B1b), because neither knew the other had picked it up. | Closed the duplicate; rewired its dependencies. | A follow-up gets one owner: claiming it records who triaged it and the resulting task, visibly on the source comment (**new**, part of the W4 emergent-work API). |

## Proposed new v2 work

These items had no owner in the plan (marked **new** above). Each is a candidate task:

1. An emergent-work API with a target repository and project (W4).
2. Baseline-aware check evaluation (W12).
3. A guard against editing a deployed tree (W20).
4. An importer check that the commands named in task text exist (W26).
5. Task moves between projects and repositories with an audit record (R7).
6. A structured task template for supervisor-authored work (R2).

They have not been filed yet.
