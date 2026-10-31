# BASE-01 — foundation qualification

Status: complete — local foundation baseline qualified; no foundation runtime fixes required.

Selected source: `eed842362f29016ee508ca200f3bb3da2473dc08` (merged foundation release lifecycle). Qualification uses a `git archive` copy, excluding concurrent uncommitted v2 documentation and implementation. The source archive SHA-256 is `95a2790c8361b3084d3907604813516520c92180a620b68e1fab8556fb4a7c3a`; `Cargo.lock` SHA-256 is `8de30ac6c3bad57cafdd97889111e4c6acdd004c46dd71b2f00da06afc20b04f`.

## Scope and Phase 1 reconciliation

The baseline already implements PostgreSQL-backed enrollment/identity, separate administration and worker listeners, heartbeats/probes, credential revocation, signed format-1 releases, durable updater recovery, maintenance admission and release readiness. These working foundations are preserved; execution grants, scheduling, agent processes, graph completion and the composite product updater are subsequent work.

The old local foundation and release lifecycle plans contain unchecked implementation steps despite merged source and executable checks. Their expected outcomes are design/history, not current test evidence. This qualification reruns the existing checks against the selected source and retains the outcomes. It does not reinterpret those checkboxes as either proof of a pass or a requirement to rebuild working features.

## Reproduction and isolation

The host is Linux x86_64 on WSL2, Rust/Cargo 1.96.1, Docker Engine 29.3.1 (Docker Desktop 4.67.0), Compose 5.1.1. The baseline is archived under `/tmp/aq-base01-eed8423-xrzw4_qo/source`, with its own Cargo target directory. Rust tests use an exclusively created PostgreSQL 17 container `aq-base01-xrzw4qo-postgres` on a Docker-assigned loopback port; each server test creates/drops its own UUID database inside that container.

Upstream smoke/update scripts hardcode shared project/resource names. The qualification changes only the archived harness: unique `aq-base01-xrzw4qo-*` projects, images and registry; dynamically assigned smoke listeners; isolated development secret/spool paths; evidence capture before teardown. The retained isolation patch records every harness change. No existing container, volume or database is reused or removed.

The first smoke attempt failed before enrollment because Docker Desktop rejected an OS-selected free port. The second enrolled successfully but its external poll used the old dynamic host port after Docker reallocated it at restart. The final harness refreshes the mapped admin address after restart. Both failed harness attempts are retained and excluded from the passing baseline result.

## Compatibility retained for extraction

[`BASE-01.fixtures.json`](BASE-01.fixtures.json) retains synthetic examples for enrollment, heartbeat, probe, revocation, unsupported protocol, saved `worker.toml`, release manifest, updater journal/request/status. Supplemental checker sources parse these examples against the archived foundation types, round-trip saved node identity, reject journal format 2 and verify status tolerance of future fields/variants. They run separately from the original suite; they do not modify foundation runtime code.

- Foundation wire protocol is version 1, accepted range 1–1, with `POST /v1/worker/enroll`, `/heartbeat` and `/probe`. It offers no execution authority. Unsupported protocol returns HTTP 426 with the supported range; incompatible enrollment leaves the code usable.
- Node identity remains `server_url`, `node_id`, `credential`, optional `cf_client_id`/`cf_client_secret`, in `$XDG_CONFIG_HOME/agent-q/worker.toml` (fallback `$HOME/.config/agent-q/worker.toml`). Parsing rejects unknown fields and insecure file permissions; saves use a private temporary file, file sync and rename.
- Revoked credentials return HTTP 401 `credential_revoked` for heartbeat and probe, do not advance `last_seen_at`, and terminate the connector loop. Network failure, HTTP 408/429 and 5xx remain retryable; redirects/HTML indicate Access rejection.
- Server state, manifest and updater journal are independently versioned, all currently format 1. The updater refuses incompatible state before pull, rejects unknown private journal fields/formats, and reads public status tolerantly for rollback compatibility. Future product manifest work must introduce a separate format rather than reinterpret format 1.

