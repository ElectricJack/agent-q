# Historical migration fixture qualification

Task: `eager-grove-45.13`; source base: `cc7b74e6a723a6dade7b632af514e6d90b7c7abb`.

Reservation owner fixtures and the retained-owner corruption test open actual
registered state37 Stores through a unit-test-only constructor. That constructor
uses the existing bounded migration profile and normal pool initialization.
The reservation fixture checks the persisted state-format metadata, rather than
comparing a historical expectation to the current binary version. The settlement
upgrade fixture passes only `MIGRATIONS[..39]` with supported version39.
Migration SQL, production startup and corruption/old-reader checks are unchanged.

Disposable PostgreSQL was started/reused with `scripts/test-db`; each `TestDb`
creates and drops its own database. The installed `aq test` supports pytest/Bazel,
so the requested Rust checks use bounded Cargo directly:

```sh
export DATABASE_URL=postgres://postgres:test@127.0.0.1:55432/postgres
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 RUST_TEST_THREADS=2
cargo test --locked --offline -p aq-server --lib agent_access::messaging::global::reservations::tests:: -- --test-threads=2
cargo test --locked --offline -p aq-server --lib store:: -- --test-threads=2
```

Results: **12 reservation tests and 25 Store tests passed**, with no failures or
ignores. These cover all eleven reported failures plus related schema, owner,
old-reader and corruption invariants. Complete output is in `reservations.log`
and `store.log`. The full workspace suite was not rerun.

`git diff --check` passed. Direct rustfmt checks passed for the four changed
fixture files. A check of `store.rs` reports only existing migration registry
formatting and module ordering; normalized findings exactly match the unmodified
source-base file. `cargo fmt --all -- --check` cannot discover the vendored
`aq-lifecycle-protocol-0.1.0` workspace from this nested worktree.
