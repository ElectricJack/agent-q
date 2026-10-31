#!/usr/bin/env bash
# Recorded commands; PostgreSQL uses a dedicated disposable local test fixture.
set -euo pipefail
export CARGO_TARGET_DIR=/home/jkern/dev/agent-q/target CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2
export DATABASE_URL=postgres://postgres:test@127.0.0.1:55432/postgres
cargo test -p aq-server --lib --test schema26_27 --test native_definitions --test native_material_issuance --test native_preparation --test native_source --test profiled_source_exports --test profiled_authority --test profiled_started --test profiled_stopped --test never_granted --test project_bindings --test profiled_results --test playbook_review_contract --test playbook_bridge --test dashboard_read --test dashboard_assets --test store --test updates --locked --offline
cargo clippy -p aq-server --all-targets --all-features --locked --offline -- -D warnings
cargo fmt --all -- --check
cargo test -p aq-updater --features test-verifier --test engine rollback --locked --offline
# restore-probe.py separately verifies the exact disposable PostgreSQL image,
# creates/drops only its own random databases, and runs actual dump/restore.
