# Local Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the locally runnable Agent Q foundation: a server with identity, roles, worker enrollment, and heartbeats, exposed as a JSON API; a worker connector; and a Compose stack. No admin page or dashboard: the admin page is deferred to a follow-on plan, and the API is the only human interface in this plan. All of it is testable without Bluehost or Cloudflare accounts.

**Architecture:** A Cargo workspace with three crates: `aq-protocol` (wire types and protocol version), `aq-server` (axum, PostgreSQL foundation store) and `aq-worker` (reqwest client). The server binds two listeners. The **admin listener** serves human routes behind an `IdentityVerifier` adapter (Cloudflare Access JWT in production, a dev substitute locally). The **worker listener** serves only machine routes. Origin isolation therefore holds by construction: cloudflared will map each hostname to one port. Foundation state lives in a dedicated PostgreSQL database (`agent_q`), run as its own Compose service. Editable non-secret configuration lives in a separately mounted vault directory.

**Tech Stack:** Rust 1.96.1, edition 2024; axum 0.8, tokio 1, tower 0.5, sqlx 0.8 (postgres, runtime-tokio-rustls, uuid, chrono — no `migrate` or `macros` features, because `migrate` pulls `sqlx-sqlite`/`libsqlite3-sys` into Cargo.lock; runtime `sqlx::query`/`query_as` only, no compile-time `query!` macros, so builds need no database), jsonwebtoken 9, reqwest 0.12 (rustls, json), serde/serde_json, toml 0.8, clap 4 (derive, env), chrono, uuid v4, sha2, hex, rand 0.8, base64 0.22, thiserror 2, tracing, rpassword 7. Dev: tempfile, http-body-util, rsa 0.9. Docker Compose v2, PostgreSQL 17 (`postgres:17-bookworm`).

**Spec:** `docs/specs/0001-infrastructure-foundation.md`. This plan covers spec increment 1 (local foundation) plus the locally testable half of increment 2: the identity adapter, owner bootstrap, roles, and enrollment. Follow-on plans are listed at the end.

## Global Constraints

