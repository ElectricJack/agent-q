# Release Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** CI builds versioned images and a signed release manifest. A host updater, outside the application container, installs a requested release and restarts the server. It rolls back on failure, recovers from interruption at every step, and refuses unsafe releases. The server exposes update request and status through its API, and SSH commands show update state and restore the last working release.

**Architecture:**
- A new `aq-release` crate holds the shared contracts:
  - the release manifest and its compatibility rules
  - the updater's status file format
  - the format of an update request file
- A new `aq-updater` binary is a host service and the only component that touches Docker. It drives a fixed Compose project through the `docker` CLI, using argument vectors only and never a shell.
- `aq-updater` keeps its journal on the host and talks to the server through a spool directory:
  - the server writes request files into `requests/`, mounted read-write;
  - the updater writes `status.json` and a `maintenance` marker into `status/`, mounted read-only.
- There is no socket and no network path between the two, and no image reference ever comes from the server.
- Releases are published to GHCR as digest-pinned images. A `FROM scratch` "release image" (`agent-q-release:<version>`) carries `manifest.json` and its cosign bundle. Channel promotion retags that release image to `:stable`.

**Tech Stack:**
- Rust 1.96.1 (edition 2024), with the workspace deps from Plan 1.
- New crates:
  - `semver 1` (versions and upgrade ranges)
  - `fd-lock 4` (update lock)
  - `ed25519-dalek 2` (test verifier only, behind a feature)
  - `nix 0.29` with the `fs` feature (statvfs for the disk check)
- Docker Engine and Compose v2 on the host.
- cosign, pinned to one exact version in both the release workflow and the operations doc.
- GitHub Actions, using `docker/build-push-action`, `sigstore/cosign-installer`, and a `postgres:17-bookworm` service container.

**Spec:** `docs/specs/0001-infrastructure-foundation.md`. This plan covers increment 3, "Release lifecycle". The "Self-update and restart" and "Development and release delivery" sections are binding. Live VPS demonstrations and the Bluehost/Cloudflare work belong to the hosted-access plan.

## Global Constraints

- Daemon-only: no admin page or UI. Update request and status go through the JSON API.
- PostgreSQL only: `grep -E 'name = "(libsqlite3-sys|sqlx-sqlite)"' Cargo.lock` must print nothing.
- The application container never receives the Docker socket. Only `aq-updater` runs `docker`.
- The host updater accepts only validated releases for the configured Agent Queue installation. It must not expose arbitrary shell commands, arbitrary images, or user-supplied Compose definitions through the web API. The server writes only `{id, version?, requested_by, requested_at}`.
- Every `docker` invocation uses `std::process::Command` with an explicit argument vector. Nothing goes through `sh -c`. Image references are always digest-pinned (`host[:port]/path@sha256:…`, matching the manifest image regex below; ruling 24).
- The image must be downloaded and verified before the running application is stopped.
- Repeating an update request must not start concurrent replacements. Failed health checks trigger bounded rollback, not an endless restart loop.
- Recovery must handle failure after every persisted update step, including host reboot.
- An incompatible release must be refused by the automatic path until a separately tested migration and restore procedure exists. A downgrade cannot simply point an old binary at incompatible migrated data.
- Releases must be authenticated using a configured trust mechanism. Production uses cosign keyless verification with:
  - `--certificate-oidc-issuer https://token.actions.githubusercontent.com`
  - `--certificate-identity-regexp '^https://github\.com/ElectricJack/agent-q/\.github/workflows/release\.yml@refs/tags/v[0-9]+\.[0-9]+\.[0-9]+$'`
- The test verifier compiles only with cargo feature `test-verifier`. Release builds never enable it.
- Fixed values:
  - updater state dir: `/var/lib/agent-q/updater`
  - spool: `<state_dir>/spool/requests` and `<state_dir>/spool/status`
  - backups: `/var/lib/agent-q/backups`, keeping the last 5
  - readiness deadline: 120 s, polled every 2 s
  - channel poll interval: 3600 s
  - minimum free space: 2 GiB, and at least 3× the previous backup size
  - request file size limit: 4 KiB
  - history kept in `status.json`: the last 20 entries
- Release names:
  - images: `ghcr.io/electricjack/agent-q-server`, `ghcr.io/electricjack/agent-q-worker`, `ghcr.io/electricjack/agent-q-release`
  - channel tag: `stable`
  - git tags: `vX.Y.Z`, which equal the workspace `version`
