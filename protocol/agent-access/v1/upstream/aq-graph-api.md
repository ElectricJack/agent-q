# aq-graph API v1

The [OpenAPI 3.1 contract](openapi.json) publishes every route, request, tagged mutation variant, and response shape. The service and CLI operate independently of Agent Q. All writes and authoritative reads use this API.

Send `Authorization: Bearer TOKEN` and `Content-Type: application/json` for JSON bodies. Credentials belong to one workspace and authenticate an actor with `read`, `write`, or `admin` capability. Read permits queries; write adds graph mutations and draft operations; admin adds credential management. A claimed actor field in mutation input is rejected. Tokens must contain 32–4096 bytes, are stored only as SHA-256 hashes, and must be supplied through the configured environment or protected file. The initial workspace/admin is provisioned by the local `init` command against configured PostgreSQL; bootstrap never replaces an existing workspace's credentials.

Every response carries `X-AQ-Graph-API-Version: 1`. Clients may send the same header; an unsupported value returns HTTP 400 with `unsupported_api_version`. The `/v1` path is the API major version. Unknown JSON fields are rejected in mutation, patch, draft, credential, and apply bodies. UUIDs are opaque identities. New server timestamps and descriptive client dates use RFC 3339 UTC at microsecond precision; supplied dates require a timezone and are normalized to UTC. Retained historical payloads and old receipts preserve their original representation. CAS versions and revision IDs are positive 64-bit integers; treat them as opaque tokens, not timestamps or global commit order. The revision field retains the wire name `content_revision`, but now covers every durable task or knowledge change, including lifecycle and owned relationships.

All workspace routes start with `/v1/workspaces/{workspace}`. The table below omits that prefix.

| Method | Path | Request / result |
| --- | --- | --- |
| GET | `/nodes` | Both kinds by default; optional kind/retirement/validity/review filters; includes disputed records |
| GET | `/nodes/{id}` | Current node, including a tombstone when deleted |
| GET | `/nodes/{id}/context` | Root node, bounded neighbors, edges, pinned snapshots, truncation flag |
| GET | `/nodes/{id}/tree` | Bounded recursive structural children: `node`, `children`, `truncated` |
| GET | `/nodes/{id}/history` | Paginated immutable common revisions, lifecycle events, and related edge changes |
| GET | `/nodes/{id}/revisions/{revision}` | Exact retained node and owned relationship versions, with legacy availability flag |
| GET | `/nodes/{id}/diff?from=1&to=2` | Full before/after node snapshots and changes to content, lifecycle and owned links |
| GET | `/ready` | Paginated intrinsically ready active tasks |
| GET | `/search?q=authentication` | Paginated full-text search across both kinds; optional shared filters |
| POST | `/mutations` | Atomic ordered batch; `MutationRequest` |
| POST | `/changesets` | Persist a named proposal; `DraftRequest` |
| GET | `/changesets/{id}` | Proposal, author, operations, status, and applied result |
| POST | `/changesets/{id}/validate` | Execute and roll back a proposal; return validated before/after preview |
| POST | `/changesets/{id}/apply` | `{"idempotency_key":"..."}`; revalidate and commit complete proposal |
| POST | `/credentials` | Admin: `{"actor_id":"agent","token":"32-or-more-random-bytes...","capability":"write"}` |
| DELETE | `/credentials/{actor}` | Admin: revoke actor; last administrator must remain |

`GET /health/live` is unauthenticated and reports process liveness. `GET /health/ready` is unauthenticated and returns 200 only when PostgreSQL is reachable and applied migrations exactly match the binary's supported versions and checksums; otherwise it returns 503 `not_ready`. All success responses use HTTP 200 and JSON. Credential success is `{"ok":true}`.

## Mutations and optimistic concurrency

A mutation envelope contains `idempotency_key`, optional `reason`, and 1–100 `operations`. The batch is all-or-nothing, executes operations in order, and can reference records created earlier in the same batch. The result is `{"change_set_id":"UUID","results":[...]}` with one resulting node or edge per operation. Required history and the idempotency receipt commit with current state.

