# aq-server agent guide

Development setup, the stack and the test database are described in
[`docs/development.md`](docs/development.md). AQ task sessions also get the
project notes in [`.aq/PRIME.md`](.aq/PRIME.md).

## Build health

These are owner requirements (2026-11-05). By November 2026 builds had become
the bottleneck: an edit followed by one test rerun took 127 s, CI ran 269 test
binaries one after another, and the update lifecycle job took 36 min. No build
budget existed, so each change that made things a little slower was locally
reasonable (retrospective W72). The rules below keep that from happening
again. Follow them in interactive sessions and in AQ task sessions.

### Interactive builds

- **Incremental.** The dev and test profiles are incremental (`Cargo.toml`).
  Rebuilding one aq-server test binary after an edit takes about 16 s, against
  127 s without incremental. Do not export `CARGO_INCREMENTAL=0` in an
  interactive session. Switching between incremental and non-incremental
  builds rebuilds the workspace crates, so stay on one setting per target
  directory. CI and one-shot builds set `CARGO_INCREMENTAL=0` themselves.
- **Disk cost.** `target/debug/incremental` reaches about 2.5 GB after one
  test binary and about 12 GB after all of them. When disk is short, delete
  your own `target/debug/incremental`. Never touch another worktree's target.
- **Private target directory per worktree.** Each worktree builds into its
  own `CARGO_TARGET_DIR` (the default `./target`, or a path only that worktree
  uses). Never share a target directory across worktrees or branches. Worktrees
  produce identical artifact hashes and cargo judges freshness by mtime, so a
  shared target links another tree's code into your tests and their green
  result means nothing (retrospective W67).
- **Do not alternate `cargo build` and `cargo test` without a reason.**
  aq-server is its own dev-dependency with `test-support` enabled, so
  `cargo build` and `cargo test` compile two different aq-server libraries,
  each a full build of the largest crate. Iterate with `cargo nextest run` or
  `cargo test` for the test binary you are working on. Run `cargo build` only
  when you need the server binary itself, and `cargo clippy` once at the end.
- **Test one binary at a time while iterating:**
  `timeout 900 cargo nextest run -p aq-server --test <binary> [filter]`.
  Run the full suite (`cargo nextest run --workspace --locked --no-fail-fast`)
  only when the task needs it. See [`docs/development.md`](docs/development.md#tests).
- **Every `cargo test` or `cargo nextest` in an agent session runs under a
  timeout**, for example `timeout 900 cargo nextest run ...`, so a hung test
  ends the command instead of the session. nextest also stops any single test
  that runs longer than `.config/nextest.toml` allows.

### Tests

- **New integration tests go into an existing test binary.** Add a module or
  a test function to the existing `tests/*.rs` binary for that area. Do not add
  a new `tests/*.rs` file: each one is another executable that links the whole
  aq-server library and all its dependencies. CI rejects an increase in the
  number of test binaries (see the budgets below). A follow-up PR consolidates
  the existing binaries into a few groups; until then, pick the closest
  existing binary.
- **Process-wide serialization needs a nextest test group.** CI runs each test
  in its own process, so a `static` mutex in `tests/common` serializes nothing
  across tests. If tests must not run concurrently, add them to a test group in
  `.config/nextest.toml`. An in-process gate is still needed for plain
  `cargo test`. Two groups exist. `selection-fixture` holds every test that
  builds a selection fixture; the fixture gate panics under nextest for a test
  outside it, so a new fixture test must be added to its filter.
  `fresh-migrations` holds the `store::*_tests` unit tests. Fresh test-database
  migrations are also serialized across processes by an advisory lock
  (`test_support::migration_slot`), because concurrent fresh migrations
  overflow PostgreSQL's shared lock table (error 53200).
- **Doctests** are not run by nextest. CI runs `cargo test --workspace --doc`.

### Budgets

`build-budget.toml` holds ratchets, and `scripts/build-budget.py` (CI lint job)
fails when a number goes above its recorded value:

| Budget | Recorded 2026-11-05 | Rule |
| --- | --- | --- |
| Integration test binaries in the workspace | 204 | May only go down. |
| Lines in one crate's lib (`src/`, minus bin targets) | target 40,000 | No crate lib may grow past the target. |
| aq-server lib lines | 109,000 (ratchet) | Over the target while it is split into crates. May only go down. |

- Check locally with `python3 scripts/build-budget.py`.
- **When a number goes down**, lower the recorded value in the same PR. Run
  `python3 scripts/build-budget.py --update`, which only ever lowers values.
  CI fails if the test binary count is below its recorded value, so a removed
  binary cannot be quietly replaced by a new one later. Line ratchets print a
  note instead.
- **Raising a number** needs an explicit edit of `build-budget.toml` in its own
  commit, a reason in the commit message, and the owner's review. Prefer
  moving code into another crate or tests into an existing binary.

### CI wall time

| Job | Budget |
| --- | --- |
| `tests` | 10 min |
| `fmt and clippy` | 10 min |
| Each image or smoke job (`smoke`, `execution-smoke`, `recovery-smoke`, `update-lifecycle`) | 15 min |
| Whole workflow (critical path, push to last green job) | 20 min |

`timeout-minutes` in `ci.yml` is a hang guard, not the budget. A job that goes
over budget on main is a defect, not a fact of life. Open a task the same day
with the measured times from the run and the cause. Until the job is back under
budget, do not add work to it. Fix it with caching, parallelism (nextest test
groups, splitting a job), consolidating test binaries or a cheaper fixture, not
by raising the budget. Raising a budget needs the owner's review, as for
`build-budget.toml`.

### Docker images

- **No shared cargo target cache without the touch step.** A Dockerfile whose
  `RUN --mount=type=cache,...,target=/build/target` cache is shared across
  trees (the default cache id is shared by every branch on the host) must run
  `find crates vendor protocol -type f -exec touch {} +` in the same `RUN` as
  `cargo build`. The alternative is a cache id unique to the exact source, as
  `Dockerfile.worker` keys its cache by the verified archive digest. Without
  either, cargo's mtime check reuses another branch's build. 458f682d hit this,
  and `cargo clean -p` of the workspace crates missed the `vendor/` path crates.
- **Release-only settings stay out of CI images.** ThinLTO lives only in
  `[profile.release]`. CI, smoke and test images build with
  `--build-arg CARGO_PROFILE=ci-image`, which is release without LTO. The
  smoke and lifecycle scripts do this by default (`AQ_IMAGE_CARGO_PROFILE`
  overrides it). `release.yml` builds `release`. A new CI-only image build must
  pass the CI profile too.