- New error codes:
  - `update_in_progress` (409)
  - `maintenance` (503)
  - `updates_unavailable` (503)
  - `invalid_version` (400)
- Never stage the user's uncommitted `README.md`. End commit messages with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Power loss mid-write of the journal.** A torn `journal.json` must not brick the updater. Writes go to a temp file, then fsync, rename, and fsync the directory. A journal that won't parse still makes `aq-updater status` print the raw file and the recovery instruction, and it never auto-acts on a guess. Test: Task 3.
2. **The owner double-clicks "update", or two requests land at once.** Exactly one replacement runs. The second request is recorded as `rejected: update_in_progress` in history, both at the server (409 when status shows in-progress) and at the updater (lock plus journal). Test: Tasks 4 and 6.
3. **Release B migrated the database and then failed its probe.** Rollback must restore the pre-update backup before starting A, never start A on B's schema. If A and B share a state format, A restarts without a restore. Test: Task 4.
4. **The disk fills during the backup.** The pre-check refuses when free space is too low. If `pg_dump` still fails, the update aborts before replacement, and A is left running with maintenance cleared. Test: Task 4.
5. **The previous release won't come back after a rollback.** Rollback is attempted once. On failure, the state becomes `failed_needs_operator`, maintenance stays on, nothing loops, and `aq-updater status` prints the next steps. Test: Task 4.

---

## File Structure

```
crates/aq-release/src/lib.rs            re-exports
crates/aq-release/src/manifest.rs       ReleaseManifest, parse/serialize, compatibility rules
crates/aq-release/src/spool.rs          UpdateRequest, UpdateStatus, HistoryEntry, file formats
crates/aq-updater/src/main.rs           clap CLI: run, status, install, restore-previous, verify-manifest
crates/aq-updater/src/config.rs         /etc/agent-q/updater.toml
crates/aq-updater/src/journal.rs        journal.json + history.jsonl, atomic writes, lock
crates/aq-updater/src/engine.rs         update state machine, recovery, rollback
crates/aq-updater/src/runtime.rs        Runtime trait + DockerCli implementation
crates/aq-updater/src/verify.rs         ReleaseVerifier trait, CosignKeyless, TestKey (feature)
crates/aq-updater/src/spool.rs          request intake, status/maintenance publishing
crates/aq-updater/tests/{manifest,journal,engine,spool}.rs
crates/aq-server/src/updates.rs         maintenance gate, /api/updates, status reader
crates/aq-server/src/readiness.rs       /readyz (loopback only) and the `readiness` subcommand
Dockerfile                              + `release` target (FROM scratch: manifest + bundle)
compose.yaml                            + spool mounts on server
scripts/update-lifecycle                local end-to-end updater suite (local registry, test verifier)
.github/workflows/{ci,release,promote}.yml
docs/operations.md                      updater install, SSH status/restore, release and promote steps
```

## Shared Contracts (defined in Task 1, used everywhere)

- `ReleaseManifest` (JSON, `manifest_version: 1`, `deny_unknown_fields`) has these fields:
  - `version: semver::Version`
  - `source_revision: String`
  - `images: { server: String, worker: String }`. Both must match `^[a-z0-9][a-z0-9.-]*(:[0-9]+)?/[a-z0-9._/-]+@sha256:[0-9a-f]{64}$` (ruling 24): a host that starts with a letter or digit, an optional registry port, a path with no `:` (so no tag), then the digest.
  - `protocol: { min: u32, max: u32 }`
  - `state_format: u32`
  - `readable_state_formats: Vec<u32>` (must contain `state_format`)
  - `upgrade_from: semver::VersionReq`
  - `created_at: DateTime<Utc>`
- `pub fn check_upgrade(current: Option<&ReleaseManifest>, current_state_format: Option<u32>, target: &ReleaseManifest) -> Result<UpgradePlan, Refusal>`
  - `UpgradePlan { needs_restore_on_rollback: bool }`. It is true when `target.state_format != current.state_format`.
  - `Refusal` is one of:
    - `SameVersion`
    - `NotInUpgradePath { current, allowed }`
    - `UnreadableState { found, readable }`
    - `Downgrade { current, target }`. A lower version is refused by `install` and allowed only through `restore-previous`.
  - Every `Refusal` has a `Display` that names the fix.
  - With no current release (first install), only the `UnreadableState` check runs, and only when a state format is known.