```json
{
  "idempotency_key": "implement-login-1",
  "reason": "Approved implementation plan",
  "operations": [
    {
      "op": "create_node",
      "kind": "task",
      "title": "Implement login",
      "body": "Use the authentication decision linked to this task.",
      "task_state": "active",
      "metadata": {"aq:priority": 1}
    }
  ]
}
```

| `op` | Required fields | Optional fields |
| --- | --- | --- |
| `create_node` | `kind`, `title` | `id`, `body`, `acceptance_criteria`, `labels`, `metadata`, `task_state`, `content_type` (legacy `knowledge_type` alias), `verification`, `provenance`, `valid_from`, `valid_until`, `review_at` |
| `update_node` | `id`, `expected_version`, `patch` | Patch can set shared content/classification/provenance/dates, task lifecycle, or privileged retirement/reason/successor; derived summaries and verification use dedicated operations |
| `delete_node` | `id`, `expected_version` | None |
| `restore_node` | `id`, `expected_version`, `revision` | `include_lifecycle` (default false); restore is a new version, never history rewinding |
| `add_edge` | `source`, `target`, `edge_type` | `id`, `source_revision`, `target_revision`, `description` |
| `set_parent` | `id`, `expected_version`, `parent_id` | Set `parent_id:null` to detach; actual membership changes advance child version |
| `verify_node` | `id`, `expected_version`, `verification`, `evidence` | Admin only for both kinds; nonempty mutation `reason` required; status verified/disputed |
| `verify_knowledge` | Same as `verify_node` | Backward-compatible operation name with the same shared behavior |
| `set_summary` | `id`, `expected_version`, `source_revision`, `summary` | Shared derived text; `summary:null` clears text and citation |
| `remove_edge` | `id`, `expected_version` | None |

Task states are `draft`, `active`, `completed`, and `canceled`; only tasks carry `task_state` and acceptance criteria. Both kinds share `content_type`: fact, decision, policy, procedure, incident, reference, or note. New nodes default to note/unverified; a task defaults to draft unless activated explicitly. Verification is unverified, verified, or disputed independently of task completion. `knowledge_type` remains an input alias for `content_type`; fresh mutation responses and current/exact-revision node reads use the canonical `content_type` and include all shared nullable response keys. Retained pre-schema-7 idempotency receipts preserve their original committed response, which may contain `knowledge_type`; raw historical payloads also remain unchanged. Supplying both aliases in one object is rejected.

Tasks and knowledge share the same version-control machinery. Every meaningful authoritative mutation advances the node's CAS `version` and creates an immutable full snapshot identified by `content_revision`: planning edits, task lifecycle transitions, retirement or verification, deletion, parent assignment, and owned relationship additions/removals. Lifecycle and relationship events remain useful audit entries alongside that revision. A combined change commits its revisions/events atomically; an unchanged patch or restore does not create another node revision. Omitted patch fields leave values unchanged. Explicit null clears `valid_from`, `valid_until`, `review_at`, or `successor_id`; other existing optional content fields retain their earlier null-as-no-op behavior.

The immutable `created_by` identifies the authenticated creator; callers cannot supply or change it. Both kinds return nullable `last_verified_by`, `last_verified_at`, `retirement_reason`, `successor_id`, derived summary/citation, and descriptive dates. Older historical snapshots may omit these newer fields; migration recovers current authorship from the initial revision and refuses to bless legacy verified/retired state lacking attribution/reason. Deletion retains identity, tombstone, and history. Active incident edges must be removed explicitly before deleting a node, which can occur in the same atomic batch.

