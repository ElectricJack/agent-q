# F1a: central supervisor host and durable lifecycle

Task `steady-harbor-87.1`. The earlier prerequisite block is resolved by
`crisp-nexus-12`. This increment implements the physical bridge, executable and
image on top of the previously committed SQL session owner. Independent
validation is recorded below. Overall qualification is blocked on the
newly observed upstream request-publication race described below; no passing
close is claimed.

## Source and integration

The dependency is the published aq-worker commit
`5794c3280611f6a95f2e60eab514393ad376cfd4`: aq-session-host 0.1.6, shared
Codex mapper 0.1.1 and scoped Responses transport 0.1.0. The exact source archive
SHA-256 is `e68ad4071935cec0bf7d67b5ab52e5e30364c12869e8e6b99b964b8fd69f3f1a`.
The archive and its receipt are retained in `vendor/`. Runtime sources are
unchanged; Cargo manifests normalize workspace inheritance and dependency paths,
with originals retained as `Cargo.toml.upstream`. The existing session and
terminal-relay protocol sources match the published closure. Docker verifies
the archive digest and compares the three consumed runtime source trees.
There is no sibling-checkout dependency.

Reopened checkout base: `b183e0cdbcdd7c5e597e3c0ea1b0465c19cfcc7e`.
Plan: `docs/superpowers/plans/2026-11-02-f1a-supervisor-host.md` (128f7f50).
Implementation checkpoint: 0c014048; async stack fix and migration regression:
0043e894. Integration checkpoint: ef114a62, merging
`origin/main` at ed027c42. Main already owns migration 45 for central delivery;
this branch's unintegrated owner and host migrations are now **46 and 47**.
The server's migration list, readable formats and image label advance to 47.
The old branch-only format-45 checkpoint was never a production upgrade target.

## Implemented behavior

- `aq-central-authority::SessionOwner` serializes the central actor head and
  retains immutable generations. It owns permits (at most 120 seconds), the next
  sequence, strictly later renewal expiry, original deadline, boot identity and
  terminal-state rules. Every successor derives MailHistoryFence from all retained
  generations. Refused late renewals still preserve the stopped/expired state.
- `aq-supervisor-host::Supervisor` joins CentralPlan to the exact HostPlan, reads
  actual CLOCK_BOOTTIME and boot ID, and holds an actor advisory lock on a dedicated
  PostgreSQL connection throughout physical operations. Immutable SQL callbacks
  retain intent, guardian, start intent, observations and stopped proof. A reserved
  launch is never physically replayed after a crash or failure.
- Restart checks current admission and re-adopts only the same recorded guardian
  with a live same-boot permit. A centrally committed but unapplied renewal is
  delivered only to the still-live old permit. Cancellation, expiry and failed
  observation stop and preserve scratch. Only the shared guardian's opaque
  stopped-writer proof becomes a verified central Stopped event. Guardian loss
  records StopUncertain and prohibits replacement; disappearance is not proof.
- Authorization is checked at plan/permit admission, again immediately before the
  durable physical start intent, and on re-adoption. Launch or late Running-report
  errors attempt stop/recovery without erasing the reservation or scratch.
- `aq-supervisor-host run|stop|status CONFIG` is a trusted local installation API,
  not a remote command endpoint. CONFIG admits one exact plan and resolved model;
  changing/removing it revokes that process's admission. The reusable PlanAuthority
  interface lets the central application supply current policy and original-mail
  authorization in F1b. No permissive production verifier is installed.
- The shared Codex mapper supplies the closed 0.166.0 recipe. A guardian-owned
  wrapper retains the scoped Responses transport; controller restart therefore
  does not rotate the still-live transport. The provider key is read from the
  protected `secret_file` placement only in that root wrapper. Native code runs
  as UID/GID 10001 with no supplementary groups, no_new_privs, a clean environment
  and startup via stdin. It receives only a local, bounded transport capability.
  The native executable is checked by SHA-256 and executed through its retained
  descriptor. Provider bytes never enter argv, environment, prompts or the native
  sandbox. Credential identity/content changes revoke the transport.
- `Dockerfile.supervisor-host` builds only the host and shared helper as runtime
  programs, plus tmux and TLS support. It has no worker enrollment/offers, Docker
  socket integration or production Compose service. Installation requirements and
  exact config fields are documented in `crates/aq-supervisor-host/README.md`.

## Verification

Linux WSL2, Rust/Cargo 1.96.1, real tmux, disposable PostgreSQL from
`scripts/test-db`. Each SQL test creates its own database. Every test/build/lint
uses `aq test` through `F1a-evidence/test_cargo.py`, with Cargo build jobs 2,
debug info/incremental compilation disabled and bounded test threads. The image
also builds with two Cargo jobs and an image-specific BuildKit target cache.

