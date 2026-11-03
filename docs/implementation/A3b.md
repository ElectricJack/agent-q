# A3b: aq-git v0.2.0 consumer pin

Status: release pin and named consumer qualifications pass. The server and worker
images contain the exact released aq-git executable. Locked workspace acceptance
remains blocked by a pre-existing historical reservation fixture failure.

## Inputs and implementation

The published [v0.2.0 release](https://github.com/ElectricJack/aq-git/releases/tag/v0.2.0)
points at `e75f7b4bc36a83ae5001fefa0d5f87618e87ad4a`. GitHub's release asset
metadata independently confirms the retained bytes:

| Input | SHA-256 |
| --- | --- |
| Release source archive | `f64891f03e2d28263cf3af20b38123935ae08d6917699becb9d77e76af2fd3c6` |
| Linux x86_64 executable | `3feca63cffa5065566f2c604fd00d4ad92e1bc367c62ec69bedd42c2b37f35d2` |
| A3a build record | `94e318a2285369002c1df8ea9844ca0295ce3b867fa35691d201cc77f1c7852f` |
| Retained Git bundle | `217d925d105b12a1fcd40b02a4a0a24b0c3cfc31a98cdc46d72918a94debd084` |
| Unchanged upstream Git 2.56.0 archive | `26c56c296b38c0695b26fa95f475f1d01704d2d38e73465ca30b0b2f5dc789d3` |

The first three files are byte-preserved copies of A3a's downloaded assets.
The bundle was created with `git bundle create <vendor/aq-git-source.bundle> HEAD`
in A3a's clean detached release checkout. Its HEAD is the published commit;
verification clones the bundle into temporary bare storage and compares every
committed source blob with the archive inventory. The consumer never reads a
sibling checkout or fetches a floating source revision.

`vendor/git-candidate.json` records release, executable, build-record and inventory
paths plus digests. `vendor/git-candidate.sha256` covers archive, bundle,
executable, build record and upstream Git. The new versioned vendor directory
preserves upstream Git and its checksum provenance, and inventories all 366
release source files. The checker validates archive safety before reading
members, embedded commit, locked package version, build record provenance,
source inventory and retained bundle. Official overrides remain refused.

Both application Dockerfiles embed the canonical aq-git stages byte-for-byte.
They verify the release archive and executable with `sha256sum -c`, then copy
the executable's exact bytes. Copying the released executable meets the consumer
release pin without requiring a reproducible Rust rebuild. Both stages preserve
`rust:1.96.1-bookworm@sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663`.
Non-aq-git base stages, state labels and Compose supervision are untouched.

The project-binding wrapper selects the newly built immutable image ID and runs
its named Rust qualification with `--locked`. The existing fixture-derived
checkout selection is preserved. `docs/source-retention.md` records the new
commit, binary digest and release-copy choice.

## Environment and commands

See [environment.json](A3b-checks/environment.json). Commands were run in the
assigned task worktree with Rust/Cargo 1.96.1, Python 3.12, Docker BuildKit and
disposable PostgreSQL 17. Rust commands use `CARGO_BUILD_JOBS=2` and
`RUST_TEST_THREADS=1`; Python behavior tests use AQ's unchanged worker defaults.
`DATABASE_URL` for database tests selects only the disposable database started by
`scripts/test-db`.

| Command | Observed result |
| --- | --- |
| `aq test scripts/test-git-candidate.py` before implementation | 5 failed, 3 passed: missing executable/provenance/path validation and old release pin; [red log](A3b-checks/verifier-red.log) |
| `aq test scripts/test-git-candidate.py` after implementation | 10 passed, including release acceptance, executable tampering, provenance tampering, unsafe paths, archive traversal, inventory mismatch, bundle/source disagreement, official overrides and Docker drift; [post-repair green log](A3b-checks/verifier-after-repair.log) |
| `python3 scripts/check-git-candidate.py --official` | Passed; [verified manifest](A3b-checks/candidate-after-repair.json) |
| `gh api repos/ElectricJack/aq-git/git/ref/tags/v0.2.0` | Published tag resolves to retained commit; [tag receipt](A3b-checks/release-tag.json) |
| `gh api repos/ElectricJack/aq-git/releases/tags/v0.2.0` with asset name/digest projection | Published source/binary/build-record digests match; [asset receipt](A3b-checks/published-assets.json) |
| `DOCKER_BUILDKIT=1 docker build --progress=plain -f scripts/Dockerfile.aq-git -t aq-git-a3b:0.2.0 .` | Passed; [build log](A3b-checks/build-aq-git.log) |
| `DOCKER_BUILDKIT=1 docker build --progress=plain -t aq-server-a3b:0.2.0 .` | Passed after prerequisite repair; [server build log](A3b-checks/build-server-after-repair.log) |
| `docker build --progress=plain -f Dockerfile.worker --build-arg AQ_SOURCE_REVISION=3fb2bcb2f63f73838ddd97e44580fe721a539bdd --build-arg AQ_WORKER_ARCHIVE_SHA256=ed33076d62d37884cab04f1206d1fda3dd5e06181ddb1ccdebec58fda789b535 --build-arg AQ_VERSION=0.3.1 -t aq-worker-a3b:0.2.0 .` | Passed; [worker build log](A3b-checks/build-worker.log) |
| `docker build --target aq-git-build` with a deliberately corrupted retained executable in a disposable context | Correctly refused by `sha256sum -c`; [negative build log](A3b-checks/corrupt-binary-build.log) |
| `cargo fmt --check` | Passed; [format log](A3b-checks/fmt-after-repair.log) |
| `cargo clippy --locked --all-targets -- -D warnings` | Initial post-repair run exposed a duplicate attribute in phase_never_granted.rs:4/6; eager-grove-45.3.2 removed it. Final rerun active; [initial lint failure](A3b-checks/clippy-after-repair.log) and [rerun](A3b-checks/clippy-after-lint-repair.log) |
| `cargo test --locked --workspace` | Failed: 349 tests passed in earlier completed suites; aq-server lib 172 passed, 9 failed, 1 ignored. All failures stop in the same historical reservation fixture seed; [workspace log](A3b-checks/workspace-tests-after-repair.log) |
| `cargo test --locked -p aq-server --test source_retention --test project_bindings` | Passed: project_bindings 12 passed/1 ignored; source_retention 10 passed/1 ignored. Ignored cases run separately; [focused log](A3b-checks/focused-rust-after-repair.log) |
| `AQ_TEST_GIT_CANDIDATE=/tmp/aq-a3b-qualification/aq-git AQ_TEST_GIT_EXECUTABLE=/tmp/aq-a3b-qualification/aq-git-runtime/bin/git cargo test --locked -p aq-server --test source_retention selected_real_aq_git_import_survives_worker_volume_loss_and_base_download_is_scoped -- --ignored --nocapture` | Passed: the named ignored test ran, 1 passed; [qualification log](A3b-checks/source-retention-after-repair.log) |
| `python3 scripts/test-project-bindings.py` | Passed: selected_real_public_repository_and_graph_workspace_bind ran, 1 passed; pinned graph guards also passed and synthetic resources cleaned up; [wrapper log](A3b-checks/project-bindings-after-repair.log) |

## Embedded executable checks

For each successful image, `docker run --rm --entrypoint sha256sum IMAGE
/usr/local/bin/aq-git` equals the release binary digest. Separate invocations
of `/usr/local/bin/aq-git --version` and `/opt/aq-git-runtime/bin/git --version`
report `aq-git 0.2.0` and `git version 2.56.0`.

| Image | Immutable image ID | Embedded aq-git SHA-256 |
| --- | --- | --- |
| Standalone `aq-git-a3b:0.2.0` | `sha256:4e405e60f637f766582fcb182d1e3bd963ba8e78c94d2cc57203d28f0f358642` | `3feca63cffa5065566f2c604fd00d4ad92e1bc367c62ec69bedd42c2b37f35d2` |
| Worker `aq-worker-a3b:0.2.0` | `sha256:bfb1309ad2f7fc1765ff0526b0b568882b4611f05981c5c9841120375d08fbf1` | `3feca63cffa5065566f2c604fd00d4ad92e1bc367c62ec69bedd42c2b37f35d2` |
| Server `aq-server-a3b:0.2.0` | `sha256:2dc0cb33fdc8e960a08204d6dadf34162b92eade587c268d0ee9d4c3a46163f8` | `3feca63cffa5065566f2c604fd00d4ad92e1bc367c62ec69bedd42c2b37f35d2` |

Machine-readable results: [image-digests.json](A3b-checks/image-digests.json).

## Prerequisite integration and limits

Initial attempts at source base `ab314605` failed with duplicate DeliveryClock
methods in aq-admission (E0592/E0034). Required child `eager-grove-45.3.1`
reconciled those implementations in `d6ed24319745d8030e71c526b498b83f5bdde5e0`.
The authorized `aq git pull --branch main --json` integrated that repair without
conflicts in merge `912155a13392ab4a3176d2202acf9355b5170d1c`. Post-repair Rust commands and the server build use that merged source. The initial failure
logs remain in `A3b-checks/`; the `*-after-repair` logs record current results.

Strict Clippy then exposed a pre-existing duplicate `allow(dead_code)` attribute
on the same terminal fixture module in `phase_never_granted.rs:4/6`. Follow-up
`eager-grove-45.3.2` removed the redundant line without changing assertions or
fixture behavior. Fetched main `c13cf4bf` was merged without conflicts as
`1cf77fc1`; that integration also retains other concurrently landed main changes.
The pinned aq-git build blocks and retained release inputs remain unchanged.

Workspace tests then failed in nine global mail-reservation unit tests, all at
`src/agent_access/messaging/global/reservations/tests.rs:99`. Their fixture
explicitly uses registered schema37, but its seed calls `writes::enqueue`,
which now delegates to the dashboard action journal. The journal queries
`dashboard_operations`, introduced by migration0042. The returned error is
`IdentityUnavailable("dashboard operation unavailable or conflicting")`.
Required child `eager-grove-45.3.3` owns reconciliation of that historical
fixture with action routing. No passing workspace result is claimed.

Supervisor message `msg-c6f2114e7cd64deeb8ab1aed23c51a9c` explicitly waives
the completion profile's missing `origin/dev` ref: fetch and merge `origin/main`,
never create `dev` or push `main`. That synchronization was performed through
AQ's guarded git command.

A3b is not marked for adversarial review. The executable is Linux x86_64 and its
recorded maximum GLIBC requirement is 2.34. No new hosted deployment, performance,
host-power-loss or exhaustive compatibility qualification is claimed. The retained
A3a build record intentionally preserves its original preparation-time status.

The implementation plan was committed before code as `32ff7432`; the
unreviewed feature history was then consolidated before final integration.
The consolidated implementation commit is `0229a53f771cec234198ac38ecdc6ed846692aa2`.
The prerequisite merge preserves its ancestry. Final evidence is committed after
the merge; its exact SHA is recorded in the AQ task comment and guarded push
receipt. No default-branch delivery is claimed by a worker push.
