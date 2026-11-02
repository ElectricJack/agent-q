# A1 plan provenance implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Freeze the old planning baseline and publish an unambiguous source index and complete successor mapping.

**Architecture:** Keep historical task data and component tables unchanged. Render the freeze notice from JSON metadata, resolve historical repository names in the checker, and maintain the source registry and old-to-new mapping in one documentation index.

**Tech Stack:** Python 3, unittest/pytest through `aq test`, existing PyYAML, Markdown and JSON.

**Spec:** A1 in `2026-11-02-v2-full-application-tasks.yaml`; program plan Global Constraints and section 3.

## Global Constraints

- Each repository builds and tests alone; no sibling path dependencies or submodules.
- Do not change any task scope or edges in the 2026-11-02 YAML.
- Do not rewrite component requirement tables or historical task/status/edge data.
- Evidence goes in `docs/implementation/A1.md`, with commands, environment, revision and limits.
- Publish only the assigned task branch through guarded `aq git push`; integration owns delivery.
- Follow the worker profile's focused-test scope; no Rust source changes require a full Rust suite.

## Review Focus

- Historical `agent-q` owners resolve to `aq-server` without modifying old tasks.
- Isolated worktrees resolve sibling documents against the common checkout while local evidence stays in the task checkout.
- Every open old task appears exactly once, including no-successor API-04 and UI-07.
- Deferred successors stay visible and distinct from active successors.
- Prefixes have one owning source family, including GIT-P; section-based sources have stable section citation rules.

### Task 1: Freeze and reconcile planning provenance

**Files:**
- Modify: `scripts/check-v2-plan.py`, `docs/plans/0001-v2-milestones.json`, generated `.md`, `docs/specs/result-media-png.md`, `docs/architecture/0001-ecosystem-and-releases.md`.
- Create: `scripts/test-v2-plan.py`, `docs/plans/requirement-sources.md`, `docs/implementation/A1.md`.

**Interfaces:**
- Consumes: old JSON `tasks`, YAML `tasks[].continues`, existing requirement tables and spec sections.
- Produces: JSON `tracking_authority` and `freeze_note`; generated frozen notice; unique requirement prefix registry; 66-row successor mapping.

- [ ] Write `test_frozen_header_and_generated_document`, `test_historical_owners_and_worktree_paths`, `test_complete_successor_mapping`, and `test_unique_requirement_sources` in `scripts/test-v2-plan.py`. Assert exact authority, unchanged task payloads, 66 open tasks, all continues successors (including deferred), explicit API-04/UI-07 gaps, and required distinct registry entries.
- [ ] Run `aq test scripts/test-v2-plan.py -q`; expect failures for absent freeze metadata/index and owner/path resolution.
- [ ] Add Git-common-checkout-aware path resolution and the `agent-q` owner alias to the checker; add JSON freeze metadata and render the historical notice.
- [ ] Write the source registry with GIT-R/V/P, PB-P/T and WRK-T aliases, plus unique prefixes and section citation rules for sources without IDs. Generate the mapping from every YAML continues list. File external alias/version and topology corrections under the immediate program parent with owning workspace kinds.
- [ ] Correct only the result-media status header and architecture 0001 section 6 target/status statement.
- [ ] Run `python3 scripts/check-v2-plan.py --render`, `python3 scripts/check-v2-plan.py`, `aq test scripts/test-v2-plan.py -q`, and `git diff --check`; expect consistency and passing focused checks.
- [ ] Record evidence, settle AQ subtasks, and review the diff against A1 acceptance. Squash local implementation commits to one feature commit preserving the tree, merge `origin/dev`, rerun targeted checks, publish and close with evidence.

## Execution ledger

Pre-flight: one task; no shared inter-task interfaces.

Ruling: Use the Git common checkout only to locate sibling documentation in an isolated worktree; all aq-server source/evidence stays in the assigned checkout. This preserves frozen relative paths and normal-checkout behavior.

Ruling: Follow the explicit complete-in-this-session instruction for implementation, and the profile's focused verification scope for this Python/docs-only change.