`restore_node` applies a retained snapshot as a new mutation with the same expected-version and idempotency requirements for either kind. It restores planning content and owned relationships while preserving current `task_state`, `retired`, `retirement_reason`, `successor_id`, and `deleted` by default. Set `include_lifecycle:true` to restore those lifecycle fields explicitly, including undeleting a tombstone; this flag requires administrator capability for either kind. Default task restoration still uses ordinary writer authority, subject to protected/retired records and affected neighbors; knowledge restoration requires admin. Meaningful restoration always clears verification actor/time and derived summary/citation for either kind, even if the historical revision was verified or summarized. A restored verification assertion never becomes a current trusted badge without a fresh explicit verification operation.

Restoration preserves stable edge identities and content such as descriptions and revision pins. Reactivated or changed edges receive new monotonically advancing edge versions. It rechecks current scope, authority, endpoint availability, expected versions, and graph invariants; a historical relationship that would now create a cycle or point to a deleted endpoint is refused. Rollback leaves node state, endpoint revisions, edge history, and idempotency receipts unchanged. Old historical revisions and edge snapshots never change. One restore may perform at most 100 relationship additions/removals in total; larger changes are rejected before commit.

`A blocks B` means B depends on A. Both endpoints of `blocks` and `parent-child` must be tasks. The combined blocks/parent-child topology is acyclic, including concurrent competing insertions. Parent-child forms a forest: one parent per child, at most three levels, and completed/canceled parents refuse new children. Parent completion requires all children completed or canceled. Draft or retired ancestors withhold descendants from readiness, retired tasks are excluded, and containers with live children are excluded from runnable leaves. An active leaf task is ready only when all blocking prerequisites are completed; a canceled prerequisite remains unresolved. Readiness conveys graph state and is not a worker claim.

Informational edge types are `references`, `motivated_by`, `produces`, `supports`, `contradicts`, and `supersedes`. References permit either endpoint kind; motivated_by/produces require task-to-knowledge; supports/contradicts require knowledge-to-knowledge; supersedes permits either endpoint kind. These can pin endpoint content revisions. Structural edges use current task state and reject revision pins. A missing pinned revision returns an explicit error; retrieval never substitutes current content. Edge addition and removal produce immutable history and advance the source record's revision/version. Structural blocks and parent-child changes advance both endpoints because prerequisites and parent membership belong to the target too. Other incoming informational links are derived views and do not change their target's revision. Changing a protected source link requires admin and invalidates verification and derived summaries. Structural edits check protected authority on both endpoints, including parent assignment and relationship-aware restoration. Duplicate structural links are no-ops; duplicate informational links conflict. Edge descriptions contain at most 4096 bytes.

Idempotency is scoped by authenticated actor, workspace, and operation (`mutate`, draft creation, or draft application). Repeating a key with the same parsed payload returns the committed outcome, including IDs; reusing it with another payload returns 409. JSON object key order is immaterial. Draft application includes the draft identity in the payload, so reusing the key for another draft conflicts. Keys contain 1–128 bytes. Pilot receipts are retained indefinitely; no automated receipt or history pruning runs. Clients can retry after a lost connection or restart with the same key. A timeout can occur after a commit, so always reuse the original key rather than inventing another one. Different keys remain distinct operations. Expected-version conflicts never overwrite newer content.

Draft creation takes the same batch fields plus a `name` of 1–200 bytes. Draft-generated IDs remain stable across validation and application. Drafts cannot affect live readiness. Validation executes the actual mutation logic in a transaction and rolls back all writes, including history; successful validation is a preview, not authorization to bypass checks during application. Application rechecks credentials, expected versions, endpoints, and invariants. Writers may propose corrections to another author's knowledge; applying any knowledge proposal requires administrator capability. Rollback-only preview may evaluate the proposed result with review authority, which never authorizes a committed writer mutation. A stale draft returns a conflict and remains unapplied. Large imports and graph branches are outside this contract.

## Shared record fields and permissions

Tasks and knowledge share content classification, provenance, verification, retirement, summaries, descriptive dates, and successor references. Ordinary task writers retain workspace write access; ordinary knowledge writers may revise only their own unverified findings. Editing verified/disputed or retired records of either kind requires administrator authority. Meaningful content, classification, provenance, date, or owned-relationship changes reset verification and clear actor/time; no-op edits retain current assertions and summaries. Task verification never completes or activates the task.

