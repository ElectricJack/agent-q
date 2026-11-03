# MS0 — first real agent run gate

Task `stark-zenith-16.1`; evidence reviewed 2026-11-03 UTC.
**Gate fails: I0 records 0/3 successful runs.** Real Codex authentication and
model responses occurred, but no edited source was retained and no run reached
settlement. The owner's conditional pre-approval therefore does not authorize a
pass. No MS1 contracts or dependencies were changed.

## Owner decision, verbatim

Owner `user:local`, 2026-11-02, task comment
`comment-626f0ab3daf0494aa56f0074ddb52482`:

> Owner pre-approved MS0 on 2026-11-02 before stepping away. Verification switched from human to qa_agent: close MS0 when I0's evidence shows three real Codex API-key runs with retained source and recorded outcomes, and every problem found is filed as a task. If I0 could not complete (for example no API key or scratch repository was provided), do not pass MS0; close it fail with what is missing.

This records the existing decision, not a new approval. The current I0 contract
uses the owner's ChatGPT login without an API key; the MS0 contract and comment
retain earlier API-key wording. I0 actually exercised ChatGPT login. This
inconsistency cannot change this gate's outcome: neither authentication path has
three successful retained and settled runs in the evidence. Owner sign-off for
any future pass must resolve which authentication requirement applies.

## Evidence and provenance

Reviewed aq-worker task `azure-horizon-22.1` report at commit
`17bb649da4aa8c5dd9d42bb12b585f6b3ad3a42a` (published merge; report change `7dba9b7`). The local
checkout was detached and clean. Its original location was
`/home/jkern/dev/aq-platform-agents/projects/aq-worker/.aq/worktrees/slot-1/docs/implementation/I0-first-real-run.md`.
A [frozen report copy](evidence/MS0/I0-first-real-run.md) and its SHA-256 in
[read-only observations](evidence/MS0/observations.json) preserve this review
across workspace recycling. Historical preflight statements in that report are
explicitly superseded by its 2026-11-03 run section.

I0 ran Codex CLI 0.166.0 through executor
`codex-0.166-chatgpt-interactive-result-local-v1`. Its image was
`sha256:0b7defe69f3e4cfde180f1de332cdae169aa16ac71805875770c4537ce9ddc7d`,
worker source `dc62cfc0faf2109485b1c9f17a024f6fcb37ebd5` (worker 0.5.1),
built using aq-server's archived E0 source
`8f6c49fd91850b924524494e26c5c17d14072796`. This was not a rebuild from current
worker main. Local image inspection confirms the image still exists; no registry
RepoDigest or reproducible-build claim is made.

The subsequent `aq-e0-qual` stack uses server source
`c802fed50bd579009b13a4cc11f77af52fe88d22`, image
`sha256:6f119d44a482162e7c8d8c04255e56e1e571b9b9e518efd03a8c0fbbd4fd4f96`.
Graph image is
`sha256:807beba98dcd20b0ae335f09cae8c06ecb182a352ee78de278cb47fccb2ee3be`;
Git support image is the earlier E0 image
`sha256:94a3d9adea2cf2f693cde66e8c0b58dd9d9cd3a2b47eced78545092dbefdb638`.
See [E0](E0.md) and [E0 refresh](E0-refresh.md) for their source/build evidence.

## Run-by-run acceptance audit

These are the latest three reported invocations, not three successes. The
original planned runs 1–3 were never admitted. The report contains no successful
three-run sequence.

| I0 invocation | Attempt ID | Grant ID | Graph task ID | Retained output revision | Settlement outcome / route |
| --- | --- | --- | --- | --- | --- |
| 9 | Report abbreviates `2d41bbec…`; full ID missing | Report says granted; ID missing | `e0a10000-0000-4000-8000-000000000004` | None | Stopped after `started`; relay 503 `identity_unavailable`; no settlement route reached |
| 10 | Not admitted | None | Second task `e0a10000-0000-4000-8000-000000000005` created for retry; no admitted binding | None | Exit 2 in 0.3 s: `/state/host` absent; no settlement route reached |
| 11 | `87d16454-c2cc-593c-9bfd-60ac4ea56911` | `d56e197c-3955-4dd0-bc52-2016724b7e58` | `e0a10000-0000-4000-8000-000000000005` | None | Exit 2 after 944.4 s; permit expired; no evaluation, combined review or completion reconcile |