- `UpdateRequest { id: Uuid, version: Option<semver::Version>, requested_by: String, requested_at: DateTime<Utc> }`. `None` means the channel's current release.
- `UpdateStatus` is written by the updater to `status/status.json` and read by the server. Its fields are:
  - `updater_version`
  - `state: Idle | InProgress { request_id, target, step, started_at } | FailedNeedsOperator { detail }`
  - `active: Option<ReleaseSummary>`
  - `previous: Option<ReleaseSummary>`
  - `available: Option<ReleaseSummary>`
  - `history: Vec<HistoryEntry>`, at most 20 entries, newest first
  - `updated_at`
- `ReleaseSummary { version, source_revision, server_image, state_format }`
- `HistoryEntry { request_id: Option<Uuid>, from: Option<Version>, to: Option<Version>, outcome: Succeeded | RolledBack | Refused | Rejected | Aborted | FailedNeedsOperator, detail: String, started_at, finished_at }`
- `Step` enum, persisted, in order: `Requested`, `Validated`, `Pulled`, `Maintenance`, `BackedUp`, `Replacing`, `Probing`, then `Active` or `RollingBack`. The journal records the step *before* acting on it.

---

### Task 1: `aq-release` crate — manifest, compatibility, spool formats

**Files:** create `crates/aq-release/{Cargo.toml,src/lib.rs,src/manifest.rs,src/spool.rs}` and `crates/aq-release/tests/manifest.rs`. Modify the root `Cargo.toml` to add the member and the `semver` workspace dep.

**Interfaces:** produces every item in *Shared Contracts*. `ReleaseManifest::from_json(&[u8]) -> Result<Self, ManifestError>` validates the digest regex, `readable_state_formats ∋ state_format`, and `protocol.min <= max`. `to_canonical_json(&self) -> Vec<u8>` produces stable key order and is the exact bytes that get signed.

- [ ] **Step 1: Failing tests:**
  - `parses_valid_manifest`
  - `rejects_tag_reference_without_digest` (`ghcr.io/x/y:latest`)
  - `rejects_unknown_field`
  - `rejects_state_format_not_readable`
  - `first_install_allowed`
  - `same_version_refused`
  - `outside_upgrade_from_refused`: current 0.1.0, target `upgrade_from = ">=0.2.0"`, giving `NotInUpgradePath`.
  - `unreadable_state_refused`: current state 2, target readable `[1]`, giving `UnreadableState { found: 2 }`.
  - `downgrade_refused`
  - `restore_needed_when_state_format_changes`
  - `canonical_json_roundtrips_byte_identical`
  - `status_json_roundtrip`
  - `request_rejects_unknown_fields`
- [ ] **Step 2: Run** `cargo test -p aq-release`. Expected: FAIL (crate missing).
- [ ] **Step 3: Implement** the crate.
- [ ] **Step 4: Run** `cargo test -p aq-release` and clippy `-D warnings`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: add release manifest and update spool contracts`

### Task 2: Server — manifest generation, readiness, maintenance gate, update API

**Files:**
- Create `crates/aq-server/src/{updates.rs,readiness.rs}` and `crates/aq-server/tests/{updates,readiness}.rs`.
- Modify `main.rs` (two new subcommands), `config.rs` (new `[updates]` section), `admin_api.rs` (`/api/status` `update` field), `worker_api.rs` (enroll gated), `lib.rs`, `error.rs`, aq-protocol (new codes) and `crates/aq-server/Cargo.toml` (depend on `aq-release`).

**Interfaces:**
- **Constants in `aq-server/src/lib.rs`:**
  - `pub const READABLE_STATE_FORMATS: &[u32] = &[1];`
  - `pub const UPGRADE_FROM: &str = ">=0.1.0";`
- **`aq-server manifest --server-image <REF> --worker-image <REF> --version <V> --source-revision <SHA>`:** prints `ReleaseManifest::to_canonical_json()`. It fills protocol, state format, readable formats and `upgrade_from` from this binary's constants. It refuses if `--version` ≠ `CARGO_PKG_VERSION`. CI runs this inside the built image, so the manifest describes exactly that artifact.
- **Config:** `[updates] spool_dir = "/updater"` is optional. When it is absent, the update API answers 503 `updates_unavailable`. The server reads `<spool_dir>/status/status.json` and `<spool_dir>/status/maintenance`, and writes `<spool_dir>/requests/<uuid>.json` via a temp file and rename (mode 0640).
- **`GET /readyz`:** on both listeners, but it answers only when the peer address is loopback (`ConnectInfo<SocketAddr>`). Anything else gets 404. The response is `{version, source_revision, state_format, protocol_min, protocol_max, database: "ok", auth_mode}`. It checks the database with `SELECT 1` and returns 503 with `database: "error"` if that fails.
- **`aq-server readiness`:** GETs `http://127.0.0.1:<admin port>/readyz` (port from the vault, as `healthcheck` does) and prints the JSON body to stdout. It exits 0 on 200, otherwise 1.
- **`run_with_listeners`:** serves with `into_make_service_with_connect_info::<SocketAddr>()`.
- **Maintenance gate:** a middleware on the admin router (non-GET/HEAD) and on worker `/v1/worker/enroll`. If the marker exists it returns 503 `maintenance`. Heartbeat and probe are not gated: workers keep their identity, and 5xx would be transient for them anyway.
- **`GET /api/updates`** (`CurrentUser`): returns the `UpdateStatus` JSON as-is. If the file is missing or unreadable it returns 200 `{state: "unavailable", detail}`.
- **`POST /api/updates`** (`RequireOwner`, CSRF): body `{version?: string}` with `deny_unknown_fields`.
  - A version that isn't semver gets 400 `invalid_version`.
  - If the status is `InProgress`, or an unprocessed request file is already in `requests/`, it returns 409 `update_in_progress`.
  - Otherwise it writes the request and returns 202 `{request_id}`.
  - It logs the requester id and request id.