Invocation: `F1A_CHECK=<check> DATABASE_URL=<disposable-test-db> aq test
 docs/implementation/F1a-evidence/test_cargo.py -q --timeout=1800`.
The adapter records exact commands and separate logs in `F1a-evidence/`.

Retained red evidence:

- `host_red.txt`: missing physical API before implementation.
- `credentials_red.txt` and `adapter_red.txt`: missing credential/adapter modules.
- `revocation_red.txt`: live restart incorrectly survived policy revocation.
- `start_revocation_red.txt`: revocation during adapter preparation incorrectly
  allowed physical launch. The new pre-start check and error cleanup address it.

The latest post-merge host suite passed the credential and mapper checks and
10 of 11 integration cases (ten physical lifecycle cases plus one database
upgrade case). The one failure was the named startup case hitting the published
runtime race below. The same startup case passed earlier; the failure is retained
rather than hidden by an automatic retry. Nested SQL-owner futures are boxed at
composition boundaries after concurrency tests exposed debug-build stack growth.
The real format-45-to-47 upgrade preserves an existing central delivery intent.
Both exact named workspace commands passed, each executing its one intended
physical integration test (the other ten host cases filtered out). The focused
SQL session-owner suite passed 8/8 and authority-model suite passed 16/16. Host
and related authority/server clippy checks passed with warnings denied. The
state-format registration check passed 1/1 and `cargo fmt --check` passed.
Corresponding logs are `owner.txt`, `model.txt`, `host_clippy.txt`, `clippy.txt`,
`format_version.txt` and `format.txt` (successful formatting emits no output).

The exact task acceptance commands are retained in the adapter as `named_start`
and `named_restart`; they run real guardian/tmux tests, not persistence-only
fixtures. Full command output is retained in `named_start.txt` and
`named_restart.txt`:

- `cargo test --locked --workspace supervisor_session_starts_under_central_permit`
- `cargo test --locked --workspace supervisor_restart_resumes_without_duplicate_session`

The image check builds the actual image and runs a fixture native CLI inside it.
The CLI must receive the exact startup stdin, run as UID 10001, have no provider
key in its environment and fail to read the root-owned credential file. It uses
a dummy key, an isolated network and no provider request. This checks process
separation, not native/provider qualification. The image check at source
checkpoint 0043e894 passed with image digest
`sha256:b102f5ed4910946cd7f2b22fa6d492fa9de8589c513a115b102208899031ad54`;
the build and boundary result are retained in `F1a-evidence/image.txt`. The
subsequent lint cleanup only avoids allocating a PathBuf for a path comparison.

## Open upstream publication race

`F1a-evidence/upstream_start_publication_race.txt` records a natural failure:
`session ended before startup: startup failed: EOF while parsing a value at line
1 column 0`. In the pinned 0.1.6 source, `SessionHandle::request` (lib.rs:907-924)
calls `durable_new`, which opens the final request path with create_new before
writing JSON (lib.rs:1106-1114). The guardian sees `start.request` existence and
immediately parses it (supervisor.rs:208-211); a reader scheduled before the write
sees empty/truncated JSON and enters Stopping (supervisor.rs:306-310). State
replacement uses atomic rename, but this first request publication does not.
The failure is safe refusal, not execution without authority, and the central
bridge attempts verified cleanup. It still prevents reliable startup acceptance.

Reported once to `session:supervisor-aq-platform` as
`msg-da16d0b744f349cbb654d80b47f3fd71`, requesting an upstream atomic-publication
fix and pinned-release/admission guidance. No vendor fork, sibling edit, silent
launch replay or test retry was introduced. A later isolated passing test does
not resolve this known failure. The held claim awaits upstream disposition after
independent work is published.

## Limits and review

ResourcePolicy::CallerAdmission records admission, not hard cgroup enforcement.
The native executable is an explicitly mounted, digest-pinned operator-supplied
runtime; no real Codex/provider qualification or aggregate spend enforcement is
claimed. Private PID namespace and root/native UID separation are required; this
is not hostile-kernel isolation. A container/guardian loss can remain uncertain
and require operator inspection; it never authorizes replacement by inference.
Original-mail delivery/reply/tool credentials (F1b), production orchestration
(E1c), enqueue (F2), wait activation (H4/H6) and final system security review (J4)
remain outside scope. No architecture/specification files were changed.

The full unfiltered workspace suite is not claimed: prime restricts this worker
to focused and related-area checks. The two required filtered workspace commands
are explicit acceptance obligations. Cross-family adversarial review and the
required PR remain outstanding before a passing close.
