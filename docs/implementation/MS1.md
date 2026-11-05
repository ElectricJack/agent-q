# MS1 — real agent, sandboxed, local

Task `stark-zenith-16.2`. Run on 2026-11-05, 05:00–05:35 UTC, on the owner's
desktop (WSL2). Owner decision of 2026-11-04: the agent is **Claude Code**
signed in with the owner's Claude subscription login (no API key), not Codex.

## Verdict

**MS1 is not met end to end.** A real Claude Code agent does run in a
docker-local sandbox on this desktop, driven by the B5 node controller with the
owner's subscription token, and the sandbox side of cancel and node restart
behaves safely. But no graph task can travel from aq-server to that sandbox:
the production worker paths that would carry it are deliberately fenced, and
no open task owns the composition. So steps 1–5 could not run *through the
stack*, and nothing in this document claims they did.

| What | Result |
| --- | --- |
| Isolated v2 stack (`aq-ms1`) from `origin/main`, seeded with the scratch repo and two dependent tasks | **Done** |
| Node controller (`aq-worker run`, docker-local, Claude template) enrolled and online against it | **Done**, but admission is fenced: it never receives work |
| Claude credential imported into the server vault | **Done** with C5b's `aq-server import-login` (merged during this run): stored version 1 of `owner-claude-subscription@1` |
| ... and leased to an attempt | **Not done**: nothing on `main` can lease it to a docker-local sandbox |
| Real Claude edit → check → publish in a docker-local sandbox | **Done at node level** (controller + real backend, no server; publish goes to a bare repo inside the sandbox, not aq-git) |
| Steps 1–5 through aq-server (graph task → attempt → grant/permit → sandbox → aq-git → D3 completion → next task) | **Not done**: blocked by the gaps in [Missing integration](#missing-integration) |
| Step 6 (Windows suspend) | Written [procedure](#step-6-windows-suspend-drill-owner-procedure), with a helper; not run by the agent |

## Environment, commits and digests

| Item | Value |
| --- | --- |
| Host | Linux 6.18.33.2-microsoft-standard-WSL2, 24 CPUs, 31 GiB RAM; Docker Desktop (host stack) and the dedicated rootless sandbox Podman 5.8.8 at `/run/user/1000/aq-sandbox/podman.sock` (single-UID mode, see I4a) |
| aq-server | Branch `ms1/demo`. The steps 1–5 runs used `df51c21abab304bb86bf76ff0a7b390c2e8bf509` (`origin/main` at the start), image `sha256:850efceb85fddaf4b28712cfa4b921cb633772cf408acecbad518f916a4ea160`. The running stack and the credential import use `2b1133512bddfe4fbdb08700a8f7bb9702688604` (`origin/main` with C5b, PR #86), image `agent-q-server:ms1` = `sha256:833eaa49f566864ec616cd60fa9f7c9cb3814b36e5f32a50d4dcbfb8b89cd7b0`. Both are local builds with no registry digest. `aq-server 0.1.0`, bundled `aq-git 0.2.0` |
| aq-graph | `aq-graph:225329cd6acf5a775fd2a0b016dd1c45fb16c79b` (pinned in `compose.dev.yaml`) |
| Postgres | `postgres:17-bookworm@sha256:839fbb5a4942a7758dd93e18b42ea5649807491a0545b5eeeb8f0b8bb11857a5` |
| aq-worker | `19ce4cde5ac3a04e7d48cfd3e7830d10b9ffbbdc` (`origin/main`) plus the `ms1/demo` test commit; `aq-worker 0.5.1`, debug build run on the host |
| Template | `127.0.0.1:5138/electricjack/aq-template-unadmitted@sha256:194e11f54fbe104734166ec219f5ac13b2e54055fae01f6c3ce264a915f172a4`, local image ID `sha256:45937ad6e4ee283342774d758823e49de1bd61080c874a245b73c3e59565721b`. This is I4a's `claude-rust-1`: the C3c `rust-1` template (`sha256:4c9c2000…62a1c5`) plus Claude Code. **Unadmitted**: the C3c scanner refuses on this host (GnuTLS) |
| Claude Code | `2.1.318 (Claude Code)`, asserted inside the sandbox; model `claude-sonnet-5-5` (subscription default) |
| Dashboard | `aq-dashboard` `3685c0d` bundle, mounted through `compose.dashboard.yaml` |

The `aq-v2-local` stack (18080/18081/18741) and the `aq-worker-local` container
were not touched.

## The aq-ms1 stack

| Endpoint | URL |
| --- | --- |
| Dashboard and admin API | <http://127.0.0.1:28080/> |
| Worker API | <http://127.0.0.1:28081> |
| aq-graph | <http://127.0.0.1:28741> |

**Stack incarnations.** The dev vault is immutable for a Compose project, so
adding the `[intelligence]` section (the Claude credential) needed a fresh
project. That is why the IDs differ between runs:

- **Incarnation 1** (05:11–05:33 UTC, server `df51c21a`) produced the step
  evidence below. Node `4eb8af55-54a4-4220-a487-9dda14fa3efa`; graph change set
  `20cb4e7f-371c-4369-8625-9e64f867a8f9`.
- **Incarnation 2** (05:33–05:40 UTC) was discarded: it lacked the base file C5b
  needs, described under [Credential handling](#credential-handling).
- **Incarnation 3** (05:40 UTC onward, server `2b113351`) is the one left
  running. It has the imported credential. Node `ms1-desktop` =
  `c7caa999-6d7b-4e5b-926c-cc80993aef76` (online, boot `3c3238c8…`, admission
  fenced). The E0 seed worker is `52029868-70b2-4fb5-b467-935172fa4ba9`. The same
  task IDs A and B were created again in change set
  `818190d4-7a1d-42fd-969a-903a7af08109`: A ready, B blocked, both at version 2
  ([incarnation3.log](MS1-evidence/incarnation3.log),
  [status-final.txt](MS1-evidence/status-final.txt)).

Compose project `aq-ms1`. State, secrets and the generated dev vault are under
`~/.local/share/aq-ms1/{state,secrets}`. The node controller's config, enrollment
and journal are under `~/.local/share/aq-ms1/worker`. Raw evidence of every run
is under `~/.local/share/aq-ms1/evidence`. Copies are in
[`MS1-evidence/`](MS1-evidence/).

## Rerunning the demo

[`scripts/ms1-demo.sh`](../../scripts/ms1-demo.sh) runs everything above;
[`scripts/ms1-stats.sh`](../../scripts/ms1-stats.sh) samples memory and CPU.
From the aq-server checkout (worktree `wt/ms1-server`):

```sh
scripts/ms1-demo.sh up              # build and start aq-ms1 (about 3 min cold)
scripts/ms1-demo.sh seed            # scratch repo + the two dependent tasks
scripts/ms1-demo.sh node            # build aq-worker, enroll once, start the controller
scripts/ms1-demo.sh import-login    # store the Claude token (C5b); idempotent, --replace rotates
scripts/ms1-demo.sh live            # real Claude: steps 1, 3 and 5 at node level, with sampling
scripts/ms1-demo.sh server-restart  # recreate aq-server under the running controller
scripts/ms1-demo.sh gaps            # print the production fences and the stuck task
scripts/ms1-demo.sh status          # containers, worker status, sandbox count, URLs
scripts/ms1-demo.sh drill           # step 6 helper (see the procedure)
scripts/ms1-demo.sh node-stop       # stop the node controller
scripts/ms1-demo.sh down            # stop controller and stack; data is kept
scripts/ms1-demo.sh all             # up, seed, node, import-login, live, server-restart, gaps
```

Inputs, all overridable by environment: `AQ_MS1_WORKER_SRC` (aq-worker checkout,
default `wt/ms1-worker`), `AQ_MS1_CLAUDE_TOKEN_FILE` (default
`~/.config/aq-secrets/claude-oauth-token`), `AQ_MS1_TEMPLATE_IMAGE` /
`AQ_MS1_TEMPLATE_DIGEST`, `AQ_DEV_INTELLIGENCE_FILE` (default
`~/.local/share/aq-ms1/intelligence.toml`, a copy is in
[MS1-evidence/](MS1-evidence/intelligence.toml)) and `CARGO_TARGET_DIR`. The
intelligence file must exist before the first `up`: the dev vault cannot change
afterwards without `docker compose -p aq-ms1 down -v` and fresh state. The template image must already
be in the sandbox daemon (I4a builds and pulls it). `seed` and `node` are
idempotent: the graph mutation replays by idempotency key, and enrollment
happens only when `worker.toml` is missing.

The node controller runs from a private copy at `~/.local/share/aq-ms1/bin/aq-worker`
(log `~/.local/share/aq-ms1/worker/controller.log`, pid in `controller.pid`).
The Cargo target dir `target-ms1` was deleted after this run, so the next
`node`, `live` or `drill` first rebuilds aq-worker (a few minutes).
`~/.local/share/aq-ms1/up.sh [COMMAND]` is a shortcut for the demo script.

Each `live` run costs three short Claude sessions (about $0.03 each at list
price; subscription usage). `live` fails loudly if any live assertion fails.

## Credential handling

- The token file `~/.config/aq-secrets/claude-oauth-token` (0600) is read only by
  the live tests (`read_token` refuses group or world access). It goes into a
  zeroizing buffer and is delivered as the controller's private material, which
  is the `/aq/private/material` archive delivered into the sandbox. Inside the
  sandbox the fixed runner reads it into `CLAUDE_CODE_OAUTH_TOKEN`, **deletes the
  file before any work starts**, and exports the variable only to the `claude`
  process tree.
- It never appears in argv, container create environment, labels, logs, the
  evidence or the commits. Checks: each test asserts no read-back output holds it;
  step 1 scans `$HOME`, `/aq` and `/tmp` inside the sandbox (0 files); and a
  token grep over `MS1-evidence/` (pattern piped from the file, never argv)
  finds 0 files.
- Claude wrote no `~/.claude/.credentials.json` in the sandbox, and the host's own
  Claude login was unchanged (size and mtime, asserted by `claude_live`).
- **Imported into the aq-ms1 vault with C5b** ([import-login.txt](MS1-evidence/import-login.txt)).
  C5b (`calm-delta-18.8`, PR #86) merged into `main` during this run, and
  `ms1/demo` was merged up to it. The setup:
  - **Configuration.** [intelligence.toml](MS1-evidence/intelligence.toml) is
    non-secret. It enters the dev vault through the new opt-in
    `AQ_DEV_INTELLIGENCE_FILE` in `scripts/dev-stack.py`. It declares
    `owner-claude-subscription@1`: `kind subscription-login`, revision 1,
    `delivery_mode server_material_v1`, `material_store
    /var/lib/aq/login-material` and `allowed_node_ids = []`. The empty list is
    deliberate: nothing could lease the login to a sandbox yet.
  - **Import.** `scripts/ms1-demo.sh import-login` pipes the token over stdin
    into an owner-only file on the server's private source volume. It then runs
    `aq-server import-login --credential owner-claude-subscription@1 --revision 1
    --source <staged file>` inside the server container and deletes the staged
    copy.
  - **Result.** Stored version **1**, `origin operator`, 108 bytes, content
    digest `25e32ec85fc9…`, in a 0700 store with a 0600 file owned by the server
    user.
  - **Leak check.** A token grep over the server logs, the controller log and
    the demo logs found 0 files.
  - **Two C5b integration findings** (worked around in the demo; follow-ups
    for the C5b/C6b owners):
    1. **`import-login` cannot be the first import.** `operator_replace`
       requires a fingerprint of the configured `secret_file` (version 0),
       otherwise it reports "no enabled subscription-login credential". The
       demo therefore stages the same token as the configured base file and
       recreates the server before importing. A login can't enter the vault
       through `import-login` alone, contrary to section 7's "operator import".
    2. **Trailing newline.** `import-login` stores the file bytes verbatim,
       including the newline that `claude setup-token > file` leaves. C6b's
       runner install accepts only ASCII-graphic Claude tokens, so such a login
       would later fail to install. The demo strips `\r\n` before staging.
       The server could reject or trim the newline for Claude logins.
  - **No rotation mid-attempt.** No attempt exists. `import-login` refuses to
    store a second version unless given `--replace`, because an operator replace
    makes running attempts get 409 at their next renewal.
- No API key was used or configured anywhere.

## Per-step results

### Step 1: two dependent tasks; Claude edits, checks and publishes

**Through the stack: not done.** **Node level: done.**

Graph (aq-ms1 workspace `e0000000-0000-4000-8000-000000000001`), change set
`20cb4e7f-371c-4369-8625-9e64f867a8f9`, [graph-seed.json](MS1-evidence/graph-seed-incarnation1.json):

| Task | ID | State | Graph version | Ready |
| --- | --- | --- | --- | --- |
| A: add a hello function to the scratch crate | `e0a20000-0000-4000-8000-000000000011` | active | 2 | yes |
| B: document hello in the README | `e0a20000-0000-4000-8000-000000000012` | active | 2 | no (blocked by A, edge `ad1ac275-be68-42bb-a097-8deae45c81ca`) |

Seed ([e0-seed.json](MS1-evidence/e0-seed.json)):

- repository `e0000000-0000-4000-8000-000000000003` is registered in central aq-git
  for `https://github.com/ElectricJack/aq-scratch.git` `main`;
- project `e0000000-0000-4000-8000-000000000002`;
- settlement namespace `cb0ca531-c218-4729-8f84-d46c0e7e8333`.

Node `ms1-desktop` = `4eb8af55-54a4-4220-a487-9dda14fa3efa` is enrolled and
`online`, with boot IDs `311a8d81…`, then `1a45ad33…` and `63345949…` after
restarts. Every boot logs
`B2 registration/issuance provider is not installed; admission fenced; foundation heartbeat remains active`
([controller.log](MS1-evidence/controller.log)). Task A therefore stays ready and
unclaimed ([gaps.txt](MS1-evidence/gaps.txt)). No task, attempt, grant or permit
ID exists to record.

Node-level real run (`claude_live`, run `run-20261105T052127Z`,
[step1/](MS1-evidence/step1/), [step1.log](MS1-evidence/step1.log)):

- The controller path was `register → reserve` (harness `claude-code`, CLI
  `2.1.318`, template digest above) `→ accept → launch` (create, deliver
  material, start runner) `→ stop → destroy`.
- Claude session `48c1161e-9d54-4c3c-acb8-f4080cc13030`: 3 turns, 4.9 s, `success`,
  `is_error false`, $0.024 list cost. It used `Edit` on `src/lib.rs` only.
- The baseline `cargo test` failed (red). After the edit the check passed
  (`check_rc 0`). The commit was published to a bare repo inside the sandbox
  (`publish_rc 0`): base `d58c454a6ca437516d86ee962ed8f52556dc1a1f` → head
  `21d5967354796131cbff712572788fc100f89733`, published = head.
- Inside the sandbox: `uid 0` (single-UID Podman, see I4a), `CapEff 0`,
  `NoNewPrivs 1`, material wiped, no token on disk, no credentials file.
- Runner `Exited(0)` after about 10 s. Stop was confirmed with the volume
  preserved, destroy was confirmed, and no owned sandboxes were left.

What this does not show: aq-git publication policy (D1a/D1b), a publication
receipt or PR, a retained source revision, or a sandbox ID bound to a server
grant.

### Step 2: automatic completion releases the next task

**Not done.** It needs a settled result from step 1. Task B is still blocked at
graph version 2 and no outbox event exists. D3's default completion path is
covered only by its own component tests (D3.md).

### Step 3: cancel mid-run (B3)

**Through the stack: not done** (no attempt exists, so there is no cancellation ID
or disposition). **Node level: done** with a real Claude session
(`ms1_cancel_mid_claude_run_stops_with_proof`,
[node/cancel/cancel.json](MS1-evidence/node/cancel/cancel.json)):

- attempt `c079bb51-e9aa-4a74-9645-79c480565d56`, sandbox
  `aq-02c0c7483e314b0f9b73f7a97e08b951-c079bb51e9aa4a74964579c480565d56`;
- Claude's first model turn came 4.3 s after launch. The stop was issued while
  Claude was writing (6 of 20 `Write` calls done) and confirmed in 6.0 s with
  proof `StoppedNoProcesses`. Runner exit was `Exited(137)`; Claude never reached
  its own exit;
- the work volume was present after the stop (preserved), the material was
  absent, destroy was confirmed and no owned sandboxes were left.

This is an operator stop. The server cancellation route
(`POST /api/execution/attempts/{id}/cancel` → node poll → controller) was not
exercised.

### Step 4: fail an attempt and see the retry (B4)

**Not done.** Replacement, retry budgets and sandbox-minute budgets are server
decisions (`profiled_execution/replacement.rs`) that need a started attempt. A
node-level "second sandbox" would prove nothing about B4, so none was staged.

### Step 5: restart the worker node, and separately aq-server, mid-attempt

**Through the stack: not done** (no attempt, so no re-adoption, renewal or
duplicate-effect evidence). Measured instead:

- **Node controller restart mid-Claude-run**
  (`ms1_node_restart_mid_claude_run_preserves_without_relaunch`,
  [node/node-restart/node-restart.json](MS1-evidence/node/node-restart/node-restart.json)):
  - attempt `8d8122e2-2411-4751-b8f9-fc4d9d51b7c3`, sandbox
    `aq-863c479cc0d44dcd889c0d7310db6e7a-8d8122e224114751b8f9fc4d9d51b7c3`;
  - the controller was dropped without stop or drain while Claude was running;
  - a new incarnation was opened on the same journal (boot `ca24fccc…` →
    `c31a2b8a…`), and reconcile took 5.9 s;
  - with no server re-adoption answer, the old-boot attempt was marked
    `NoCurrentGrant` and **stopped and preserved** (`StoppedNoProcesses`, volume
    present);
  - **owned sandboxes before = after = 1, with the same backend ID: no second
    create, no relaunch**;
  - destroy was refused 5 times during the 25 s cleanup grace (preserve before
    destroy), then confirmed.

  This is the stop-and-preserve branch of Review Focus 4 on the node side. The
  re-adopt-and-continue branch needs the server's B1 re-adoption through a real
  provider, which does not exist.
- **aq-server recreated under the running controller**
  ([server-restart.log](MS1-evidence/server-restart.log)):
  - `docker compose up --force-recreate server` ran from 05:25:21Z to 05:25:32Z;
  - controller pid 2157025 kept running with the same boot; the node was `online`
    again at 05:25:46Z;
  - no sandbox existed or was created.

  This covers the heartbeat foundation only. The real renewal-window behaviour
  is covered only by B2's `server_restart_mid_attempt_resumes_or_preserves`.
- The server was also recreated twice during setup, by the seed's settlement
  step and by the dashboard overlay. The controller restarted twice under
  `node` (boots above), with the same result.

### Step 6: Windows suspend drill

**Not run by the agent** (it requires the owner at the machine). Procedure
[below](#step-6-windows-suspend-drill-owner-procedure). The helper test was
dry-run without a suspend: one 100-file Claude run to completion. See
[Verification](#verification).

## Missing integration

These are the concrete reasons steps 1–5 cannot run through the stack. Each one
is larger than a small wiring fix (most of all, the first two are the product's
central worker composition), so none was attempted here.

1. **The node controller has no server transport (aq-worker).**
   - `crates/aq-worker/src/main.rs:700` builds the production controller with
     `UnavailableProvider`.
   - `controller/service.rs:61–84` makes `register` and `issue` return
     `ProviderUnavailable`.
   - `service.rs:114–116` states: "B2 will compose native offer/acceptance
     observations with reserve/accept_with/launch. There is deliberately no
     guessed polling or issuance route in this service."

   Missing:
   - a production `PermitProvider` over aq-server's worker API (register,
     offer poll, accept with delegated-permit handoff, `readopt`, `poll` for
     cancellation, `retain`, `emit`);
   - the service loop that turns an accepted offer into
     `reserve → accept_with → launch`.

   The only `PermitProvider` implementations on `main` are test fakes and I4a's
   `Lease`.
2. **The in-sandbox runner never launches (aq-worker).** `main.rs:423–441`,
   `sandbox-run`, validates the frozen manifest and permit transport, then bails
   with "sandbox runner B2 receipt codec is unavailable; permit transport
   validated, no launch attempted".

   The launch pieces exist as library code, but nothing in the binary calls
   them:
   - `native::runner::sandbox::run_with_login` (`sandbox.rs:240`, C6b wiring from
     `calm-delta-18.10.1`);
   - the Claude recipe (`native/adapter/claude.rs`, C6a);
   - hydration (B6c);
   - output publication.

   `calm-delta-18.9.1` ("Wire C6a harness homes into sandbox launch") is
   **PAUSED**.
3. **The server has no production graph-to-v2-offer path (aq-server).**
   - `/v2/worker/poll` offers only synthetic dev fixtures
     (`execution/api.rs:262–287`, "synthetic assignments are development-only").
   - `execution.rs:447` notes that "Production graph scheduling must supply
     checked immutable inputs here".

   Graph tasks reach workers only through the profiled native path
   (`/v1/worker/profiled/*`, the MS0 path). That path needs an `[intelligence]`
   configuration (profiles, credentials, catalog, node binding) which E0's dev
   seed does not create, and its only worker consumer is `run-native`. That
   consumer is in-process (not docker-local) and its installed executor is Codex.
   The in-sandbox runner's routes are the profiled ones
   (`runner/sandbox/auth.rs:68–81`), so the server side is meant to be the
   profiled path. The binding from a docker-local node's offer to a profiled
   attempt with a delegated permit (B2 handoff, C5a2 lease binding) is
   implemented server-side but has no worker consumer.
4. **Claude login lease (aq-server C5b, now merged).** The login is stored and
   versioned (see [Credential handling](#credential-handling)). It still has no
   path to a sandbox, because of items 1–3. The two C5b findings recorded
   there need fixing before a real lease works.
5. **Templates are unadmitted.** The C3c scanner refuses on this host
   (GnuTLS), so the docker-local allowlist here names an unadmitted digest (as
   in I4a).
6. **Single-UID sandbox daemon.** The agent runs as in-container uid 0 (no
   capabilities, no-new-privs), not the `agent` user. This is I4a's recorded
   owner fix (`uidmap`, restart without `AQ_SANDBOX_SINGLE_UID`).

**Ownership.** No open v1 task owns items 1–3. The closest closed owners are B2
(`noble-summit-37.2`, server permits), B5a/B5b (`noble-summit-37.5/.6`, the
controller "provider seam") and B6a1/B6a2 (`noble-summit-37.11/.12`, the
runner). I propose three new tasks:

- **(aq-worker)** a production B2 `PermitProvider` and the offer → launch
  service loop in `aq-worker run`, with a live test against compose.dev;
- **(aq-worker)** `sandbox-run` launch composition: the receipt codec, hydrate
  → harness home → subscription login → Claude recipe → publish → result and
  stopped reports. This continues `calm-delta-18.9.1`;
- **(aq-server)** an E0/dev seed that configures profiled execution for a
  docker-local node (profiles, catalog, node binding). The credential half now
  exists as `AQ_DEV_INTELLIGENCE_FILE` and `import-login`.

Items 4–6 belong to C5b (its two findings), C3c and the I4a owner fix. Following the operator's
instruction to treat agent-queue2 as read-only, this agent filed no v1 tasks.
The operator should file the three tasks above.

## Memory and CPU

These were sampled by `scripts/ms1-stats.sh` during run
`run-20261105T052127Z`. The stack uses `docker stats` every 5 s, the sandboxes
use cgroup `memory.peak` and `cpu.stat` every 1 s, and the node controller uses
`VmRSS`. Raw CSVs: [stats-stack.csv](MS1-evidence/stats-stack.csv),
[stats-sandbox.csv](MS1-evidence/stats-sandbox.csv),
[stats-node.csv](MS1-evidence/stats-node.csv). Summary:
[stats-summary.txt](MS1-evidence/stats-summary.txt).

| Component | Peak memory | Max CPU |
| --- | --- | --- |
| postgres | 134.3 MiB (cap 768 MiB) | 40.6% (during seed and recreate) |
| aq-server | 18.2 MiB (cap 512 MiB) | 4.9% |
| aq-git | 19.4 MiB (cap 256 MiB) | 3.3% |
| aq-graph | 3.8 MiB (cap 256 MiB) | 3.1% |
| **Stack total** (sum per sample) | **172.7 MiB** | — |
| Node controller (`aq-worker run`) | 26.5 MiB RSS | — |
| Sandbox, step 1 (Claude + `cargo test` on a one-file crate) | 531.4 MiB (cgroup peak; cap 4 GiB) | about 30% of one core averaged over the run |
| Sandbox, cancel and restart runs (Claude only, first turns) | 145.3 / 142.7 MiB | 11–14% of one core |
| Sandbox, drill dry run (Claude only, 100 sequential writes, 170 s) | 199.8 MiB | 4.3% of one core averaged over 140 s ([drill-dryrun/](MS1-evidence/drill-dryrun/)) |

Further figures are in [Verification](#verification).

**For E5 sizing:**

- The idle-to-light stack is under 200 MiB.
- A Claude sandbox needs about 150 MiB for the CLI and up to about 0.5 GiB
  with a trivial Rust build.
- These runs do **not** measure a realistic Rust build: the crates are
  one-file. The plan's "two concurrent Rust-building sandboxes" figure still
  needs a real-project build to measure.
- The aq-server sample is from a stack that never ran an attempt.

## Step 6: Windows suspend drill (owner procedure)

The full drill needs the end-to-end path. Until that exists, this interim
version exercises the same WSL2 clock and network behaviour against the real
sandbox, the real Claude session and the running node controller.

1. Make sure the stack and controller are up (`scripts/ms1-demo.sh status`
   shows `ms1-desktop online` and 4 healthy containers). Note the time
   (`date -u`).
2. Run `scripts/ms1-demo.sh drill`. When it prints
   `drill: sandbox aq-… running; suspend Windows now`, choose Start → Power →
   Sleep at once. The run lasts about 3 min if it is not interrupted.
3. Wait at least 60 s, then wake the machine and log in. Do not touch the WSL
   terminal until the drill finishes. It keeps polling every 5 s and prints
   `drill: wall-clock gap of N ms detected` after wake.
4. Record the results:
   - **Timings:** `suspend_windows` (wall and monotonic gap) and `elapsed_ms`,
     in `~/.local/share/aq-ms1/evidence/drill-*/suspend-drill/suspend-drill.json`.
   - **Did the attempt continue?** `runner_exit` (`Exited(0)` means the sandbox
     survived and finished), `claude_rc` and `write_calls` (100 means no lost
     work; fewer with exit 0 means Claude stopped early; read `claude.jsonl`).
   - **Reconcile log:** `drill-*/controller.log` (look for reconnect and
     heartbeat warnings around the gap) and `drill-*/server.log`.
   - **After wake:** `scripts/ms1-demo.sh status` shows `ms1-desktop online` with
     a fresh `last_seen_at`, and no sandboxes are left behind (`sandboxes on … : 0`).
5. Once the end-to-end path exists, repeat with a real graph attempt. Record the
   attempt's renewal and deadline, and whether the server kept the grant
   (continue) or replaced the attempt in a new sandbox (retry).

## What was changed

No production Rust code was changed. The one integration fix is in the dev
tooling: `scripts/dev-stack.py` gains an opt-in `AQ_DEV_INTELLIGENCE_FILE`, a
non-secret TOML file holding only `[intelligence]` tables. It is appended to the
generated dev vault, so E0 can carry a subscription-login credential for
`import-login`. It is tested by `scripts/test-dev-stack.py::test_prepare_appends_only_an_operator_intelligence_section`:
the file is appended, replay is stable, dropping it is refused, other tables,
non-TOML and symlinks are refused. No other defect small enough to fix inline
was found on the reachable path.

- **aq-worker**, branch `ms1/demo`: `crates/aq-worker/tests/ms1_live.rs`, with
  these tests:
  - one pure test, `long_runner_carries_no_secret_and_wipes_the_material_first`;
  - three ignored live tests: `ms1_cancel_mid_claude_run_stops_with_proof`,
    `ms1_node_restart_mid_claude_run_preserves_without_relaunch` and
    `ms1_suspend_drill_long_claude_run`.

  They reuse I4a's `tests/support/docker_live.rs`.
- **aq-server**, branch `ms1/demo`:
  - `scripts/ms1-demo.sh` and `scripts/ms1-stats.sh`;
  - `scripts/dev-stack.py` (`AQ_DEV_INTELLIGENCE_FILE`) and its test in
    `scripts/test-dev-stack.py`;
  - this document;
  - `docs/implementation/MS1-evidence/`.

  No migrations.

Operator notes found while standing the stack up (documented, not code
defects):

- **The controller root must not be pre-created.** An existing directory with
  mode 0755 gives `controller root changed`; an existing empty directory gives
  `controller file open failed`. The demo lets the controller create it.
- **`dev-stack.py seed` drops the dashboard overlay.** Its settlement step
  recreates the server with only `compose.yaml` and `compose.dev.yaml`, so the
  demo recreates the server again with `compose.dashboard.yaml`.

## Verification

- `cargo test --locked -p aq-worker --test claude_live live -- --ignored`:
  1 passed, run twice (05:08 and 05:21 UTC).
- `cargo test --locked -p aq-worker --test ms1_live ms1_ -- --ignored --test-threads=1`:
  2 passed, at 05:11 and 05:22 UTC. The first attempt at 05:09 failed on a test
  defect: it destroyed inside the cleanup grace. That run's sandbox was removed
  by hand; the test now waits out the grace and records the refusals.
- `cargo test --locked -p aq-worker --test ms1_live ms1_suspend_drill -- --ignored`
  (dry run, no suspend):
  - 20-file version: passed, 37.9 s, `Exited(0)`, 20 writes;
  - 100-file version: passed, 170.3 s, `Exited(0)`, `claude_rc 0`, 100 writes,
    no suspend windows ([suspend-drill.json](MS1-evidence/drill-dryrun/suspend-drill.json)).
- `python3 -m pytest -q scripts/test-dev-stack.py`: 13 passed. The new test
  failed first (red) and then passed.
- `scripts/ms1-demo.sh import-login` against incarnation 3: stored version 1.
  An earlier run against incarnation 2 failed with "no enabled subscription-login
  credential" (C5b finding 1).
- `cargo test --locked -p aq-worker --test ms1_live`: 1 passed, 3 ignored.
- `cargo clippy --locked -p aq-worker --test ms1_live -- -D warnings`: clean.
- `rustfmt --edition 2024 --check crates/aq-worker/tests/ms1_live.rs`: clean.
- `bash -n` over `scripts/ms1-demo.sh` and `scripts/ms1-stats.sh`. Every demo
  subcommand above except `all`, `down` and `drill` was run against the live
  stack.
- The token grep over `MS1-evidence/` found 0 files.
- Cargo settings: `CARGO_TARGET_DIR=…/target-ms1`, `RUST_TEST_THREADS` 1 for the
  live tests and 2 otherwise.

## Limits

- MS1's acceptance (owner approval after watching or rerunning) cannot be met
  until the gaps above are closed. The node-level runs are evidence for the
  sandbox, credential and recovery layers only. They are not substitutes for
  steps 1–5.
- The node-level runs publish to a bare repo inside the sandbox, not through
  aq-git, and use I4a's fixed runner, not the C6a/B7 runner with the scoped
  `aq-agent`.
- The aq-server image and the worker binary are local builds with no registry
  digests. The template is unadmitted.
- The memory figures come from trivial workloads (see [Memory and CPU](#memory-and-cpu)).
- The `aq-ms1` stack is left running for the owner.

Owner verification: pending