## Current results

| Check | Current result |
| --- | --- |
| `cargo fmt --all --check` | Pass |
| `cargo test --workspace --locked` with isolated PostgreSQL | 237 passed, 0 failed/ignored |
| `cargo test -p aq-updater --locked --features test-verifier` | 137 passed, 0 failed/ignored; overlaps the workspace suite |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo clippy -p aq-updater --all-targets --locked --features test-verifier -- -D warnings` | Pass |
| `cargo build --release --locked -p aq-updater --target x86_64-unknown-linux-musl` | Pass; resulting ELF is static PIE |
| Isolated `scripts/smoke` | Pass; same node before/after daemon and PostgreSQL restart |
| Supplemental fixture checks (`cargo test --locked -p aq-worker -p aq-updater --test base01_fixtures`) | 3 passed |
| PostgreSQL-only `Cargo.lock` check | Pass; neither SQLite crate is present |
| Isolated `scripts/update-lifecycle` | Pass; all seven cases, fixed runner SHA-256 recorded |

Rust checks use `CARGO_BUILD_JOBS=4` and an unset `AQ_SOURCE_REVISION`; the recorded command runner includes the exact test environment. Supplemental checkers are copied into the archived crates only for their own run, then removed.

The lifecycle was repeated with a private executable copy because a concurrent supplemental Cargo invocation briefly replaced `target/debug/aq-updater` with a different feature build. Both attempts passed all seven cases, but only the final fixed-binary run closes acceptance. Its runner SHA-256 is `97516d7aefd86c3333f545c8ecb0647fe3be507d89872f348e89d2096bf45fe4`.

## Retained evidence

[`BASE-01.evidence.json`](BASE-01.evidence.json) records exact source/lock/toolchain hashes, commands, UTC timings, counts, image IDs and registry digest identities. [`BASE-01.logs.tar.gz`](BASE-01.logs.tar.gz) retains original stdout, both failed smoke harness attempts, both lifecycle attempts, final public release manifests/signatures, updater journal/status and generated versioned Cargo inputs. [`BASE-01.isolation.patch`](BASE-01.isolation.patch) records the complete harness adaptation. No signing private key, live credential or vault is retained.

The static x86_64 musl updater SHA-256 is `491971464932b5309a26027f6efd5ad7a482685bd6b36de50c53f74da7e70c9c`. The smoke server image ID is `sha256:fee25a4384f1f627f3ed7a68b3367cd44d1959171cbb1c12e9e619dc70729e04`; all worker and lifecycle images are enumerated in the machine evidence. Local image IDs and local-registry digests are artifact identities, not published release references. Synthetic `suite-VERSION` labels used by update tests map to the archived baseline and explicitly recorded workspace-version changes.

BASE-01 acceptance is met: the selected existing foundation was qualified before extraction, applicable CI layers ran locally, connector/update formats have retained fixtures, and hosted limits remain explicit. Disposable containers, volumes, networks and unique smoke image tags were removed after evidence capture.

## Acceptance limits

This is local foundation qualification for BASE-01, not hosted/product acceptance. The image smoke uses development authentication. The update suite uses a production-shaped vault and real PostgreSQL/OCI images, but a test signing key and no real Cloudflare login. It exercises actual install/upgrade/rollback/interrupted-probe recovery; unit tests exercise the remaining updater crash boundaries with a fake runtime. Process interruption does not prove host power-loss durability.

No VPS, ingress/access policy, external identity tenant, off-host encrypted restore, real release-signing authority, distributed execution, capacity profile or product gate is certified. There is no prior supported foundation protocol to test; E4/D8/W16 coverage here is preserved version-1 connectivity and rejection semantics. Their full v2 requirements remain owned by later implementation/gates. E5 coverage here is the foundation updater, not unimplemented component migrations.