- **`/api/status`:** its `update` field becomes `{state, active_version, available_version}`, taken from the same status file.

- [ ] **Step 1: Failing tests:**
  - `manifest_subcommand_uses_binary_constants`
  - `manifest_rejects_version_mismatch`
  - `readyz_loopback_only`: peer `10.0.0.5` gets 404, `127.0.0.1` gets 200.
  - `readyz_reports_database_error`: closed pool, giving 503.
  - `maintenance_blocks_admin_mutations_and_enroll`
  - `maintenance_allows_heartbeat`
  - `updates_unavailable_without_spool_config`
  - `viewer_reads_update_status`
  - `viewer_cannot_request_update`
  - `owner_request_writes_spool_file`: assert file contents, mode 0640, and no image fields.
  - `request_rejected_while_in_progress` (409)
  - `invalid_version_rejected`
  - `request_body_rejects_image_field`: `{"version":"1.0.0","image":"evil"}` gets 400.
- [ ] **Step 2: Run** `cargo test -p aq-server --test updates --test readiness`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test --workspace` and clippy. Expected: PASS.
- [ ] **Step 5: Commit** `feat: add readiness, maintenance gate, and update request API`

### Task 3: Updater journal, lock, and history

**Files:** create `crates/aq-updater/{Cargo.toml,src/main.rs (stub),src/lib.rs,src/config.rs,src/journal.rs}` and `crates/aq-updater/tests/journal.rs`.

**Interfaces:**
- **`UpdaterConfig`** (TOML, `deny_unknown_fields`, loaded from `--config`, default `/etc/agent-q/updater.toml`). Fields:
  - `state_dir`
  - `backup_dir`
  - `compose: { project_dir, files: Vec<PathBuf>, env_files: Vec<PathBuf>, project_name }`
  - `release_repo` (default `ghcr.io/electricjack/agent-q-release`)
  - `channel` (default `stable`)
  - `verifier: { kind: "cosign_keyless", cosign_path, identity_regexp, oidc_issuer }`. `kind: "test_key", public_key_hex` exists only with the `test-verifier` feature.
  - `readiness_deadline_secs` (120)
  - `poll_interval_secs` (3600)
  - `min_free_bytes` (2147483648)
  - `backups_keep` (5)
- **`Journal`** at `<state_dir>/journal.json`, plus an append-only `<state_dir>/history.jsonl`:
  - `pub fn open(state_dir) -> Result<Journal, JournalError>` returns `JournalError::Corrupt { path, raw }` instead of guessing.
  - `pub fn record(&mut self, JournalState) -> Result<()>` writes atomically: temp file, fsync, rename, then fsync the directory.
  - `pub fn append_history(&self, HistoryEntry) -> Result<()>`
  - `pub fn recent_history(&self, n) -> Vec<HistoryEntry>`
- **`JournalState { active: Option<ReleaseManifest>, previous: Option<ReleaseManifest>, current: Option<InFlight> }`**, where `InFlight { request_id, target: ReleaseManifest, step: Step, backup: Option<PathBuf>, needs_restore_on_rollback: bool, started_at, rollback_attempted: bool }`.
- **`pub fn lock(state_dir) -> Result<UpdateLock, AlreadyLocked>`**: a non-blocking `fd-lock` write lock on `<state_dir>/lock`, held for the updater process lifetime.

- [ ] **Step 1: Failing tests:**
  - `record_is_atomic`: simulate a crash by leaving `journal.json.tmp` with garbage. `open` ignores the tmp file and returns the last committed state.
  - `corrupt_journal_reported_not_guessed`
  - `history_appends_and_reads_newest_first`
  - `second_lock_fails_fast`
  - `config_rejects_unknown_key`
  - `test_key_verifier_kind_rejected_without_feature`: a config test compiled without the feature.
- [ ] **Step 2: Run** `cargo test -p aq-updater --test journal`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests and clippy. Expected: PASS.
- [ ] **Step 5: Commit** `feat: add updater journal, history, and lock`

### Task 4: Update engine — state machine, rollback, recovery (fake runtime)

**Files:** create `crates/aq-updater/src/{engine.rs,runtime.rs (trait only),verify.rs}` and `crates/aq-updater/tests/engine.rs`, plus `tests/common/fake_runtime.rs`.

**Interfaces:**
- **`#[async_trait] pub trait Runtime`**:
  - `fetch_release(&self, version_or_channel: ReleaseSelector) -> Result<(Vec<u8> /*manifest*/, Vec<u8> /*bundle*/)>`
  - `pull(&self, image: &str) -> Result<()>`
  - `image_labels(&self, image) -> Result<HashMap<String,String>>`
  - `free_bytes(&self, path) -> Result<u64>`
  - `backup(&self, dest: &Path) -> Result<u64 /*bytes*/>`
  - `restore(&self, src: &Path) -> Result<()>`
  - `start_server(&self, image: &str) -> Result<()>`
  - `stop_server(&self) -> Result<()>`
  - `wait_healthy(&self, deadline: Duration) -> Result<()>`
  - `readiness(&self) -> Result<ReadinessReport>`
  - `server_state_format(&self) -> Result<Option<u32>>`, which reads `meta` via `psql` in the postgres container.
