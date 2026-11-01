# From agent-q source077a0ad (later commits only retain documentation/evidence):
AQ_SOURCE_REVISION=077a0ad CARGO_TARGET_DIR=/tmp/aq22-observations/target CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo build -p aq-server --bin aq-server --locked --offline
export DATABASE_URL=postgres://postgres:test@127.0.0.1:55432/postgres
export CARGO_TARGET_DIR=/home/jkern/dev/agent-q/target CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 RUST_TEST_THREADS=2
cargo test -p aq-server --test schema26_27 --test native_preparation --test playbook_activation --test playbook_bridge --test playbook_review_contract --test dashboard_read --test dashboard_assets --locked --offline
cargo clippy -p aq-server --lib --test schema26_27 --test native_preparation --test playbook_activation --test playbook_bridge --test playbook_review_contract --test dashboard_read --test dashboard_assets --all-features --locked --offline -- -D warnings
cargo fmt --all -- --check
# Actual upgraded fixture preview is separately running on127.0.0.1:4188.
# From aq-dashboard:
AQ_LIVE_URL=http://127.0.0.1:4188 pnpm test:live
