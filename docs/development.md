# Development

## Prerequisites

- Rust 1.96.1 (pinned by `rust-toolchain.toml`; rustup installs it automatically).
- Docker with the Compose plugin. Under WSL2, use Docker Desktop's WSL integration.
- Python 3.12 or newer, and `curl` (used by the integration/update scripts).

## Run the stack

```sh
scripts/dev-up
```

This creates `secrets.dev/postgres_password` if it is missing (random contents,
gitignored), then builds and starts PostgreSQL and the server from source using
`compose.yaml` plus `compose.dev.yaml`. The admin listener is on
`http://127.0.0.1:8080` and the worker listener on `http://127.0.0.1:8081`,
both bound to loopback only. The vault is `vault.example/`, which enables dev
auth with the owner `owner@localhost`.

The PostgreSQL data lives in the `aq_postgres` Docker volume and persists
between `scripts/dev-up` runs. To reset it (this deletes all dev data):

```sh
docker compose -f compose.yaml -f compose.dev.yaml down -v
```

PostgreSQL only reads the password when it first initializes the volume. If you
delete or regenerate `secrets.dev/postgres_password` while an old volume exists,
the server fails with password authentication errors until you reset the volume
as above.

Cloudflared is disabled in development (it sits behind the `tunnel` profile).

## Run a worker from source

```sh
scripts/dev-worker
```

With no worker config yet, it prints how to enroll:

```sh
curl -fsS -X POST -H 'X-AQ-Request: 1' http://127.0.0.1:8080/api/enrollments
scripts/dev-worker enroll --server http://127.0.0.1:8081 --name "$(hostname)"
```

Paste the returned `code` when prompted. Run `scripts/dev-worker` again to
connect and heartbeat. `GET /api/workers` shows the worker as `online`.

The command verifies and extracts the immutable source candidate described by
`vendor/worker-candidate.json`, then runs its independent Cargo workspace. It
does not read a sibling checkout. `AQ_WORKER_BIN=/absolute/path/to/aq-worker`
explicitly selects a development executable; the command prints that override.
Official artifact checks reject local executable/source overrides.

## Simulating another user

Dev auth treats every request as the vault's `dev_email`. To act as a viewer
(or any other user), send the `X-AQ-Dev-Email` header:

```sh
curl -H 'X-AQ-Dev-Email: viewer@example.com' http://127.0.0.1:8080/api/me
```

Dev auth is refused in production: the server will not start with
`environment = "production"` and `auth.mode = "dev"`. In addition,
`compose.yaml` sets `AQ_DEPLOYMENT=production` on the server. With that
variable set, the server exits with status 2 unless the vault has
`environment = "production"` and a non-dev `auth.mode`, so a development vault
such as `vault.example/` cannot be mounted into the production stack by
mistake. `compose.dev.yaml` overrides it to `development`.

## Tests