- **`pub trait ReleaseVerifier { fn verify(&self, manifest: &[u8], bundle: &[u8]) -> Result<(), VerifyError>; }`**
  - `CosignKeyless` runs `cosign verify-blob --bundle <tmp> --certificate-identity-regexp <re> --certificate-oidc-issuer <iss> <tmp-manifest>` with fixed arguments.
  - `TestKey` (feature `test-verifier`) checks that the bundle is an ed25519 signature over the manifest bytes.
- **`pub struct Engine<R: Runtime, V: ReleaseVerifier>`**:
  - `pub async fn install(&mut self, req: UpdateRequest) -> HistoryEntry`
  - `pub async fn recover(&mut self) -> Option<HistoryEntry>`, called at startup
  - `pub async fn restore_previous(&mut self, allow_restore: bool) -> HistoryEntry`
  - `pub fn status(&self) -> UpdateStatus`
  - It also gets a `MaintenanceSink` trait (`enter()`/`leave()`) so Task 5's spool can toggle the marker.
- **Install sequence.** The journal records each step before acting on it. Each step maps to the spec's update sequence:
  1. `Requested`. Fetch the release, verify it, and parse it. Run `check_upgrade` against active and `server_state_format()`. On failure, end with `Refused` and the refusal's text.
  2. `Validated`. Check free space. This requires `free_bytes(backup_dir) >= max(min_free_bytes, 3 × last backup size)`, otherwise `Refused`.
  3. `Pulled`. Pull the target image and assert the label `io.agent-q.state-format == target.state_format`. On any failure, end with `Aborted`. The active release is never stopped.
  4. `Maintenance`. Call `enter()`.
  5. `BackedUp`. Back up to `<backup_dir>/<utc>-<from>-<to>.dump` and prune to `backups_keep`. If the backup fails, call `leave()`, end with `Aborted`, and leave the active release running.
  6. `Replacing`. Call `start_server(target)`.
  7. `Probing`. Call `wait_healthy(deadline)` and then `readiness()`. The report's `version`, `source_revision` and `state_format` must equal the target's.
  8. On success, `previous ← active` and `active ← target`, then `leave()` and end with `Succeeded`.
  9. On failure, go to `RollingBack`.