The report identifies the shared worker image/source above; it does not provide
separate immutable per-container image inspection for each invocation. Run 11
used `gpt-6.1-sol` high, with one auxiliary `gpt-5.6-luna` turn. A stand-in
answered the interactive trust prompt through the public terminal relay.
Both tool calls then failed because `/opt/aq-runtime/codex-code-mode-host` was
missing. The agent reported no files changed and no AQ result submitted.
The host reaped three processes; the report says the attempt was stopped,
retention pending, with no profiled result head, report or settlement evaluation.
That last server detail is reported by I0, not independently verified here.

I0 reports no login refresh and an unchanged 0600 owner `auth.json`. This task
did not read, copy or hash that credential. Run state is reported preserved at
`/state/preserved/run9` and `/state/preserved/run11`; this review did not modify it.

## Independent stack checks

Observed at the timestamp in [observations.json](evidence/MS0/observations.json):

- All four long-running `aq-e0-qual` services are healthy, as is the retained
  `aq-e0-final` server. Qualification admin/worker/Graph ports are 32848/32849/32847;
  E0 final uses 32838/32839/32837. Health is not evidence of completed execution.
- Authenticated Graph GETs for tasks `…0004` and `…0005` both return 200,
  `task_state: active`; neither is completed.
- A public aq-git `retention.inspect` request confirms base handle
  `e0a10000-0000-4000-8000-000000000011` is active, generation 9, binding
  generation 1, retaining `2e32645bab743729bfaeb0f2eec417c2ace7794b`.
  This is the original scratch base, not an edited I0 result. The separate
  E0-final base has handle prefix `e0000000`; they are distinct stacks.
- `GET /api/execution/attempts/87d16454-c2cc-593c-9bfd-60ac4ea56911` returns
  404. The legacy attempt-inspection surface did not verify this native
  profiled attempt; a 404 is not proof that it never existed.
- No admission, terminal input, source mutation, settlement mutation, service
  restart, provider run or owner credential reconciliation was performed.

## Problems and proposed MS1 impact

All IDs below are transcribed from I0. Finding labels summarize the report;
canonical task titles, stored priorities/severities and current task status were
not independently queried outside this worker's scope. Severity and affected
MS1 tasks are **proposals**, not owner decisions. “Yes” means a contract or
qualification clarification may be needed; it does not edit that task's scope.
The report identifies eleven filed tasks. Server-only findings are a cross-repo
comment without filed server task IDs, so “every problem filed” is not proven.

| v1 task | Finding label from I0 | Proposed severity | MS1 tasks potentially affected | Affects MS1 scope? |
| --- | --- | --- | --- | --- |
| `sharp-harbor-18` | Missing real native qualification/configuration inputs | High | B2, B6a1, C6a, I4a | Yes: exact measured inputs must reach the runnable stack |
| `quick-glacier-78` | Human settlement runtime/routes absent from original local stack | High | D3, I4a | Unclear: later qualification stack differs; recheck mounting before expanding scope |
| `fresh-delta-13` | Refreshed ChatGPT login lacks durable owner/server reconciliation | High | C5b, C6b, I4a | Yes: rotation/writeback is part of real-login qualification |
| `vivid-bridge-32` | Imported project/supervisor directions exceed live project scope | Medium | I4a/operator runbook | No: v1 task routing/runbook issue, no demonstrated MS1 runtime change |
| `amber-horizon-91` | Native image lacks current worker, helper/client, pinned tmux/Codex and libraries | High | C1, C3a, C6a, I4a | Yes: image/runtime input contract |
| `stark-grove-17` | Baseline formatting and nested-worktree metadata failures | Medium | I4a verification | No: repository validation issue; no runtime contract change established |
| `sharp-dune-88` | Default Docker seccomp blocks the real Codex sandbox probe | High | C2, C3a, C6a, I4a | Yes: supported sandbox configuration requires explicit qualification |
| `azure-horizon-22.1.1` | Image omits Codex companion executables/resources; tools fail | Blocker | C1, C3a, C6a, I4a | Yes: full pinned Codex package layout must be installed |
| `azure-horizon-22.1.2` | Interactive executor stalls at workspace trust prompt | High | C6a, I4a | Yes: unattended launch must address workspace trust |
| `azure-horizon-22.1.3` | Stopped unsettled attempts consume graph tasks without native recovery | High | B4, B7, D3 | Yes: stopped-attempt recovery and disposition ownership |
| `azure-horizon-22.1.4` | run-native/check-native hide failure cause | Medium | C6a, I4a | Unclear: could be diagnostics only; owner to decide |
| No server task ID in report | Relay one-poll-per-node 409 storm (219 in run 11), one output-poll 500 | High | D1b, I4a | Unclear: needs server triage and task filing |
| No server task ID in report | Input deadlines over two seconds produce generic 422 | Medium | D1b, I4a | Unclear: classify protocol limit versus usability defect |
| No server task ID in report | Missing server release for held graph tasks | High | B4, B7, D3 | Yes: coordinated recovery needs explicit server owner |
| No server task ID in report | Missing `[terminals]` validation permits an unusable qualification stack | High | C6a, I4a | Unclear: corrected local configuration versus product validation |