```sh
eval "$(scripts/test-db)"   # disposable PostgreSQL on 127.0.0.1:55432
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## Smoke test

```sh
scripts/smoke
```

Builds the daemon and pinned standalone worker from a frozen source snapshot,
then starts a uniquely named disposable Compose project with Docker-assigned
loopback ports. It enrolls the worker, verifies heartbeat/probe/protocol
rejection, restarts the daemon and database, checks the same saved node identity
reconnects, and revokes the credential. The worker must exit with code 3.
Existing development services can remain running.

`AQ_SMOKE_SERVER_IMAGE` and `AQ_SMOKE_WORKER_IMAGE` select a prebuilt pair.
`AQ_SMOKE_OFFICIAL=1` requires both to be digest references and rejects local
worker overrides. The worker's embedded source manifest must match the selected
candidate. `AQ_SMOKE_EVIDENCE_DIR` selects the retained log/identity directory;
otherwise its temporary path is printed. Disposable resources are removed.

```sh
scripts/execution-smoke
```

This uses the same assembly and additionally runs the real standalone runtime
with a deterministic child. It cancels an offer before acquisition, executes a
different offer, loses acceptance/report responses, checks one launch and a
durable accepted result, replays the report, and restarts the daemon. Synthetic
assignment creation is development-only. This fixture does not certify graph
completion, real model harnesses, hosted access or host-power-loss recovery.

```sh
scripts/recovery-smoke
```

The 20-case recovery matrix builds the pinned worker and a separate daemon image
with `recovery-test-support`. It injects launch-boundary crashes, transport loss,
expiry, cancellation races, database lock waits and bulk saturation through
observable barriers. A historical worker also attempts to read journal format 2.
`AQ_RECOVERY_EVIDENCE_DIR` selects retained evidence; owned containers and volumes
are removed. The production daemon image excludes these clock/load controls.
This qualifies the cooperative Linux guardian profile for runtime-process
restart. Guardian loss retains uncertainty; whole-container reattachment and
physical suspend/power-loss remain separate work.

Run the additional daemon regressions with
`cargo test --workspace --features recovery-test-support --locked` against the
disposable test database. The [agent graph contract](../protocol/agent-access/v1/README.md)
has a separate offline conformance suite with pinned developer dependencies.
CI runs the contract suite and container recovery matrix as separate jobs, and
keeps the ordinary daemon tests alongside the recovery-feature regressions.

## Update lifecycle suite

```sh
scripts/update-lifecycle
```

Proves the host updater against real images, a real registry, the production
`compose.yaml` and real PostgreSQL. It builds `aq-updater` with the
`test-verifier` feature in a private target directory, starts a uniquely named
throwaway `registry:2` on a Docker-assigned loopback port, and builds and pushes server
images for 0.1.0, 0.1.1 and a broken 0.1.2, each with a manifest signed by a
throwaway test key and its `release` image. Under the Compose project
a uniquely named disposable project with a production-shaped vault it checks, through
`aq-updater status` and `aq-server readiness`: a first install, an upgrade, a
rollback of the broken release, recovery after `kill -9` mid-update, refusal of
a release that cannot read the installed state format, refusal of a downgrade
requested through the spool, and that the database survives all of it.
It also starts from a fresh host: the updater's first `run --once` creates the
spool and a `release.env` with a placeholder image, which is what lets Compose
start postgres before any release is installed. The independently pinned worker
candidate remains unchanged across these synthetic server versions. A future
state-format refusal always tests the current format plus one. Disposable
resources are removed on exit.
It ends with `update-lifecycle: PASS` or names the failed step; on failure its
logs are copied to a kept `${TMPDIR:-/tmp}/aq-update-lifecycle-logs.*`
directory, whose path it prints. Public manifests, runner/image identities and
logs are also retained on success (`AQ_UPDATE_EVIDENCE_DIR` selects the destination).
The first run compiles all dependencies;
later runs reuse BuildKit cache mounts.

## Releases and operations

[`docs/operations.md`](operations.md) covers the CI, release and promote
workflows, cutting a release, and installing and running the host updater
(`aq-updater`), including what to do when an update needs an operator.

`scripts/smoke` can also test prebuilt images instead of building: set both
`AQ_SMOKE_SERVER_IMAGE` and `AQ_SMOKE_WORKER_IMAGE` (tags or digest
references; missing ones are pulled). The release workflow uses this to smoke
test the exact pushed digests.

## Production notes

`compose.yaml` is the production shape: no published ports, a read-only vault
mount, the updater spool from `$AQ_UPDATER_SPOOL` (`requests/` read-write at
`/updater/requests`, `status/` read-only at `/updater/status`), and secrets from
`$AQ_SECRETS_DIR` (`postgres_password`, `tunnel_token`). To check it:

```sh
AQ_SERVER_IMAGE=example/agent-q-server@sha256:$(printf '0%.0s' {1..64}) \
AQ_VAULT_DIR=/tmp/vault AQ_SECRETS_DIR=/tmp/secrets AQ_UPDATER_SPOOL=/tmp/spool \
  docker compose -f compose.yaml config
```

Compose bind-mounts secret files with their host permissions, so they must be
readable by the container users (the server runs as UID 10001). The server
entrypoint exports the database password as `PGPASSWORD` from the secret file
and uses a password-less `DATABASE_URL`, so the password needs no URL encoding.
The image sets `AQ_VAULT_DIR=/vault`; `aq-server healthcheck` reads the vault to
find the admin port.