- **Rollback**, attempted at most once:
  1. Set `rollback_attempted = true` and persist it.
  2. If `needs_restore_on_rollback`, call `stop_server()` and then restore the backup.
  3. `start_server(active)`, then `wait_healthy` and `readiness`, which must equal active.
  4. On success, `leave()` and end with `RolledBack`, with a detail naming the failed check.
  5. On failure, set the state to `FailedNeedsOperator { detail }`. Maintenance stays entered, and no further automatic action is taken.
- **Recovery** from the persisted step:
  - `Requested`, `Validated` or `Pulled`: end with `Aborted`. Ensure the active release is running with `start_server(active)` plus a probe.
  - `Maintenance` or `BackedUp`: call `leave()`, end with `Aborted`, and ensure the active release is running.
  - `Replacing`, `Probing` or `RollingBack` with `rollback_attempted = false`: perform the rollback.
  - `RollingBack` with `rollback_attempted = true`: `FailedNeedsOperator`.
- **First install:** there is no rollback target. A failed probe calls `stop_server()`, leaves maintenance off and ends `Aborted` with the failed check in the detail.
- **Concurrency:** while `current.is_some()`, `install` returns a `Rejected` entry with detail `update_in_progress` without touching the runtime.

- [ ] **Step 1: Failing tests** (all use `FakeRuntime`, which can be scripted to fail at a named call or to "crash" after a named step by returning early with the journal left as written):
  - `successful_update_a_to_b`: history `Succeeded`, active = B, previous = A, maintenance entered then left, backup taken.
  - `unverified_release_refused_before_pull`: the fake records no `pull` call.
  - `incompatible_state_refused`
  - `low_disk_refused_before_maintenance`
  - `pull_failure_aborts_without_stopping_a`: no `start_server` call.
  - `backup_failure_aborts_and_leaves_maintenance` (Review Focus 4)
  - `unhealthy_b_rolls_back_to_a`: history `RolledBack`, active = A.
  - `readiness_version_mismatch_rolls_back`
  - `rollback_restores_backup_when_state_format_changed` (Review Focus 3): the fake asserts `restore` is called before `start_server(A)`.
  - `rollback_skips_restore_when_state_format_same`
  - `failed_rollback_stops_with_needs_operator` (Review Focus 5): exactly one rollback attempt, maintenance still entered.
  - `recover_after_crash_at_each_step`: a table-driven test over every `Step`, asserting the outcome and the final active release for each.
  - `second_request_rejected_while_in_flight` (Review Focus 2)
  - `restore_previous_requires_flag_when_state_format_differs`