Creation starts unverified. Generic patches cannot assert a verification badge. `verify_node` requires administrator capability, verified/disputed status, a nonempty envelope reason, and named evidence. Evidence merges into provenance by `source_id`; the server records the authenticated actor and current timestamp and appends a revision. `verify_knowledge` remains an accepted legacy operation name. Verification is an attributed assertion about evidence; it never confers policy authority or modifies task lifecycle.

Each provenance entry is a typed descriptor with unique `source_id` of 1–128 bytes. Supported kinds are task (task_id, optional attempt_id/session_id), review (review_id, positive revision, sha256), git (repository, full commit identity, path, optional blob_sha256), artifact (artifact_id UUID and sha256), url (absolute HTTP(S) URL, observed_at RFC3339 timestamp, retained boolean, optional artifact_id/sha256), and legacy (snapshot_id, store, key, sha256). Fields must match their kind; hashes use 64 lowercase hexadecimal characters and Git commits use full 40- or 64-character hexadecimal identities. A URL claiming retained bytes must identify an artifact. These rules apply identically to provenance on either record kind.

These are attributed external descriptors. The service does not fetch caller URLs, open supplied paths, read AQ task/review tables, verify external retention, or infer review approval. Retention/access/approval assertions require an explicit external adapter before being treated as authority. See [the knowledge compatibility map](compatibility/knowledge.md) for translated assertions and remaining integrations. No v1 database importer is introduced by these shared fields.

`set_summary` stores supplied derived text and `summary_of_revision` bound to the record's current `content_revision`. Use `node.content_revision` as the source token together with `node.version` as the current CAS `expected_version`; a changed summary with a stale source token conflicts. Nonempty summary text is limited to 8192 UTF-8 bytes. A meaningful set or clear creates a common revision and resets verification; null clears both text and citation. Identical no-op values retain history. Any other meaningful change, including verification, dates, lifecycle, links, and restoration, clears the current summary. Exact history retains prior summaries and their original source citations. Summaries cannot be forged through create or ordinary patch bodies.

```json
{"op":"set_summary","id":"RECORD_UUID","expected_version":7,"source_revision":7,"summary":"A concise description of revision 7"}
```

`valid_from`, `valid_until`, and `review_at` are optional descriptive timestamps for either kind. With both validity endpoints present, valid_from must strictly precede valid_until after normalization to microseconds. They do not schedule work, set task due dates, or automatically change task state or readiness. Clear a timestamp by passing explicit null in an update patch. Clients choose whether to apply the query filters described below.

Retirement requires admin and a nonempty `retirement_reason` of at most 4096 bytes. The reason belongs only to retired state. It retains identity/history and leaves task_state unchanged; deletion is a separate tombstone operation. Retired content and outgoing links require restoration before ordinary editing. Admin may correct retirement reasons and successor pointers on retired records. Unretiring requires explicit lifecycle restoration.

A retired node may name `successor_id` of either kind in the same workspace. The successor must exist and be nondeleted, with a live `supersedes` edge directed from successor to retiring node. A successor may itself be retired and point forward. Named successor chains cannot cycle; informational supersedes edges are not independently constrained to be acyclic. Removing the required relationship or deleting a named successor is refused unless the referring metadata is explicitly corrected in the same atomic batch. `successor_id:null` clears that pointer. Current successor references and graph invariants are rechecked for restore and draft application.

## Queries, history, and limits

List, ready, search, and history queries take `limit` (default 50, range 1–200) and an opaque `cursor`. Pages contain `items` and `next_cursor` (null when exhausted). Pass the returned cursor unchanged with the same filters. Current-record lists use stable UUID order; history uses an internal sequence cursor. Pagination is not a whole-graph snapshot or change subscription. Concurrent commits may change later pages, and sequence allocation is not commit order.