- Language decision (resolves the spec's open "implementation language for the shell and connector"): Rust, matching `aq-graph` (axum 0.8, edition 2024, `rust:1.96.1-bookworm` build image, `debian:bookworm-slim` runtime, non-root UID 10001).
- Validate the signed `Cf-Access-Jwt-Assertion` using configured issuer, audience, expiration, and signing keys. Never trust an email header alone.
- The first visitor is never automatically the owner. Bootstrap requires both the configured verified email and an explicit one-time bootstrap configuration.
- Roles: Owner (manage enrollment, revocation, configuration, roles, updates; inspect status) and Viewer (read status and version; no mutations or secret access). Enforce them on every server operation, including direct API requests.
- Protect browser mutations against forged cross-origin requests.
- Human administration routes must not be reachable through the worker hostname/listener.
- Enrollment codes are short-lived and single-use. The server stores only a non-reusable verifier (SHA-256) of opaque credentials, never the credential itself.
- Worker secrets are stored with mode 0600. Setup must not expose them in shell history, process arguments, or logs.
- Heartbeat interval is 15 seconds. A worker is offline after 60 seconds without a heartbeat. Offline status never deletes identity.
- Reconnection uses bounded backoff with jitter.
- The handshake includes worker identity, software version, and protocol version. Unsupported versions get an actionable compatibility response.
- Any local authentication substitute cannot be enabled in the production configuration.
- The application container never receives the Docker socket. It publishes no host port in the production Compose file.
- Log rotation is configured from the first deployment: json-file, `max-size: 10m`, `max-file: "3"`.
- PostgreSQL is the only database. No SQLite anywhere: no `sqlite` sqlx feature, no `libsqlite3-sys`, no `.sqlite` files. Verify with `grep -E 'name = "(libsqlite3-sys|sqlx-sqlite)"' Cargo.lock` → no output. Migrations use a small in-house runner (Task 3), and test databases come from `aq_server::test_support::TestDb`, not `#[sqlx::test]`. This overrides the spec's provisional SQLite default.
- The database URL contains a secret. It comes from the `DATABASE_URL` environment variable, which the container entrypoint builds from a Compose secret file (the `aq-graph` pattern). It never goes in the vault or in Compose config.
- Credentials never go in version control. `.gitignore` already excludes `.env`, `secrets/`, `state/`, `vault.local/`.

## Review Focus

1. **Clock skew between Cloudflare and the VPS.** A token issued a few seconds "in the future", or just expired, should be judged with a 60-second leeway rather than rejected outright. The test goes in Task 4.
2. **Cloudflare signing-key rotation.** A JWT whose `kid` is not in the cached JWKS triggers one refetch (at most once per 60 s), then succeeds or fails cleanly. The server should not need a restart. The test goes in Task 4.
3. **Access rejects the service token.** Cloudflare answers with a 302 redirect or an HTML login page instead of JSON. The worker must report "Cloudflare Access rejected the request (check service token)", not a JSON parse error. It must not follow redirects. The test goes in Task 8.
4. **Revoked or unknown credential vs. transient failure.** Connection refused and 5xx mean retry with backoff. 401 `credential_revoked`/`invalid_credential` means stop with exit code 3 and a re-enroll instruction. 426 means stop with exit code 4 and the server's diagnostic. The test goes in Task 8.
5. **Malformed vault config.** An unknown key, a missing required key, or an unsupported `format_version` must fail startup with a message naming the key and the file path, never a silent default. The test goes in Task 2.

---

## File Structure

```
Cargo.toml                                  workspace + shared deps
rust-toolchain.toml                         channel = "1.96.1"
crates/aq-protocol/src/lib.rs               wire types, PROTOCOL_VERSION, error codes
crates/aq-server/src/main.rs                clap CLI: `serve --vault DIR` (DATABASE_URL from env)
crates/aq-server/src/lib.rs                 AppState, build_admin_router, build_worker_router, run
crates/aq-server/src/config.rs              vault config.toml load + validation
crates/aq-server/src/store.rs               PostgreSQL foundation store
crates/aq-server/migrations/0001_foundation.sql
crates/aq-server/src/tokens.rs              random token generation + SHA-256 verifiers
crates/aq-server/src/identity/mod.rs        IdentityVerifier trait, VerifiedIdentity
crates/aq-server/src/identity/dev.rs        DevIdentity (development only)
crates/aq-server/src/identity/cloudflare.rs CloudflareAccessVerifier (JWKS + RS256)
crates/aq-server/src/authz.rs               Role, CurrentUser extractor, bootstrap, CSRF guard
crates/aq-server/src/admin_api.rs           /api/* human JSON routes
crates/aq-server/src/worker_api.rs          /v1/worker/* machine routes
crates/aq-server/tests/common/mod.rs        test harness: temp state, fixed clock, JWT minting
crates/aq-server/tests/{config,store,identity,authz,enrollment,worker_api}.rs
crates/aq-worker/src/main.rs                clap CLI: `enroll`, `run`, `status`
crates/aq-worker/src/{config,client,backoff}.rs
crates/aq-worker/tests/{backoff,client,lifecycle}.rs
Dockerfile                                  multi-target: `server`, `worker`
compose.yaml                                production-shaped: server + cloudflared, no ports
compose.dev.yaml                            dev override: dev vault, published 127.0.0.1 ports
vault.example/config.toml                   documented example config
scripts/test-db  scripts/dev-up  scripts/dev-worker  scripts/smoke
docs/development.md
```

Shared time handling: `pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;` defined in `aq-server/src/lib.rs`. Every expiry and offline computation reads `state.clock`, so tests use a fixed, advanceable clock. Every test in Tasks 3–8 that touches the store uses `aq_server::test_support::TestDb::create().await` (Task 3) and calls `.store().await`. Those tests need `DATABASE_URL` from `scripts/test-db`.

---

### Task 1: Workspace scaffold, protocol crate, health and version

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `crates/aq-protocol/{Cargo.toml,src/lib.rs}`, `crates/aq-server/{Cargo.toml,src/main.rs,src/lib.rs}`, `crates/aq-worker/{Cargo.toml,src/main.rs}`
- Test: `crates/aq-server/tests/health.rs`

**Interfaces:**
- Produces (`aq-protocol`):
  - `pub const PROTOCOL_VERSION: u32 = 1;`
  - `pub const SUPPORTED_WORKER_PROTOCOLS: (u32, u32) = (1, 1);` (inclusive min, max)
  - `#[derive(Serialize, Deserialize)] pub struct VersionInfo { pub server_version: String, pub source_revision: String, pub protocol_min: u32, pub protocol_max: u32, pub state_format_version: u32 }`
  - `#[derive(Serialize, Deserialize)] pub struct ApiError { pub error: String, pub message: String }` with code constants `UNSUPPORTED_PROTOCOL`, `INVALID_CREDENTIAL`, `CREDENTIAL_REVOKED`, `ENROLLMENT_INVALID`, `FORBIDDEN`, `UNAUTHENTICATED`, `CSRF_REJECTED`.
- Produces (`aq-server`): `pub fn version_info() -> VersionInfo`. It uses `env!("CARGO_PKG_VERSION")` and `option_env!("AQ_SOURCE_REVISION").unwrap_or("dev")`, with `state_format_version = 1`.
- Produces: a temporary `pub fn health_router() -> Router` serving `GET /healthz` → `200 {"status":"ok"}`. Task 7 folds it into both listeners.

- [ ] **Step 1: Write the failing test.** `health_returns_ok` sends `GET /healthz` via `tower::ServiceExt::oneshot` and asserts status 200 and body `{"status":"ok"}`. `version_info_reports_protocol_range` asserts `version_info().protocol_min == 1 && protocol_max == 1` and that `server_version == env!("CARGO_PKG_VERSION")`.
- [ ] **Step 2: Run** `cargo test -p aq-server --test health`. Expected: compile failure (crate empty).
- [ ] **Step 3: Implement** the workspace (`resolver = "3"`, `[workspace.dependencies]` holding every dep from Tech Stack, `[profile.release] lto = "thin"`, `strip = true`), the protocol types, `version_info`, and `health_router`. `aq-worker/src/main.rs` is a clap stub printing its version.
- [ ] **Step 4: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`. Expected: PASS, no warnings.
- [ ] **Step 5: Commit** `feat: scaffold agent-q workspace with health and version`

### Task 2: Vault configuration

**Files:**
- Create: `crates/aq-server/src/config.rs`, `vault.example/config.toml`
- Test: `crates/aq-server/tests/config.rs`

**Interfaces:**
- Produces:
  - `pub struct Config { pub format_version: u32, pub environment: Environment, pub server: ServerConfig, pub auth: AuthConfig, pub bootstrap: BootstrapConfig, pub enrollment: EnrollmentConfig, pub workers: WorkersConfig }`. All structs are `#[serde(deny_unknown_fields)]`.
  - `pub enum Environment { Development, Production }` (serde `lowercase`)
  - `pub struct ServerConfig { pub admin_bind: SocketAddr, pub worker_bind: SocketAddr, pub public_origin: String }`
  - `#[serde(tag = "mode", rename_all = "snake_case")] pub enum AuthConfig { Dev { dev_email: String }, CloudflareAccess { team_domain: String, audience: String } }`
  - `pub struct BootstrapConfig { pub owner_email: Option<String>, pub enabled: bool }`
  - `pub struct EnrollmentConfig { pub code_ttl_secs: u64 }` with a default of `600`
  - `pub struct WorkersConfig { pub heartbeat_interval_secs: u64, pub offline_after_secs: u64 }` with defaults of `15` and `60`
  - `pub fn load(vault_dir: &Path) -> Result<Config, ConfigError>`. It reads `vault_dir/config.toml`.
  - `#[derive(thiserror::Error)] pub enum ConfigError { Read{path, source}, Parse{path, message}, Invalid{path, key: String, reason: String} }`. `Display` always includes the path.

Validation rules (all yield `Invalid` naming the key):
- `format_version` must be 1. Any other value is rejected with reason `unsupported format_version N; this server reads 1`.
- `environment = "production"` with `auth.mode = "dev"` → key `auth.mode`, reason `dev authentication cannot be enabled in production`.
- In production, `public_origin` must start with `https://`. In cloudflare_access mode, `team_domain` must start with `https://`.
- `offline_after_secs` must be greater than `heartbeat_interval_secs`.
- `bootstrap.enabled = true` requires `owner_email`.

- [ ] **Step 1: Write failing tests:**
  - `loads_example_vault`: `load("../../vault.example")` succeeds, with `environment == Development`.
  - `rejects_dev_auth_in_production`: the error string contains `auth.mode` and `cannot be enabled in production`.
  - `rejects_unknown_key` (Review Focus 5): the extra `[server] admni_bind = ...` produces an error containing `admni_bind` and the file path.
  - `rejects_future_format_version`: `format_version = 2` produces an error containing `unsupported format_version 2`.
  - `missing_file_names_path`.
  - `bootstrap_enabled_requires_email`.
- [ ] **Step 2: Run** `cargo test -p aq-server --test config`. Expected: FAIL (module missing).
- [ ] **Step 3: Implement** `config.rs` and `vault.example/config.toml`. The example uses development, `dev_email = "owner@localhost"`, bootstrap enabled for `owner@localhost`, and binds `0.0.0.0:8080` and `0.0.0.0:8081`. Every key gets a comment.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat: load and validate versioned vault config`

### Task 3: Foundation store (PostgreSQL) and token verifiers

**Prerequisite:** Docker is reachable from WSL (Docker Desktop WSL integration), because there is no native PostgreSQL here.

**Files:**
- Create: `crates/aq-server/src/store.rs`, `crates/aq-server/src/tokens.rs`, `crates/aq-server/migrations/0001_foundation.sql`, `scripts/test-db`
- Test: `crates/aq-server/tests/store.rs`

**Test database:**
- `scripts/test-db` starts a disposable container named `agent-q-test-db` (`postgres:17-bookworm`, `127.0.0.1:55432`, password `test`, `--tmpfs /var/lib/postgresql/data`) if it isn't running.
- It prints `export DATABASE_URL=postgres://postgres:test@127.0.0.1:55432/postgres`.
- Database tests use `aq_server::test_support::TestDb` (module `crates/aq-server/src/test_support.rs`, compiled under `#[cfg(any(test, feature = "test-support"))]`; aq-server's own dev-dependency on itself enables the feature, and later aq-worker's dev-dependency does too). `TestDb::create().await` reads `DATABASE_URL` (panics with `set DATABASE_URL via scripts/test-db` if unset), creates database `aq_test_<uuid simple>`, and exposes `url()` and `store()`. Its `Drop` drops the database `WITH (FORCE)` on a helper thread. A missing `DATABASE_URL` fails the test run; it never skips.

**Schema (`0001_foundation.sql`):**
- `meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)`, seeded with `state_format_version = '1'`.
- `users(id UUID PRIMARY KEY, email TEXT NOT NULL, role TEXT NOT NULL CHECK (role IN ('owner','viewer')), issuer TEXT, subject TEXT, created_at TIMESTAMPTZ NOT NULL, bound_at TIMESTAMPTZ, UNIQUE (issuer, subject))`, with `CREATE UNIQUE INDEX users_email_lower ON users (lower(email))`. No `citext` extension.
- `enrollment_codes(id UUID PRIMARY KEY, code_hash TEXT NOT NULL UNIQUE, created_by UUID NOT NULL REFERENCES users(id), created_at TIMESTAMPTZ NOT NULL, expires_at TIMESTAMPTZ NOT NULL, used_at TIMESTAMPTZ, node_id UUID)`
- `nodes(id UUID PRIMARY KEY, name TEXT NOT NULL, credential_hash TEXT NOT NULL UNIQUE, worker_version TEXT NOT NULL, protocol_version INTEGER NOT NULL, enrolled_at TIMESTAMPTZ NOT NULL, last_seen_at TIMESTAMPTZ, revoked_at TIMESTAMPTZ)`

All timestamps come from `state.clock`, never `now()` in SQL, so tests control time.

**Interfaces:**
- `tokens.rs`:
  - `pub fn new_enrollment_code() -> String`: the format is `aqe_` plus base64url (no padding) of 20 random bytes.
  - `pub fn new_node_credential() -> String`: the format is `aqn_` plus base64url of 32 random bytes.
  - `pub fn verifier(secret: &str) -> String`: lowercase hex SHA-256. A plain hash suffices because the secrets are high-entropy random values.
- `store.rs`:
  - `pub struct Store { pool: PgPool }` (`Clone`)
  - `pub async fn connect(database_url: &str) -> Result<Store>`: a pool with max 5 connections and a 5 s acquire timeout, then `from_pool`.
  - `pub async fn from_pool(pool: PgPool) -> Result<Store>`: checks the state format **before** migrating, then migrates.
  - Migration runner (`crates/aq-server/src/store/migrate.rs`, or private fns in store.rs): `MIGRATIONS: &[(i32, &str)]` = `[(1, include_str!("../migrations/0001_foundation.sql"))]`. One transaction holding `pg_advisory_xact_lock(0x6167_656e_7471)` creates `schema_migrations(version INTEGER PRIMARY KEY, applied_at TIMESTAMPTZ NOT NULL DEFAULT now())` if absent. If `meta` exists and `state_format_version` > 1, or `schema_migrations` holds a version this binary doesn't know, it returns `StoreError::IncompatibleState { found, supported: 1 }` without changing anything (`found` = the meta value, or the highest unknown migration version). Otherwise it applies the missing migrations in order with `sqlx::raw_sql` and records each.
  - `pub enum Role { Owner, Viewer }`. It is defined here and re-exported by `authz`.
  - `pub struct User { pub id: Uuid, pub email: String, pub role: Role, pub issuer: Option<String>, pub subject: Option<String> }`
  - `pub async fn find_user_by_binding(&self, issuer: &str, subject: &str) -> Result<Option<User>>`
  - `pub async fn bind_invited_user(&self, email: &str, issuer: &str, subject: &str, now) -> Result<Option<User>>`. It binds only when the email row exists with `issuer IS NULL`.
  - `pub async fn bootstrap_owner(&self, email, issuer, subject, now) -> Result<Option<User>>`. One transaction: `INSERT INTO meta (key, value) VALUES ('owner_bootstrap_completed_at', $now) ON CONFLICT DO NOTHING`. If 0 rows are affected it returns `None`; otherwise it inserts the owner.
  - `pub async fn invite_user(&self, email, role, now) -> Result<User>`, `pub async fn list_users(&self) -> Result<Vec<User>>`
  - `pub async fn create_enrollment(&self, code_hash, created_by: Uuid, now, expires_at) -> Result<Uuid>`
  - `pub async fn redeem_enrollment(&self, code_hash, node: NewNode, now) -> Result<Option<Node>>`. One transaction: `UPDATE enrollment_codes SET used_at=$1, node_id=$2 WHERE code_hash=$3 AND used_at IS NULL AND expires_at > $1`. When `rows_affected == 1` it inserts the node; otherwise it returns `None`. Under READ COMMITTED the row lock makes a concurrent second redeem see `used_at` set.
  - `pub struct NewNode { pub name: String, pub credential_hash: String, pub worker_version: String, pub protocol_version: u32 }`
  - `pub struct Node { pub id: Uuid, pub name: String, pub worker_version: String, pub protocol_version: u32, pub enrolled_at: DateTime<Utc>, pub last_seen_at: Option<DateTime<Utc>>, pub revoked_at: Option<DateTime<Utc>> }`
  - `pub async fn find_node_by_credential(&self, credential_hash) -> Result<Option<Node>>`. This returns revoked nodes too, so the caller can distinguish revoked from unknown.
  - `pub async fn record_heartbeat(&self, node_id, worker_version, protocol_version, now) -> Result<()>`, `pub async fn revoke_node(&self, node_id, now) -> Result<bool>`, `pub async fn list_nodes(&self) -> Result<Vec<Node>>`

- [ ] **Step 1: Write `scripts/test-db`, run it, and export `DATABASE_URL`.** Expected: `pg_isready` succeeds against `127.0.0.1:55432`.
- [ ] **Step 2: Write failing tests** (each on its own `TestDb`):
  - `reconnect_preserves_state`: create a user, drop the `Store`, `Store::connect(db.url())` again, and find the user.
  - `migrations_are_idempotent`: `connect` twice on one database; the second run applies nothing and succeeds.
  - `refuses_unknown_future_migration`: insert `schema_migrations(version) VALUES (99)`; reconnecting returns `IncompatibleState` and the schema is unchanged.
  - `bootstrap_owner_only_once`: the second call with a different email returns `None`.
  - `email_unique_case_insensitive`: inviting `A@x.com` after `a@x.com` errors.
  - `enrollment_code_single_use`: the second redeem returns `None`.
  - `enrollment_code_expires`: redeeming at `expires_at + 1s` returns `None`.
  - `concurrent_redeem_yields_one_node`: race two `redeem_enrollment` calls on one hash with `tokio::join!`; exactly one is `Some` and `nodes` holds one row.
  - `refuses_newer_state_format`: set the meta value to `2`; reconnecting returns `IncompatibleState`.
  - `verifier_is_deterministic_and_hex64` (plain `#[test]`).
  - `tokens_have_prefixes_and_are_unique` (plain `#[test]`).
- [ ] **Step 3: Run** `cargo test -p aq-server --test store`. Expected: FAIL.
- [ ] **Step 4: Implement** `tokens.rs`, `store.rs`, and the migration. Use `rand::rngs::OsRng`.
- [ ] **Step 5: Run** the tests, then `grep -E 'name = "(libsqlite3-sys|sqlx-sqlite)"' Cargo.lock`. Expected: tests PASS; grep prints nothing.
- [ ] **Step 6: Commit** `feat: add PostgreSQL foundation store and credential verifiers`

### Task 4: Identity adapter (dev and Cloudflare Access)

**Files:**
- Create: `crates/aq-server/src/identity/{mod.rs,dev.rs,cloudflare.rs}`, `crates/aq-server/tests/common/mod.rs`
- Test: `crates/aq-server/tests/identity.rs`

**Interfaces:**
- `pub struct VerifiedIdentity { pub issuer: String, pub subject: String, pub email: String }`
- `#[async_trait] pub trait IdentityVerifier: Send + Sync { async fn verify(&self, headers: &HeaderMap) -> Result<VerifiedIdentity, IdentityError>; }`. Use the `async-trait` crate because the trait is used as `dyn`. Add `async-trait = "0.1"` to the deps.
- `pub enum IdentityError { Missing, Invalid(String), KeysUnavailable(String) }`. `Missing`/`Invalid` map to 401 and `KeysUnavailable` to 503 (in Task 5).
- `DevIdentity::new(default_email: String)`. It returns `issuer = "dev"`, `subject = email`, and `email` taken from the `X-AQ-Dev-Email` header when present, else the default. It exists only behind `AuthConfig::Dev`, which Task 2 forbids in production.
- `CloudflareAccessVerifier::new(team_domain: String, audience: String, http: reqwest::Client)`. It fetches JWKS from `{team_domain}/cdn-cgi/access/certs`. `CloudflareAccessVerifier::with_jwks_url(...)` is for tests.
  - It reads only the `Cf-Access-Jwt-Assertion` header. It ignores `Cf-Access-Authenticated-User-Email`.
  - Validation: RS256 only; `iss == team_domain`; `aud` contains `audience`; `exp`/`nbf` checked with `leeway = 60`; `email` claim required; `sub` required.
  - JWKS cache: `RwLock<(JwkSet, Instant)>`. On an unknown `kid` it refetches at most once per 60 s, then fails with `Invalid("unknown signing key")`.
- `pub fn verifier_from_config(auth: &AuthConfig, http: reqwest::Client) -> Arc<dyn IdentityVerifier>`
- Test harness `tests/common/mod.rs`:
  - `TestKeys::get() -> &'static TestKeys`: a 2048-bit RSA key generated once via `OnceLock`, with `kid = "test-1"`.
  - `TestKeys::jwk_set_json(&self) -> serde_json::Value`
  - `TestKeys::mint(&self, claims: serde_json::Value) -> String`
  - `serve_jwks(json) -> (String /*base url*/, JoinHandle)`: an axum server on `127.0.0.1:0` that counts fetches.

- [ ] **Step 1: Write failing tests:**
  - `accepts_valid_token`
  - `rejects_wrong_audience`
  - `rejects_wrong_issuer`
  - `rejects_expired_beyond_leeway`: `exp = now - 120`.
  - `accepts_within_clock_skew_leeway` (Review Focus 1): `exp = now - 30`, `nbf = now + 30`.
  - `rejects_hs256_token`
  - `ignores_email_header_without_jwt`: only `Cf-Access-Authenticated-User-Email` set → `Missing`.
  - `refetches_jwks_on_unknown_kid_once` (Review Focus 2): start the JWKS server with key `old`, mint with `test-1`, swap the served set to include `test-1`; verify succeeds and the fetch count is 2. Then an immediate second unknown kid fails without a third fetch.
  - `dev_identity_uses_override_header`
- [ ] **Step 2: Run** `cargo test -p aq-server --test identity`. Expected: FAIL.
- [ ] **Step 3: Implement** the identity module.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat: verify Cloudflare Access JWTs behind an identity adapter`

### Task 5: Authorization, owner bootstrap, CSRF guard

**Files:**
- Create: `crates/aq-server/src/authz.rs`; add `AppState` to `lib.rs`
- Test: `crates/aq-server/tests/authz.rs`

**Interfaces:**
- `pub struct AppState { pub config: Arc<Config>, pub store: Store, pub identity: Arc<dyn IdentityVerifier>, pub clock: Clock }` (`Clone`)
- `pub struct CurrentUser(pub User)` implements `FromRequestParts<AppState>`. Resolution order:
  1. `identity.verify`
  2. `find_user_by_binding(issuer, subject)`
  3. `bind_invited_user(email, ...)`
  4. If `bootstrap.enabled && owner_email` equals the verified email case-insensitively, `bootstrap_owner`
  5. Otherwise, 403 `FORBIDDEN` with the message `"<email> is not authorized for this Agent Q installation"`
- `pub struct RequireOwner(pub User)` returns 403 for a Viewer.
- `pub async fn csrf_guard(State(state), req, next) -> Response` is middleware on admin routes. For any method other than GET/HEAD it requires the header `X-AQ-Request: 1`. If `Origin` is present it must equal `config.server.public_origin`, else 403 `CSRF_REJECTED`. No CORS headers are ever emitted.
- `impl IntoResponse for` an `AppError` enum wrapping the `aq_protocol::ApiError` JSON.

- [ ] **Step 1: Write failing tests.** Use a router with one `GET /probe-viewer` (`CurrentUser`) and one `POST /probe-owner` (`RequireOwner`) built on test state with the Cloudflare verifier and static JWKS.
  - `first_visitor_is_not_owner`: an unrelated email gets 403 and no user is created.
  - `configured_email_bootstraps_owner`
  - `bootstrap_disabled_blocks_owner_creation`
  - `bootstrap_happens_once`: after bootstrap, a *different* subject with the same email is 403. This is the IdP-change case, which needs explicit rebinding.
  - `invited_viewer_binds_on_first_login`
  - `viewer_cannot_call_owner_route`: 403.
  - `mutation_without_csrf_header_rejected`
  - `mutation_with_foreign_origin_rejected`
  - `mutation_with_header_and_matching_origin_allowed`
- [ ] **Step 2: Run** `cargo test -p aq-server --test authz`. Expected: FAIL.
- [ ] **Step 3: Implement** `authz.rs`.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat: enforce owner and viewer roles with one-time bootstrap`

### Task 6: Admin API and enrollment issuance

**Files:**
- Create: `crates/aq-server/src/admin_api.rs`
- Test: `crates/aq-server/tests/enrollment.rs` (admin side)

**Interfaces (`pub fn admin_routes() -> Router<AppState>`, all under `csrf_guard`):**

| Route | Guard | Response |
| --- | --- | --- |
| `GET /api/me` | CurrentUser | `{email, role, auth_mode}` |
| `GET /api/status` | CurrentUser | `{version: VersionInfo, health: "ok", update: {state: "unavailable", detail: "host updater not installed"}}` |
| `GET /api/workers` | CurrentUser | `[{id, name, worker_version, protocol_version, enrolled_at, last_seen_at, status: "online"\|"offline"\|"revoked"}]` |
| `POST /api/enrollments` | RequireOwner | `201 {code, expires_at}`. The plaintext code appears only here. |
| `POST /api/workers/{id}/revoke` | RequireOwner | `204`; `404` if unknown |
| `GET /api/users` | RequireOwner | `[{email, role, bound}]` |
| `POST /api/users` `{email, role}` | RequireOwner | `201` |

- `pub fn worker_status(node: &Node, now, offline_after: Duration) -> &'static str`: revoked wins; then online when `now - last_seen_at <= offline_after`.

- [ ] **Step 1: Write failing tests:**
  - `owner_creates_enrollment_code`: the code starts with `aqe_`, and `expires_at` is now + 600 s.
  - `viewer_cannot_create_enrollment_via_direct_api`: 403. This is the spec's direct-API demonstration.
  - `viewer_cannot_revoke`
  - `worker_status_offline_after_60s`: fixed clock, `last_seen` at t0, status at t0 + 60 s is online and at t0 + 61 s is offline.
  - `revoked_status_wins`
  - `status_reports_update_unavailable`
- [ ] **Step 2: Run** `cargo test -p aq-server --test enrollment`. Expected: FAIL.
- [ ] **Step 3: Implement** `admin_api.rs`.
- [ ] **Step 4: Run** the tests. Expected: PASS.
- [ ] **Step 5: Commit** `feat: add admin API for status, workers, enrollment, and users`

### Task 7: Worker API, two listeners, and protocol negotiation

**Files:**
- Create: `crates/aq-server/src/worker_api.rs`; finish `lib.rs` (`build_admin_router`, `build_worker_router`, `run`) and `main.rs`
- Test: `crates/aq-server/tests/worker_api.rs`

**Interfaces:**
- `aq-protocol` additions:
  - `EnrollRequest { code, name, worker_version, protocol_version }` → `EnrollResponse { node_id: Uuid, credential: String, heartbeat_interval_secs: u64 }`
  - `HeartbeatRequest { worker_version, protocol_version }` → `HeartbeatResponse { server_version, heartbeat_interval_secs }`
  - `ProbeRequest { nonce: String }` (max 64 chars) → `ProbeResponse { nonce, server_time }`
  - `CompatibilityError { error: "unsupported_protocol", message, supported_min, supported_max }`
- Worker routes (no human routes mounted):
  - `POST /v1/worker/enroll`
  - `POST /v1/worker/heartbeat` (`Authorization: Bearer aqn_...`)
  - `POST /v1/worker/probe` (Bearer)
  - `GET /healthz`
- Protocol outside `SUPPORTED_WORKER_PROTOCOLS` → `426` with `CompatibilityError`. The message reads `"worker protocol N is not supported; this server accepts 1–1. Install a compatible aq-worker release."`
- Bearer auth: an unknown verifier → 401 `invalid_credential`; a revoked node → 401 `credential_revoked`. Cloudflare service-token headers are not checked by the app. They are the outer gate.
- `pub fn build_admin_router(state) -> Router` = `admin_routes` + `/healthz`. Every response carries `X-Content-Type-Options: nosniff` and `Cache-Control: no-store`.
- `pub fn build_worker_router(state) -> Router` = worker routes + `/healthz`.
- `pub async fn run(config: Config, store: Store) -> Result<()>` binds both listeners. Shutdown is graceful on SIGTERM/SIGINT.
- `main.rs`: `aq-server serve --vault <DIR>`, also via env `AQ_VAULT_DIR`. `DATABASE_URL` is required from env; a missing value prints `DATABASE_URL is not set` and exits 2. On a config, connection, or `IncompatibleState` error it prints the error and exits 2. The URL is never logged; log host and database name only.

- [ ] **Step 1: Write failing tests:**
  - `enroll_with_valid_code_returns_credential`
  - `reused_code_fails`: 400 `enrollment_invalid`.
  - `expired_code_fails`: advance the clock 601 s.
  - `heartbeat_updates_last_seen`
  - `revoked_credential_rejected_with_reason`: 401 `credential_revoked`.
  - `unknown_credential_rejected`
  - `unsupported_protocol_gets_426_diagnostic`: `protocol_version: 99`; the body names `supported_min = 1`.
  - `worker_listener_has_no_admin_routes`: `GET /api/status` and `POST /api/enrollments` on the worker router → 404.
  - `admin_listener_has_no_worker_routes`: `POST /v1/worker/enroll` on the admin router → 404.
  - `probe_echoes_nonce`
- [ ] **Step 2: Run** `cargo test -p aq-server --test worker_api`. Expected: FAIL.
- [ ] **Step 3: Implement** `worker_api.rs`, `lib.rs` wiring, and `main.rs`. Never log credentials or codes. Log node IDs only.
- [ ] **Step 4: Run** `cargo test -p aq-server`. Expected: all PASS.
- [ ] **Step 5: Commit** `feat: serve worker enrollment and heartbeats on an isolated listener`

### Task 8: Worker connector

**Files:**
- Create: `crates/aq-worker/src/{config.rs,client.rs,backoff.rs}`; flesh out `main.rs`
- Test: `crates/aq-worker/tests/{backoff.rs,client.rs,lifecycle.rs}` (`aq-server` as a dev-dependency)

**Interfaces:**
- `backoff.rs`: `pub fn delay(attempt: u32, rng: &mut impl Rng) -> Duration` uses full jitter, `uniform(0, min(60s, 1s * 2^attempt))`. Attempt 0 means a 1 s ceiling.
- `config.rs`:
  - `pub struct WorkerConfig { pub server_url: String, pub node_id: Uuid, pub credential: String, pub cf_client_id: Option<String>, pub cf_client_secret: Option<String> }`
  - `pub fn default_path() -> PathBuf` = `$XDG_CONFIG_HOME/agent-q/worker.toml` (fallback `~/.config/...`)
  - `pub fn save(&self, path) -> Result<()>` writes via a temp file created with mode 0600, then renames.
  - `pub fn load(path) -> Result<WorkerConfig>` refuses a file whose mode grants group or other access, with the message `"<path> is readable by other users; run chmod 600 <path>"`.
- `client.rs`:
  - `pub struct Client` wraps `reqwest::Client` with `redirect::Policy::none()`, a 10 s timeout, and adds `CF-Access-Client-Id`/`CF-Access-Client-Secret` when configured.
  - `pub enum CallError { Transient(String), Revoked, InvalidCredential, Incompatible(CompatibilityError), AccessRejected, Other(String) }`. Classification:
    - Connect errors, timeouts, and 5xx → `Transient`.
    - 401 `credential_revoked` → `Revoked`; 401 `invalid_credential` → `InvalidCredential`.
    - 426 → `Incompatible`.
    - Any 3xx, a 401/403 without our JSON body, or a `text/html` response → `AccessRejected`.
  - `pub async fn enroll(&self, req) -> Result<EnrollResponse, CallError>`, `heartbeat`, `probe`.
- `main.rs` commands:
  - `aq-worker enroll --server <URL> --name <NAME> [--cf-service-token]` reads the enrollment code (and, with the flag, the CF client id and secret) from a no-echo prompt (`rpassword`), or from stdin lines when stdin is not a TTY. These values are never taken as arguments. It saves the config.
  - `aq-worker run` loops: heartbeat every `heartbeat_interval_secs`, plus one probe at startup. On `Transient` it sleeps `backoff::delay` and increments the attempt; a success resets it. Exit codes: 3 on `Revoked`/`InvalidCredential` with `"credential rejected; re-enroll with aq-worker enroll"`, 4 on `Incompatible` printing the server message, 5 on `AccessRejected` with `"Cloudflare Access rejected the request (check service token)"`.
  - `aq-worker status` prints the node id, server, and last result without the credential.

- [ ] **Step 1: Write failing tests:**
  - `backoff_is_bounded_and_jittered`: over 1,000 samples at attempt 10, every value ≤ 60 s and they are not all equal.
  - `config_saved_with_0600`
  - `load_refuses_world_readable_config`
  - `redirect_classified_as_access_rejected` (Review Focus 3): a mock server answers 302 to `https://team.cloudflareaccess.com/...`.
  - `html_login_page_classified_as_access_rejected`
  - `server_down_is_transient` (Review Focus 4): a closed port.
  - `revoked_is_fatal`
  - `lifecycle_enroll_heartbeat_restart_revoke` in `lifecycle.rs`. It uses `aq_server::test_support::TestDb` (aq-worker dev-depends on aq-server with `features = ["test-support"]`). It starts `aq-server` routers in-process on ephemeral ports with dev auth, creates an enrollment via the admin router as the owner, enrolls the worker client, and heartbeats. It drops and restarts the server on the same database, and the heartbeat with the same credential succeeds with the same node id. After revoking, the next heartbeat yields `Revoked`.
- [ ] **Step 2: Run** `cargo test -p aq-worker`. Expected: FAIL.
- [ ] **Step 3: Implement** the connector.
- [ ] **Step 4: Run** `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: add worker connector with enrollment, heartbeat, and backoff`

### Task 9: Container images, Compose, developer commands, smoke test

**Prerequisite:** Docker is reachable from WSL (already needed since Task 3).

**Files:**
- Create: `Dockerfile`, `compose.yaml`, `compose.dev.yaml`, `scripts/dev-up`, `scripts/dev-worker`, `scripts/smoke`, `docs/development.md`; modify `README.md` (Development section) and `.gitignore` (add `secrets.dev/`)

Decisions:
- `Dockerfile` stages: `build` (`rust:1.96.1-bookworm`, `cargo build --locked --release`, `ARG AQ_SOURCE_REVISION` passed as an env var), `server` (bookworm-slim, UID 10001, `HEALTHCHECK` via `aq-server healthcheck`, a subcommand that GETs `http://127.0.0.1:8080/healthz`, add it to `main.rs`), and `worker`. OCI labels: `org.opencontainers.image.revision`, `io.agent-q.protocol-max=1`, `io.agent-q.state-format=1`.
- `compose.yaml` (production shape) defines three services:
  - `postgres`: `postgres:17-bookworm`, `POSTGRES_DB=agent_q`, `POSTGRES_USER=agent_q`, `POSTGRES_PASSWORD_FILE=/run/secrets/postgres_password` (Compose secret from `${AQ_SECRETS_DIR}/postgres_password`), named volume `aq_postgres:/var/lib/postgresql/data`, a `pg_isready` healthcheck, internal network `database` only, no `ports:`, and the same restart and logging settings.
  - `server`: image `${AQ_SERVER_IMAGE}`, no `ports:`, vault bind-mounted read-only at `/vault` from `${AQ_VAULT_DIR}`, `depends_on: postgres: condition: service_healthy`, networks `database` + `edge`. The entrypoint is `/bin/sh -ec 'export DATABASE_URL="postgres://agent_q:$$(cat /run/secrets/postgres_password)@postgres:5432/agent_q"; exec aq-server serve --vault /vault'` (as in `aq-graph`'s compose), `restart: unless-stopped`, `read_only: true`, `cap_drop: [ALL]`, `security_opt: [no-new-privileges:true]`, and log rotation per Global Constraints.
  - `cloudflared`: `cloudflare/cloudflared:<pinned tag>` running `tunnel run`, token via Compose secret file `${AQ_SECRETS_DIR}/tunnel_token`, the same restart and logging settings, and network `edge`.
  - Neither service mounts the Docker socket.
- `compose.dev.yaml`: `build: {target: server}`, vault `./vault.example`, the postgres secret from `./secrets.dev/postgres_password` (gitignored; created by `scripts/dev-up` if missing), publishes `127.0.0.1:8080:8080` and `127.0.0.1:8081:8081`, and disables cloudflared via `profiles: [tunnel]`.
- `scripts/dev-up` = `docker compose -f compose.yaml -f compose.dev.yaml up --build`. `scripts/dev-worker` runs `cargo run -p aq-worker -- run` against `http://127.0.0.1:8081` and prints enrollment instructions when no worker config exists.
- `scripts/smoke` (the release-verification check against built images, not source): build images, start the dev stack with a fresh `aq_postgres` volume, wait for health, and create an enrollment through the dev admin API with `X-AQ-Request: 1`. Then enroll the worker image (`docker run --network host ... aq-worker enroll` with the code on stdin), confirm one heartbeat, `docker compose restart server postgres`, confirm a second heartbeat from the same node id, and tear down with `down -v`. Exit nonzero naming the failed step.

- [ ] **Step 1: Write** `scripts/smoke` first and run it. Expected: it fails at "build images" (no Dockerfile).
- [ ] **Step 2: Implement** the Dockerfile, Compose files, and scripts.
- [ ] **Step 3: Run** `docker compose -f compose.yaml config` with dummy env. Expected: valid. Then `grep -c ports:` on the rendered production config prints `0`, and `grep docker.sock` finds nothing. The Cargo.lock SQLite grep still prints nothing.
- [ ] **Step 4: Run** `scripts/smoke`. Expected: `smoke: PASS` with each step logged.
- [ ] **Step 5: Write** `docs/development.md`. Cover the prerequisites (Rust 1.96.1, Docker), the two documented commands (`scripts/dev-up`, `scripts/dev-worker`), simulating a viewer with `X-AQ-Dev-Email`, a reminder that dev auth is refused in production, and how to run tests. Link it from the README.
- [ ] **Step 6: Run** `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && scripts/smoke`. Expected: all pass.
- [ ] **Step 7: Commit** `feat: add container images, compose stack, and lifecycle smoke test`

---

## Follow-on plans (written after this one lands)

- **Admin page:** moved out of the infrastructure plans into a separate dashboard task (owner decision, 2026-10-30). Phase-one demonstrations use the JSON API until then.
- **Plan 2: Hosted access** (rest of spec increment 2):
  - Bluehost NVMe 4 provisioning runbook (SSH, recovery console, firewall, Docker) and the Cloudflare DNS, Tunnel ingress (admin hostname → `server:8080`, worker hostname → `server:8081`), and two Access applications (email OTP allowlist; Service Auth).
  - Per-node service tokens.
  - `aq-server admin` SSH recovery subcommands (list users, rebind owner, re-arm bootstrap).
  - The worker running as a systemd user service in WSL2, with documented shutdown behavior.
  - Live demonstrations: owner login, origin isolation, enrollment.
  - Blocked on purchasing the VPS, choosing the domain, and setting up the Cloudflare account.
- **Plan 3: Release lifecycle:**
  - CI builds, a registry, a signed release manifest (revision, digests, protocol and state compatibility).
  - The host updater service (lock, journal on host, verify, maintenance mode, `pg_dump` backup, replace, probe, rollback) and an SSH status/restore command.
  - Blocked on choosing the registry, CI provider, and signing mechanism.
- **Plan 4: Recovery proof:** reboot and interrupted-update tests, credential revocation end to end, off-host encrypted `pg_dump`-based backup and restore with the credential-recovery decision, and operating instructions.