- [ ] **Step 2: Run** `cargo test -p aq-updater --test engine`. Expected: FAIL.
- [ ] **Step 3: Implement** `engine.rs`, the `Runtime` trait, and `verify.rs`.
- [ ] **Step 4: Run** the tests and clippy, plus `cargo test -p aq-updater --features test-verifier`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: add update engine with bounded rollback and step recovery`

### Task 5: Docker runtime, spool, and updater CLI

**Files:** create `crates/aq-updater/src/spool.rs` and `crates/aq-updater/tests/spool.rs`. Complete `runtime.rs` (`DockerCli`) and `main.rs`.

**Interfaces:**
- **`DockerCli`** is built from `UpdaterConfig.compose`. Every Compose call is `docker compose --project-name <n> --project-directory <d> -f <f>… --env-file <e>… --env-file <state_dir>/release.env <args>`.
  - `start_server(image)` writes `release.env` atomically with `AQ_SERVER_IMAGE=<image>`, then runs `up -d --no-deps --force-recreate server`.
  - `wait_healthy` polls `docker inspect --format {{.State.Health.Status}}` on the Compose server container every 2 s until `healthy` or the deadline.
  - `readiness` is `compose exec -T server aq-server readiness`, with stdout parsed as `ReadinessReport`.
  - `backup` is `compose exec -T postgres pg_dump -U agent_q -Fc agent_q`, with stdout streamed into a 0600 file.
  - `restore` runs only with the server stopped. It runs `psql -U agent_q -d postgres -c 'DROP DATABASE agent_q WITH (FORCE)'`, then `CREATE DATABASE agent_q OWNER agent_q`, then `pg_restore -U agent_q -d agent_q` with the file on stdin, all via `compose exec -T postgres`. Recreating the database means tables a newer release added can't survive the restore.
  - `server_state_format` is `compose exec -T postgres psql -U agent_q -d agent_q -tAc "SELECT value FROM meta WHERE key = 'state_format_version'"`. A missing database or table means `None`.
  - `stop_server` is `compose stop server`.
  - `fetch_release(sel)` runs `docker pull <release_repo>:<version|channel>`, then `docker create <ref> none`, then `docker cp <id>:/manifest.json -` and `docker cp <id>:/manifest.sigstore.json -` (tar streams, extracted in memory), then `docker rm`.
  - `free_bytes` uses `nix::sys::statvfs`.
  - Every failure includes the command name and its stderr tail (at most 2 KiB). It never includes environment contents.
- **Spool** (`<state_dir>/spool`):
  - `requests/` has mode 2770 and group 10001. `status/` has mode 0755.
  - `pub fn take_requests(dir) -> Vec<Result<UpdateRequest, SpoolReject>>` returns oldest first. It deletes each file after reading it. A file over 4 KiB or that doesn't parse is rejected and recorded as a `Rejected` history entry.
  - `pub fn publish_status(dir, &UpdateStatus)` writes atomically.
  - `SpoolMaintenance` implements `MaintenanceSink` by creating or removing `status/maintenance`.
- **CLI** (`aq-updater --config <path>`):
  - `run`: take the lock, `recover()`, then loop. Every 2 s it takes requests and installs them one at a time. Every `poll_interval_secs` it fetches the channel and sets `available` if the version is newer and compatible. It publishes status after every change and exits cleanly on SIGTERM.
  - `status`: print a human summary covering state, active, previous, available, the last 10 history entries and, for `FailedNeedsOperator`, the next-step instructions. It works without the lock, and it prints the raw journal if the journal is corrupt.
  - `install <version>` (runs under the lock) and `restore-previous [--restore-backup]`.
  - `verify-manifest <manifest> <bundle>`: exit 0 or 1. CI uses it.

- [ ] **Step 1: Failing tests:**
  - `take_requests_rejects_oversized_and_malformed`
  - `take_requests_deletes_processed`
  - `publish_status_atomic`
  - `maintenance_marker_toggles`
  - `compose_args_are_fixed_vectors`: unit-test the argument builder so the image appears only as `AQ_SERVER_IMAGE=…` in `release.env`, never as a command argument.
  - `status_prints_needs_operator_instructions`: run the binary with a seeded journal.
- [ ] **Step 2: Run** `cargo test -p aq-updater`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test --workspace` and clippy. Expected: PASS.
- [ ] **Step 5: Commit** `feat: add docker runtime, spool intake, and updater CLI`

### Task 6: Compose wiring, release image, and local lifecycle suite

**Files:**
- Modify `Dockerfile`: add a `release` target (`FROM scratch`, `COPY manifest.json manifest.sigstore.json /`, build context passed by CI) and pass `AQ_SOURCE_REVISION` and the version as image labels.
- Modify `compose.yaml`: on `server`, bind-mount `${AQ_UPDATER_SPOOL:?}/requests` → `/updater/requests` (read-write) and `${AQ_UPDATER_SPOOL}/status` → `/updater/status` (read-only).
- Modify `compose.dev.yaml`.
- Create `scripts/update-lifecycle`.

**Suite design.** This is bash with `set -euo pipefail`, a trap that tears everything down, bounded polling, and named steps.
1. Start a throwaway `registry:2` on `127.0.0.1:5000`, plus a temporary state, backup and vault set. The vault is production-shaped, using `cloudflare_access` with `team_domain = "https://example.cloudflareaccess.com"`.
2. Build server images A (0.1.0), B (0.1.1) and C0 (0.1.2). B and C0 are built from temporary copies of the workspace with bumped versions, because the version is compiled into the binary and must match its manifest.
3. Build a broken image C from C0 with `FROM C0` and `CMD ["serve","--vault","/missing"]`. C's manifest is generated from C0's binary.
4. Push A, B and C. For each, generate the manifest with `docker run <img> manifest …`, sign it with a throwaway ed25519 test key, and push its release image.
5. Build `aq-updater` with `--features test-verifier`, and write an updater config that points at the real `compose.yaml` with the suite's env files.

**Cases, each asserted through `aq-updater status` and `/readyz`:**
- `install 0.1.0`: first install succeeds.
- `install 0.1.1`: succeeds; history shows A→B.
- `install 0.1.2` (C): rolled back to B within the deadline, and the failure is recorded.
- Interrupted update: start `install 0.1.2` again and send `kill -9` to the updater once the journal shows `Probing`. Then run `aq-updater run --once`. It recovers to B and records the rollback.
- Compatibility refusal: a manifest with `readable_state_formats: [2]` is `Refused` without any pull.
- Spool request: drop a request file `{id, version: "0.1.0", …}` into `requests/` in the server's format, then run `aq-updater run --once`. It must be refused as a `Downgrade`, recorded in `status.json`, and deleted from the spool. Task 2 already tests the server writing request files.