The server findings already have an originating I0 cross-repository follow-up
comment. This gate reports the missing server IDs to the supervisor for routing;
it does not invent IDs or duplicate filings without their provenance. No problem
was fixed as part of MS0.

## Decision questions for owner/supervisor

1. Keep the failed MS0 gate until I0 records three consecutive real runs with
   edited source retained and actual settlement outcomes? The existing owner
   instruction answers yes; this review applies it.
2. Confirm the updated ChatGPT-login requirement supersedes the older MS0
   API-key wording before any future passing verification.
3. Which proposed MS1 impacts above require contract changes, and which are
   covered by existing work? In particular: complete Codex package, unattended
   trust handling, sandbox settings, login rotation and held-task recovery.
4. Supply the server follow-up task IDs and canonical problem titles/severities,
   or route the I0 cross-repository comment into server tasks. Until then, the
   complete-filing requirement remains unverified.
5. After the image/configuration/recovery fixes and fresh qualification inputs,
   resume I0 and provide three full attempt/grant/task IDs, retained revisions,
   image/source provenance and settlement receipts for MS0 re-verification.

These questions accompany the concrete failed evidence record. There is no new
owner approval recorded and no implied approval of an MS1 scope change.

## Demo script and expected observations

This is a documented replay, **not executed by MS0**. The historical image will
reproduce the missing-tool failure. A successful replay requires the filed image
and trust fixes, current measured input/pin regeneration, a runnable settlement
owner, and a fresh offerable scratch graph task; the held tasks cannot simply be
reused. Do not treat the existing `check-native` result as a tool-execution pass.
The measurement in I0 expires 2026-11-04 07:22:45 UTC.

Use a stable checkout of worker report revision `17bb649` for the historical
helper scripts, and the retained credential-free build directory. Paths below
are local handoff paths, not repository dependencies.

### 1. Verify the E0 stack

From an aq-server checkout:

```sh
AQ_SERVER_IMAGE=sha256:6f119d44a482162e7c8d8c04255e56e1e571b9b9e518efd03a8c0fbbd4fd4f96 \
AQ_DEV_GRAPH_IMAGE=sha256:807beba98dcd20b0ae335f09cae8c06ecb182a352ee78de278cb47fccb2ee3be \
AQ_DEV_SMOKE_NO_BUILD=1 scripts/compose-smoke.sh dev
```

Expected: `compose_dev_reaches_ready_with_graph_and_git` passes. This command
brings up and tears down a disposable E0 stack; it does not leave an I0 stack.
The separate, already retained qualification stack is inspected from aq-worker:

```sh
python3 scripts/e0-qual-stack.py status
```

