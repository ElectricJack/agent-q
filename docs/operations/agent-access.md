# AQ-06 agent credentials and graph reads

Enable `[agent_access]` with `graph_url` (an HTTP origin), `graph_token_file`
(a private regular file containing an ordinary workspace service credential), and
`credential_ttl_ms` (1000–120000, default 120000). Startup validates configuration
and builds one HTTP client. There is no admin fallback and no graph credential
is returned to workers. Operators must provision an ordinary graph principal;
AQ does not infer or upgrade the privilege class of an opaque upstream token.

The trusted owner provision endpoint is `POST /api/agent-access/provision` with
`{attempt_id, profile, context}`. Profile and ContextSnapshot are API-01 objects.
The active attempt's frozen `ExecutionSpec.bindings.graph` must already name
exactly these profile/context digests and profile identity/revision, plus the
workspace. Project, node, boot, task, session and generation come from central
execution records. Existing fixture-only specs cannot confer this authority.
AQ-08's future scheduler must resolve and seal these bindings before launch;
this owner interface supplies trusted provisioning until that exists. Repeating
identical provision is safe; a changed profile/context requires a new attempt.
`POST /api/agent-access/{attempt}/revoke` permanently closes that authority.

The enrolled assigned node calls `POST /v2/worker/agent-session` with
`{identity, attempt, expected_credential_version}`. The first expected version is
0. A successful response contains `{api_version:1, token, context}`; `context` is
API-01 ContextResponse. Store the token privately. Only SHA-256 verifiers persist
centrally. Issuance serializes on node, attempt and binding rows and checks live
registration, generation, cancellation, revocation and grant expiry after locks.
Credential expiry is bounded by both the current grant and configured TTL.

Issuance uses an explicit version compare-and-swap. Replaying a request for an
old version yields `id_conflict` and never silently rotates the current token.
A lost issuance response has an unknown delivery outcome: plaintext cannot be
recovered. An authenticated assigned node may explicitly request rotation with
the next expected version; that invalidates the previous verifier. This is not
an idempotent plaintext-token recovery endpoint. WRK-05's token-file launch and
client retry integration remain separate work.

`GET /v1/agent/context` and `POST /v1/agent/graph/read` provide scoped reads.
Every read authenticates current central authority before dispatch and again
before responding, including immutable launch-context reads. Node, owner and
upstream service credentials cannot substitute for `aqa_` session credentials.
[AQ-07 durable mutation routes](graph-mutations.md) and [AQ-11 messaging routes](messaging.md) add separately authorized operations.

The read gateway validates API-01 shapes, unknown fields, duplicate JSON keys,
UTF-8 lengths, integer revision tokens and canonical digests. It projects nodes,
edges, historical nodes and history entries through the authorized scope;
upstream errors, metadata, provenance and actor fields are never forwarded.
Search/history filter before paging, with opaque centrally stored cursors bound
to attempt, credential version, authority revision and exact query/budget.
Rotation invalidates cursors. Per-attempt live cursors are capped at 128.

Scans, response bytes, upstream calls and request duration are bounded. A context
or relationship scan may return explicit partial coverage with no continuation;
it never asserts global absence. Search/history scan at most eight pages per
call. Historical unavailable relationships remain null and unavailable. Confirmed AQ-07 findings can receive creation-derived read grants when the frozen
profile permits them; the include-created flag alone grants no record, and unknown
operations or unapplied draft tasks confer no derived read permission.

Run unit and PostgreSQL tests with `cargo test -p aq-server --lib agent_access`
and `cargo test -p aq-server --test agent_access` using a disposable DATABASE_URL.
`python3 scripts/test-agent-graph.py` additionally runs all six gateway read shapes
against the selected qualified graph image, using an ordinary writer token and
synthetic hidden-canary records. It reuses API-01's disposable setup/cleanup,
checks the pinned image identity, and does not rewrite retained fixtures.

Response limits cover the full serialized ContextResponse and issuance envelope,
including profile/binding/credential metadata. Provision reserves maximum
credential-version widths; issuance checks its response before committing any
new verifier. Upstream responses must carry `X-AQ-Graph-API-Version: 1`.