- [ ] **Step 1: Write** `scripts/update-lifecycle` and run it. Expected: FAIL at the first missing piece.
- [ ] **Step 2: Implement** the Dockerfile, Compose and script changes. Add `--once` to `aq-updater run` (process pending requests, recover, then exit), which the suite needs.
- [ ] **Step 3: Run** `scripts/update-lifecycle`. Expected: `update-lifecycle: PASS`, with each case logged. Then run `scripts/smoke`. Expected: `smoke: PASS`.
- [ ] **Step 4: Commit** `feat: wire updater spool into compose and add update lifecycle suite`

### Task 7: CI, release, and promotion workflows; operations doc

**Files:** create `.github/workflows/{ci.yml,release.yml,promote.yml}` and `docs/operations.md`. Link `docs/operations.md` from `docs/development.md`.

**Decisions:**
- **`ci.yml`** runs on pull requests and pushes to `main`:
  - fmt, clippy `-D warnings`, `cargo test --workspace` with a `postgres:17-bookworm` service and `DATABASE_URL`
  - `cargo test -p aq-updater --features test-verifier`
  - the Cargo.lock SQLite grep, `scripts/smoke` and `scripts/update-lifecycle`
  - Use `Swatinem/rust-cache`. Pin every action by full commit SHA, with a version comment.
- **`release.yml`** runs on tags matching `v*.*.*`, with permissions `contents: write`, `packages: write` and `id-token: write`:
  1. Assert the tag equals the workspace version.
  2. Build and push the server and worker images with `docker/build-push-action`, capturing their digests.
  3. Run `docker run ghcr.io/…/agent-q-server@<digest> manifest …` to produce `manifest.json`.
  4. Sign it with `cosign sign-blob --yes --bundle manifest.sigstore.json manifest.json`.
  5. Build `aq-updater` in release mode without features. Run `aq-updater verify-manifest` against the bundle with the production identity regexp.
  6. Build and push `agent-q-release:<version>`.
  7. Run `scripts/smoke` against the pushed digests. This is the spec's "isolated test deployment".
  8. Create the GitHub Release with `manifest.json`, the bundle and the `aq-updater` binary.
- **`promote.yml`** is `workflow_dispatch` with input `version`. It runs `docker buildx imagetools create -t ghcr.io/electricjack/agent-q-release:stable ghcr.io/electricjack/agent-q-release:<version>` and verifies the manifest before retagging.
- **`docs/operations.md`** covers:
  - installing `aq-updater` as a systemd service, with a dedicated user in the `docker` group and spool dir ownership and modes
  - `docker login ghcr.io` with a read-only `read:packages` token
  - first install
  - requesting an update through the API, with a curl example
  - `aq-updater status` and `restore-previous` over SSH
  - what `FailedNeedsOperator` means and the steps to take
  - cutting a release (bump the version, tag, wait for green, run promote)
  - upgrading the updater by hand, as the spec defers automatic updater replacement
  - installing a worker release by hand, from the worker image digest in the manifest

- [ ] **Step 1: Run** `actionlint` (install it with `go install` or the release binary into the scratchpad) on the three workflows. Expected: no findings.
- [ ] **Step 2: Push** the branch and open a draft PR. Expected: `ci.yml` is green on GitHub. `release.yml` is exercised later by the first real tag. That needs owner approval, because it publishes packages.
- [ ] **Step 3: Commit** `ci: add build, release, and promote workflows` and `docs: add operations guide for releases and the updater`

---

## Follow-on

- **Hosted access plan:**
  - Bluehost and Cloudflare provisioning.
  - Installing `aq-updater` on the VPS, GHCR login, and first install.
  - Live demonstrations: successful self-update, failed update, and interrupted update on the real host, including a reboot.
  - Carried over from Plan 1's final review: hardening `AQ_DEPLOYMENT`, refusing non-loopback `http://`, and the worker-port hint on `csrf_rejected`.
- **Recovery proof plan:** off-host encrypted backups, restore drills, and reboot tests.
- **Later:** automatic install from the channel. It is a policy switch on top of `run`'s poll; the mechanism is unchanged.