Expected: `aq-e0-qual` healthy at admin 32848, worker 32849, Graph 32847.
For a missing qualification stack, the owned helper commands are `prepare`,
`up`, `enroll`, `seed`, `measure`, `configure`, `apply`, in that order. They
install real private material and measured configuration: use the authorized
handoff, never fixture measurements or a replacement project root over held work.
The simple E0 smoke alone does not supply these inputs.

### 2. Build and inspect the historical I0 image

```sh
docker build --progress=plain --target i0 \
  --build-arg AQ_SOURCE_REVISION=dc62cfc0faf2109485b1c9f17a024f6fcb37ebd5 \
  --build-arg AQ_WORKER_ARCHIVE_SHA256=38dfa8fed216d7bc2621d68d58550849714fdd009db4006fadfffb743356ef01 \
  -t aq-i0-worker:dc62cfc /home/jkern/.local/state/aq-platform-i0/build
docker image inspect --format '{{.Id}}' aq-i0-worker:dc62cfc
```

Expected: build completes, image provides worker 0.5.1 and Codex 0.166.0.
The previously observed image ID is recorded above; repository package drift
means rebuilding need not reproduce that ID. This exact historical context
omits Codex companions, so it cannot produce successful tool calls. Successful
qualification must use the corrected image, matching installed pins and current
worker source, recording the new digest/commit instead of claiming the old one.

### 3. Run one fresh native invocation

From the worker checkout, after qualification configuration and task recovery
are ready, set `MS0_WORKER_IMAGE` to the immutable qualified image ID. The
following creates a separate fresh state volume and copies the existing private
configuration without printing it. It preserves all earlier attempts.

```sh
: "${MS0_WORKER_IMAGE:?set the qualified immutable worker image ID}"
MS0_RUN_NAME="aq-ms0-demo-$(date +%s)"
MS0_VOLUME="${MS0_RUN_NAME}-state"
MS0_BUNDLE=/home/jkern/.local/state/aq-platform-e0-qual/qual/worker
docker volume create "$MS0_VOLUME"
docker run --rm --network=none --user=0:0 --cap-drop=ALL \
  --cap-add=CHOWN --cap-add=DAC_OVERRIDE --cap-add=FOWNER \
  -v "$MS0_VOLUME:/state" -v "$MS0_BUNDLE:/bundle:ro" \
  --entrypoint=/bin/sh "$MS0_WORKER_IMAGE" -c '
    set -eu; umask 077
    chown 10001:10001 /state; chmod 0700 /state
    for d in config host git; do install -d -o 10001 -g 10001 -m 0700 /state/$d; done
    for f in native.json adapter.json client.json composite.json inventory.json worker.toml; do
      install -o 10001 -g 10001 -m 0600 /bundle/$f /state/config/$f
    done'
docker run --name "$MS0_RUN_NAME" --network=host --user=10001:10001 \
  --read-only --tmpfs=/tmp:mode=1777 --cap-drop=ALL \
  --security-opt=no-new-privileges --security-opt=seccomp=unconfined \
  -v "$MS0_VOLUME:/state" \
  -v "$PWD/scripts/e0-qual/i0-run-native.py:/i0-entry.py:ro" \
  --entrypoint=/usr/bin/python3 "$MS0_WORKER_IMAGE" /i0-entry.py
```

`seccomp=unconfined` reflects the bounded I0 qualification exception, not a
production sandbox or hostile-code isolation claim. The entry starts worker-side
aq-git, verifies its worker binding, and executes exactly:

```sh
/opt/aq-runtime/aq-worker --config /state/config/worker.toml \
  run-native --native-config /state/config/native.json
```

Expected after fixes: one admission/grant, real tool execution, code changes,
retained source and a result report. Record attempt/grant/task IDs and retained
OID. On the historic image, expect the trust prompt, missing companion tool
failure and no settled result. On any failure preserve the container/volume;
do not remove private auth state or edits. The existing manual auth helper takes
`--owner`, `--baseline`, `--refreshed`; reconcile any actual refreshed login
atomically at 0600 before disposal, with owner writers quiescent as I0 documents.
Do not invent an output auth path or assume absence means successful rotation.

### 4. Settle through the human-review route

