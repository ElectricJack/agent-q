# MS1 execution wiring: graph task to a Claude agent in a docker-local sandbox

Date: 2026-11-05. Branches: `ms1/demo` in aq-server and aq-worker (aq-git only if
a defect forces it). Context: [MS1.md](../../implementation/MS1.md), which records
that no graph task can reach a sandbox on `main`.

## What already works end to end (and is reused unchanged)

MS0 (I0 run 13c, aq-worker `docs/implementation/I0-first-real-run.md`) proved the
**profiled native path** against a real server: register → scoped inventory →
poll/offer → proposal/accept → material lease → source hydration into a
worker-side aq-git worktree → Prepared → grant → Started → terminal relay →
Stopped → result-source capture and transfer into central retention → result
report. All of it is the existing `InputPipeline` in
`aq-worker/src/native/` plus the server's `profiled_execution`. D3
(`settlement/automatic.rs`) completes the graph task automatically after the
result report when the project's validation policy is the default completion
policy and the review requirement is `PolicyActor`.

What MS0 lacked for MS1: the only installed executor is Codex
(`runner/codex.rs`, `launch/native.rs`), the harness ran directly in the worker
process tree, and the server config was a Codex-only generator
(`aq-worker/scripts/e0-qual/definitions.py`).

## Decision: minimal path

Keep the proven pipeline and replace only the harness host:

1. **aq-worker: a Claude docker-local executor for the native pipeline.**
   - New `ExecutorConfig` kind `claude-2.1.318-docker-local-result-local-v1` and a
     launch profile beside `NativeCodexHostProfile`. The pipeline still owns
     journal, hydration, Started/Stopped, renewal, terminal output, result
     channel, source capture and the report.
   - The pinned program the session host starts in its private tmux is a small
     node-side driver (`aq-sandbox-harness`). It creates one sandbox per attempt
     through the existing `DockerLocalBackend` fixed launch policy (C2), copies
     the hydrated worktree in, delivers the leased Claude token and prompt as
     private material, starts the fixed in-sandbox runner (I4a pattern), streams
     the runner log to the pane (so the terminal relay shows Claude), waits,
     copies the workspace back into the aq-git worktree, stops and destroys the
     sandbox, then submits the AQ result claim through the installed
     `aq-agent result submit` client.
   - In the sandbox Claude runs with the C6a recipe in `container` confinement
     (`settings.json`, `HOME`/`CLAUDE_CONFIG_DIR` private home) and
     `CLAUDE_CODE_OAUTH_TOKEN` exported to the `claude` process only, read from
     the material file that is deleted before Claude starts.
   - `run-native --loop` (or repeated invocations) runs task A, then B.
2. **aq-server: config, not code.**
   - `scripts/ms1-profiled.py` generates `[intelligence]` and
     `[profiled_execution]` for this node, harness `claude` 2.1.318, credential
     `owner-claude-subscription@1`, the scratch project, and the D3 default
     completion policy (`delivery = not_required`), plus catalog and project
     binding revisions applied through the public API.
   - `AQ_DEV_INTELLIGENCE_FILE` widens to carry `[profiled_execution]`. That
     needs a fresh `aq-ms1` incarnation, because the dev vault is immutable.
   - C5b fixes: `import-login` may be the first import, and Claude tokens have
     trailing CR/LF trimmed (or refused).
3. **Evidence**: tasks A then B through the stack with IDs (task, attempt, grant,
   sandbox, template digest, retained checkpoint commit), then B3 cancel, B4
   fail→retry and restarts if time allows.

## Deliberately not done here (owners unchanged)

- **Delegated permits and the in-sandbox `sandbox-run` runner**
  (B2/B5a/B5b/B6a–c). The node, not the sandbox, holds the grant and talks to
  the server. The sandbox holds no node credential and no permit, only the
  attempt's leased token. `aq-worker run` stays fenced (`UnavailableProvider`).
- **D1 publication to the GitHub remote.** "Publish" here means retention of the
  attempt's commit in central aq-git (D3's `source_retained` gate). Delivery
  stays `not_required`.
- Template admission (C3c) and the single-UID daemon (I4a owner fix) are
  unchanged.

## Test-first

- Worker: unit tests for the driver's material/runner script (no secret in
  argv/env/labels; the material is wiped first) and the workspace copy-back
  (adds, edits, deletes; `.git` excluded), plus config validation of the new
  executor kind.
- Server: `scripts/test-dev-stack.py` gains a case for `[profiled_execution]`;
  `import-login` unit tests for first import and newline trimming.
- Live: `scripts/ms1-demo.sh run` drives A then B on aq-ms1.

## Outcome (2026-11-05 08:00 UTC)

Done live: tasks A3 → B3 through the stack (MS1.md, "Update 2026-11-05").
Deviation from "server: config, not code": real refusals required six small
server changes (D3 review requirement and outcome gate, Claude adapter in the
long-phase contract and result opt-in, `require_container` confinement, the
Claude token newline, refusal diagnostics). No migrations. Cancel reached the
server but not the executor; B4 replacement and restarts were not shown.
