# Development

## Prerequisites

- Rust 1.96.1 (pinned by `rust-toolchain.toml`; rustup installs it automatically).
- Docker with the Compose plugin. Under WSL2, use Docker Desktop's WSL integration.
- `python3` and `curl` (used by `scripts/smoke`).

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
cargo run -p aq-worker -- enroll --server http://127.0.0.1:8081 --name "$(hostname)"
```

Paste the returned `code` when prompted. Run `scripts/dev-worker` again to
connect and heartbeat. `GET /api/workers` shows the worker as `online`.

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

Builds the server and worker images, starts a fresh stack under the separate
Compose project `agent-q-smoke` (so your dev data is untouched), enrolls the
worker image, confirms a heartbeat, restarts the server and database, confirms
the same node heartbeats again, and tears everything down with `down -v`. It
ends with `smoke: PASS` or names the failed step. It needs ports 8080 and 8081
free, so stop `scripts/dev-up` first.

## Production notes

`compose.yaml` is the production shape: no published ports, a read-only vault
mount, and secrets from `$AQ_SECRETS_DIR` (`postgres_password`, `tunnel_token`).
Compose bind-mounts secret files with their host permissions, so they must be
readable by the container users (the server runs as UID 10001). The server
entrypoint exports the database password as `PGPASSWORD` from the secret file
and uses a password-less `DATABASE_URL`, so the password needs no URL encoding.
The image sets `AQ_VAULT_DIR=/vault`; `aq-server healthcheck` reads the vault to
find the admin port.