A revision read returns `{node,links,relationships_available}`. Owned links comprise all outgoing relationships plus incoming `blocks` and `parent-child` relationships; other incoming informational links are derived from their source records. Stored edges retain exact historical versions. Neighboring record content is not recursively frozen: floating targets stay floating and explicit endpoint revision pins stay pinned. Restoring one record does not rewind the whole graph.

For a legacy revision created before relationship snapshots existed, `relationships_available:false` and `links:null` explicitly mean its exact link set is unavailable. The API never substitutes current relationships. Restoring such a legacy revision is refused safely rather than guessing or silently losing relationships. Newly created revisions retain their exact link sets. Exact revision reads are not silently truncated or paginated: an oversized result returns the explicit 2 MiB transport-cap error. A large-snapshot export/pagination facility is not part of the initial API.

History entries contain `sequence`, `entry_id`, `category` (`revision`, `lifecycle`, or `edge`), authenticated `actor_id`, `operation`, optional `reason`, `change_set_id`, `data`, and `created_at`. Change-set IDs group atomically committed edits. A history query for a node includes incident relationship changes. Diff takes node-specific common revision identifiers and reports durable field changes, including task state, retirement/deletion state and a `links` change entry for owned relationship changes. It also returns `before_links`, `after_links`, and `relationships_available`; unavailable legacy link sets remain null and are never guessed. It does not undo transitions or reconstruct a graph at a historical time.

Context query parameters are `depth` (default 1, range 0–5), `limit` (default 50, range 1–200, including the root), `max_bytes` (default 262144, range 1024–1048576), and `direction` (`both`, `incoming`, or `outgoing`; default both). Its shape is:

```json
{"node":{},"nodes":[],"edges":[],"pinned_revisions":[],"truncated":false}
```

`nodes` excludes the root. Context uses one consistent committed database snapshot and batched database-side adjacency reads. Edge count is capped at four times the requested record limit. Record, edge, and byte caps disclose omissions with `truncated:true`. The byte budget covers the serialized JSON envelope and pinned snapshots. A root larger than the requested budget returns 400 with guidance to increase it. A depth boundary defines the requested neighborhood; it does not itself imply truncation. The tree route uses the same depth/record/byte bounds but follows only structural children and returns recursive `{node,children,truncated}` branches. It does not include informational links or blocking neighbors.

Search uses PostgreSQL English full-text matching over indexed title/body in the authenticated workspace and returns both kinds by default. Search text contains 1–1024 bytes. Both list and search accept optional `kind=task|knowledge`, `include_retired=true`, `valid_at=RFC3339`, and `review_due_at=RFC3339`. List excludes retired/deleted records by default and includes disputed records. Search excludes retired/disputed/deleted records by default; its search-only `include_disputed=true` flag explicitly includes disputed results. Deleted records remain excluded regardless of flags.

`valid_at=t` matches the half-open interval `(valid_from IS NULL OR valid_from <= t) AND (valid_until IS NULL OR t < valid_until)`. Null bounds are unbounded. `review_due_at=t` requires a nonnull review_at at or before t. These filters combine with each other, kind, scope and pagination, and are inactive when omitted. Supply timezone-bearing RFC3339 values; URL-encode plus signs in non-UTC offsets. Semantic search and synthesis are not part of v1.

Server caps are 256 admitted requests, a 1 MiB request body, a 2 MiB serialized response, and a ten-second deadline covering body collection, authentication, pool wait, database work, and response serialization. Excess admission is rejected immediately without an unbounded queue. Database pools have 1–128 connections (default selected by deployment) and a two-second acquisition deadline. Mutation transactions use four-second statement, two-second lock, and five-second idle-transaction deadlines. Each mutation transaction attempt has a six-second overall deadline; serializable/deadlock retries are bounded to eight attempts with exponential backoff of 16–256 ms plus 0–16 ms jitter, all within the service request deadline. Pure node creation/edit uses READ COMMITTED and record row locks for version checks; graph mutations, relationship-aware restores, credential changes, and draft application use SERIALIZABLE. Draft creation uses READ COMMITTED. A large page may hit the response cap; reduce its limit. A mutation whose result would exceed 2 MiB is rejected with 400 `validation` before commit. JSON body errors are structured, including invalid content types and malformed fields.

