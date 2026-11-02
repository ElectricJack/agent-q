# A4 Image and CI Hygiene Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pin the server and worker base images, enforce the server's default state-format label, remove obsolete playbook sources, run contract checks in CI, and publish the profile CLI as `aq-profile`.

**Architecture:** Keep the existing image stages and release build arguments. A server integration test reads Dockerfile defaults and compares them with the source constants. Keep only the playbook source closure consumed by the workspace.

**Tech Stack:** Rust 1.96.1, Cargo, Docker BuildKit, Python 3, GitHub Actions.

**Spec:** A4 in `docs/superpowers/plans/2026-11-02-v2-full-application-tasks.yaml`; Global Constraints in `docs/superpowers/plans/2026-11-02-v2-full-application-plan.md`.

## Global Constraints

- Rust uses the committed lockfile with `--locked`; repositories build alone without sibling dependencies.
- While v1 is live, no v2 binary, service or CLI may be installed under the names `aq` or `agent-queue`.
- Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked --workspace` for this Rust workspace.
- Record commands, environment, commit and limits in `docs/implementation/A4.md`; publish through guarded AQ integration.
- Leave the pinned aq-git stage blocks, release build arguments, historical playbook revision identifiers, Compose images and manifest format unchanged.
- Record the worker's stale version/state-format defaults separately; correcting them is outside A4.
- A4 has direct integration and no adversarial-review requirement. Perform the native execution skill's final fresh review.

## Review Focus

- A migration changes the source state format while a plain image still advertises an older format: `dockerfile_state_format_matches_source` catches this.
- A worker protocol maximum changes without its image default changing: `dockerfile_protocol_max_matches_source` catches this.
- The renamed executable still advertises `aq` in help: `profile_cli_uses_distinct_binary_name` catches this.
- Old playbook source deletion accidentally removes a live dependency: the source check and locked workspace build/tests catch this.
- Docker's build context lacks the file read by the new test: use a runtime file read in an integration test, then validate a plain image build separately.

---

### Task 1: Reproducible image inputs and CI hygiene

**Files:**
- Create: `crates/aq-server/tests/image_labels.rs` (Dockerfile defaults versus source constants).
- Modify: `Dockerfile`, `Dockerfile.worker` (only non-aq-git bases; server default 41).
- Modify: `Cargo.toml` (remove three stale excludes).
- Delete: `vendor/aq-playbooks-{42f9449,a463e59,d3bd349}` and their respective `aq-playbooks-source{,-0.1.1,-0.1.2}.json` pins.
- Modify: `.github/workflows/ci.yml` (two source-contract checks in lint).
- Modify: `crates/aq-profiles/Cargo.toml`, `src/main.rs`, `tests/cli.rs`, `README.md` (binary name, help, errors, test binary lookup).
- Create: `docs/implementation/A4.md` (evidence and remaining limits).

**Interfaces:**
- Consumes: `aq_server::STATE_FORMAT_VERSION: u32`, `aq_protocol::SUPPORTED_WORKER_PROTOCOLS: (u32, u32)` and Dockerfile `ARG` defaults.
- Produces: digest-pinned `build`/`runtime`/`worker` bases, truthful default server labels, and `aq-profile profile ...` with the existing subcommands.
- Test helper: `fn dockerfile_arg(name: &str) -> u32`, reading the repository Dockerfile using `CARGO_MANIFEST_DIR`; require exactly one matching numeric default.

- [x] **Step 1: Write the failing tests.** Add `dockerfile_state_format_matches_source` asserting `dockerfile_arg("AQ_STATE_FORMAT") == aq_server::STATE_FORMAT_VERSION`; add `dockerfile_protocol_max_matches_source` asserting the protocol maximum. Add `profile_cli_uses_distinct_binary_name`, executing the existing binary's `--help` and asserting successful output contains `Usage: aq-profile <COMMAND>`.
- [x] **Step 2: Verify red.** Run `cargo test --locked -p aq-server --test image_labels` (state format fails 40 versus 41; protocol passes), and `cargo test --locked -p aq-profiles --test cli profile_cli_uses_distinct_binary_name -- --exact` (help still names aq). Keep logs.
- [x] **Step 3: Apply configuration changes.** Use rust digest `sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663`. Resolve `debian:bookworm-slim` using `docker buildx imagetools inspect` and pin both runtime stages to its index digest. Set server `AQ_STATE_FORMAT=41`. Delete only the three specified trees and pins, preserving `0173673`, its 0.1.3 pin and tar. Add both Python checks beside existing lint checks. Rename the Cargo target and clap name to `aq-profile`, update its error prefix and integration-test binary lookup.
- [x] **Step 4: Verify green.** Run both complete focused test targets, the existing `state_format_is_the_latest_migration` test, and both Python check scripts. Confirm the historical playbook identifiers and aq-git blocks have no changes.
- [x] **Step 5: Validate acceptance and definition of done.** Run formatting, workspace all-target clippy, locked workspace tests and build. Run `docker build -t aq-a4-server:eager-grove-45-4 .` with no build args, then inspect `io.agent-q.state-format` and compare it to 41. Record actual failures and unavailable checks, without broadening scope.
- [ ] **Step 6: Commit and review.** Record evidence in `docs/implementation/A4.md`; commit implementation. Obtain the native execution skill's final fresh whole-branch review and resolve substantive findings. Squash this task's local plan/implementation history into one leaf commit before first publication, preserving the final tree and author identity.
- [ ] **Step 7: Publish.** Merge `origin/dev` into the assigned branch, resolve with context if needed, run targeted checks after any merge changes, publish with `aq git push`, record HEAD and settle the AQ checklist. Close pass only with all acceptance checks met; otherwise close fail with exact remaining work.

## Execution notes

`aq test --help` invokes a Python pytest runner in this installation, with no Rust execution interface. Use direct Cargo commands with `CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=2`, `CARGO_INCREMENTAL=0`, and debug info disabled, in this isolated worktree. Retain logs in this plan's ignored workspace. Do not raise the AQ pytest worker cap or mutate shared build directories.