This step is unreachable for the recorded runs. After an actual result and
retained source exist, an eligible owner inspects the exact combined decision,
answers it, evaluates the matching intent and reconciles the original completion.
Set `MS0_DECISION_ID` to the real decision ID and `MS0_OPERATION_ID` to the stable
evaluation/completion operation ID. The two JSON files below must contain the
actual bound values from that run; they are not fixture bodies.

```sh
: "${MS0_DECISION_ID:?set the real combined decision ID}"
: "${MS0_OPERATION_ID:?set the stable evaluation/completion ID}"
MS0_ADMIN=http://127.0.0.1:32848
curl --fail-with-body -sS "$MS0_ADMIN/api/reviews/combined/$MS0_DECISION_ID"
curl --fail-with-body -sS -H 'X-AQ-Request: 1' -H 'Content-Type: application/json' \
  --data-binary @ms0-review-answer.json \
  "$MS0_ADMIN/api/reviews/combined/$MS0_DECISION_ID/resolve"
curl --fail-with-body -sS -H 'X-AQ-Request: 1' -H 'Content-Type: application/json' \
  --data-binary @ms0-evaluate-intent.json "$MS0_ADMIN/api/settlement/evaluations"
curl --fail-with-body -sS "$MS0_ADMIN/api/settlement/evaluations/$MS0_OPERATION_ID"
curl --fail-with-body -sS -H 'X-AQ-Request: 1' -H 'Content-Type: application/json' \
  --data-binary '{}' "$MS0_ADMIN/api/settlement/completions/$MS0_OPERATION_ID/reconcile"
```

`ms0-review-answer.json` contains `expected_subject` (the complete current
`{id, version, digest}` reference) and `answer` with `schema_version: 1`, a stable
answer `operation_id`, the real `decision_id`, `expected_decision_revision`,
owner-chosen `choice` (`approve` or `reject`) and `comment`. This script does not
choose approval for the owner. `ms0-evaluate-intent.json` contains exactly
`schema_version: 1`, `operation_id`, `report_id`, `source_operation_id`,
`decision_id`, `subject` (the same exact ArtifactRef). No third request file is
needed: completion's closed body is `{}`. Requests use E0's dev owner identity;
authenticated deployments need their normal owner session.

Expected: an immutable human decision receipt, evaluation receipt and confirmed
completion followed by a completed Graph task. An evaluation before the human
answer may return 503; retry the same exact evaluation intent after the answer.
On conflict or uncertain dispatch, inspect/reconcile the original operation;
never manufacture a new approval or operation. See
[GATE-04 route contract](GATE-04-human-settlement-http.md) and the actual request
types in `crates/aq-server/src/profiled_execution/settlement/owner.rs` and
`crates/aq-review-protocol/src/lib.rs`. A successful demo is one run, not the
three-run acceptance evidence still owed by I0.

## Verification and limits

Documentation-only changes; no Rust workspace, dashboard source or wire
contract changed. No named test deliverables exist for MS0. Application TDD,
Cargo Clippy/tests and dashboard suites are not applicable; prior I0/E0 tests
are references, not new MS0 execution qualification.

- Initial `test -f docs/implementation/MS0.md`: exit 1, document absent.
- Final documentation checks: `git diff --check`; Python JSON/report hash,
  run-ID and local-link consistency checks; `bash -n` over extracted `sh` blocks.
- Live observations: two bounded Graph GETs, attempted legacy attempt GET,
  aq-git `retention.inspect`, formatted container health/image inspection.
- Environment: local WSL2/Linux, Docker Desktop, Python 3; checked tools and
  sanitized observation timestamp are recorded in the evidence directory.
- Plan commit: `97be5493`; final evidence commit is identified by the guarded
  AQ publication receipt and task-close record to avoid a self-referential hash.
- Remaining limits: 0/3 successful runs; incomplete run-9 IDs; no retained I0
  result revision, report, human review or settlement receipt; canonical problem
  metadata and server follow-up IDs unverified; demo documented, not executed.

Disposition: publish this record and close MS0 **fail**, preserving the owner's
conditional decision. Reopen only with the missing I0 and filing evidence.