Content caps are title 512 bytes, body 262144 bytes, acceptance criteria 16384 bytes, 64 labels of up to 128 bytes, and namespaced metadata up to 16384 bytes. Provenance is up to 64 objects and 16384 bytes. Derived summaries contain 1–8192 UTF-8 bytes when present; mutation reasons are up to 4096 bytes. Store large artifacts externally and place stable references in content. Never put credentials or secrets in versioned records.

## Errors, overload, and shutdown

Errors always have this shape:

```json
{"error":{"code":"conflict","message":"expected version does not match current state"}}
```

| HTTP | Stable code | Meaning |
| --- | --- | --- |
| 400 | `validation` | Malformed JSON, invalid UUID/query/fields, graph cycles, unavailable requested revision, or invalid domain input |
| 400 | `unsupported_api_version` | Requested header version unsupported |
| 401 | `unauthorized` | Missing, revoked, or wrong-workspace bearer credential |
| 403 | `forbidden` | Authenticated actor lacks capability |
| 404 | `not_found` | Record or route unavailable in this workspace |
| 405 | `method_not_allowed` | Unsupported route method |
| 409 | `conflict` | Stale version, unresolved references, duplicate identity, applied draft, or key payload mismatch |
| 413 | `body_too_large` | Request exceeds server byte cap |
| 413 | `response_too_large` | Response exceeds server byte cap |
| 500 | `internal` | Internal failure; SQL, connection strings, and driver details withheld |
| 503 | `overloaded` | Admission/pool/transaction capacity exhausted |
| 503 | `timeout` | End-to-end request deadline exceeded |
| 503 | `not_ready` | Health readiness schema/database check failed |

Every 401 includes a bearer `WWW-Authenticate` challenge. Every 503 includes `Retry-After: 1`. Apply jittered backoff and keep mutation keys and payloads unchanged. Error messages are explanatory; automate on status and code. Logs contain method, matched route template, status, and elapsed service time; they omit credentials, request payloads, concrete paths, and search query values. End-to-end client latency additionally includes network and response transfer. SIGINT/SIGTERM stop acceptance and drain requests; committed outcomes remain retryable after restart. Operational log retention is managed by deployment. Admission covers request processing through response buffering, then releases before socket transmission completes. The baseline server does not cap accepted TCP connections or total memory retained by slow socket consumers. Production ingress must set connection caps and header, read, write, and idle deadlines; those network protections are deployment responsibilities.

## Running the contract tests

Run a dedicated PostgreSQL instance or database, then:

```sh
AQ_GRAPH_TEST_DATABASE_URL=postgres://aqgraph@127.0.0.1:5432/aq_graph_test cargo test --all-targets
```

The suite applies real migrations and creates a fresh workspace and credentials per test. It fails clearly if the environment variable is missing or PostgreSQL is unavailable; it never silently skips database assertions. `tests/shared_nodes.rs` exercises the common fields, filter boundaries, summaries and draft previews, successor integrity, and protected structural endpoints for both kinds. `tests/unified_versions.rs` applies the same exact-snapshot, owned-link restore, lifecycle flags, CAS, replay, scope, and atomic rollback assertions to tasks and knowledge. Tests cover workspace/capability isolation, revocation, JSON/body errors, selective history and pagination, restore, competing updates, concurrent same-key retries, both structural cycle races, readiness semantics, draft rollback/application/staleness, atomic batch rollback, restart persistence, pinned content, deletion/search exclusion, and context limits. Transport unit tests cover overload, deadline cancellation/admission release, and serialized response caps.
