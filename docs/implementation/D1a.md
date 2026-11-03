# D1a: central candidate creation and publication

Status: implementation in progress; no delivery claim.

Task: `crisp-meadow-36.1`. Plan: [implementation plan](../superpowers/plans/2026-11-02-D1a-central-delivery.md).

The central service resolves ordered receipt IDs from server-owned source retention records. It never accepts worker-supplied candidate source OIDs, retention handles, remote URLs, credentials or arbitrary operation JSON. The configured central aq-git binding owns the target allowlist. D1b owns policy and reconciliation; D7 owns command adapters.

Publication records a destination hold and a durable dispatch marker before socket I/O. No receipt after that marker means unknown outcome, including a crash before the call. This conservative case needs D1b reconciliation; the service never automatically re-pushes. Exact replies are immutable byte receipts including their newline and all protocol fields. New read operation IDs obtain fresh observations; replay returns the old observation.

Environment: Linux x86_64, repository-pinned Rust 1.96.1, disposable PostgreSQL 17 at loopback port 55432 (`scripts/test-db`). Cargo uses `CARGO_BUILD_JOBS=2`, `RUST_TEST_THREADS=2` under the foreground AQ test semaphore via a one-test pytest subprocess wrapper. No full-suite run is claimed; worker validation scope requires focused/related checks.

Real Git qualification uses retained `vendor/aq-git-v0.2.0-linux-x86_64`, SHA-256 `3feca63cffa5065566f2c604fd00d4ad92e1bc367c62ec69bedd42c2b37f35d2`, and Git 2.56.0 extracted from the preloaded pinned `aq-git-a3b:0.2.0` runtime to `/tmp/d1a-runtime/aq-git-runtime`. The temporary extraction container is removed. The acceptance fixture creates only temporary repositories, source bundles and sockets.

Validation, final SHA, review dispositions and remaining limits will be recorded after execution.

Validation so far:

- TDD red: `cargo test --locked -p aq-server --test source_retention delivery_` failed with E0432, missing `aq_server::delivery`. [Log](D1a-checks/red.log).
- `cargo test --locked -p aq-server --test source_retention`: **16 passed, 0 failed, 2 ignored**. This includes `candidate_from_retained_source_publishes_once` and all six delivery integration scenarios. [Log](D1a-checks/source-retention.log).
- `cargo fmt --check` and `git diff --check`: passed. `cargo fmt --all` cannot discover excluded vendored package workspaces in this nested worktree; the required ordinary workspace `cargo fmt`/`--check` succeeds.

The actual Git qualification is separate from the default acceptance test so ordinary tests do not require externally installed Git 2.56.0. Its explicit invocation and result are recorded below when complete. No ignored test is counted as a pass.

Reproduce the AQ-controlled Cargo invocations with [the pytest subprocess wrapper](D1a-checks/run_cargo.py): set `DATABASE_URL`, `D1A_CHECK` to the exact Cargo command, and `D1A_CARGO_LOG` to a fresh log path, then run `aq test docs/implementation/D1a-checks/run_cargo.py -q`. The wrapper fixes Cargo jobs and Rust test threads at two, preserving AQ's test semaphore and marker/worker defaults. Real qualification also sets `AQ_TEST_GIT_CANDIDATE` and `AQ_TEST_GIT_EXECUTABLE` as described above.
