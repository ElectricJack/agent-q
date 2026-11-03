# dashboard-actions-1

This server-owned contract adds `POST /api/v1/actions` and
`GET /api/v1/operations/{operation_id}` to the authenticated human router.
All mutations require `X-AQ-Request: 1` and the existing same-origin policy.
Production registers `task.create`, `decision.resolve`,
`supervisor.message.send` and the typed task and knowledge kinds listed below.
The owner-only `dashboard.actions@1` capability advertises this boundary. Check
each mutation capability as well: task creation requires the configured Graph
mutation identity, decision resolution requires a mounted review owner, and
supervisor send requires the mounted global mailbox. Each typed kind has its own
bootstrap capability named after the kind (for example `task.retry@1`).

## Admission and recovery

Requests carry `operation_id` (non-nil UUID), `kind` (dotted domain.verb),
`subject` (`{type,id,revision?}`), `expected_versions` (positive decimal revision
strings), optional bounded `reason`, and a kind-specific strict `payload`.
The entire request is limited to 65,536 bytes; kind names to 128 bytes,
entity types to 64, entity IDs to 128, reason to 1,024, and expected-version
entries to 32 (keys up to 128 bytes; revisions up to 40 decimal digits).
References/reason reject control characters. Operation receipts have at most
100 receipt and 100 affected-entity refs and a 65,536-byte storage ceiling.
Unknown kinds and fields return 422. A registry is assembled from trusted Rust
adapters at startup, never from a client-supplied command, URL or shell string.
An adapter's `Payload` must deny unknown fields and its Schemars schema must
agree with deserialization. Each adapter must check current subject/scope
permission and expected versions; hints and old receipts confer no authority.

Canonicalization recursively sorts JSON object keys lexicographically and uses
Serde JSON compact UTF-8 serialization (array order preserved, no whitespace).
The SHA-256 digest includes the entire validated request and the server-derived
scope. Object-key order and insignificant JSON whitespace do not change identity;
changed values, fields, array order or scope do. Numbers use Serde JSON's finite
number representation; future kinds should use decimal strings for revisions
and lossless large integers. JSON duplicate fields are invalid.

Admission commits the original principal, scope, canonical digest and pending
operation before any adapter runs. Only the request that inserts the operation
may dispatch. Concurrent and later retries return the retained disposition;
changed identity returns 409. Authentication and adapter authorization run on
every retry. No request retry redispatches an admitted operation. Adapters must
implement durable domain-specific reconciliation before offering a recovery
that might cause effects. A crash leaves pending evidence; a dispatch timeout
or ambiguous effect is unknown. HTTP 202 means pending/unknown, never completion.

Operation lookup first filters by principal, then invokes the kind's mandatory
`authorize_lookup` policy on the current scope and every returned reference.
The policy must separately authorize a retired scope-only tombstone. Keep lookup
policies installed for retained kinds; missing policy or revoked access fails
closed with neutral 404. No request payload is retained just for read authorization.

Operation lookup is principal-private; missing IDs and another principal's IDs
both return neutral 404. A missing row means **admission unknown**. Preserve the
original ID and request bytes. Operation responses identify the digest, scope,
created/updated timestamps, subject refs, receipt refs, affected entities and
permitted recovery. Confirmed means only the effects named in that receipt.

Ordinary terminal receipts survive at least 90 days after their last update.
Pending and unknown operations never expire. Retirement erases response details
but preserves immutable ID/principal/scope/digest tombstones indefinitely; 410
for the original principal means retired, and the ID can never execute again.
Retention is published on the action capability. Cleanup is explicit;
keeping receipts longer than the minimum is safe.

## Errors and cursors

All `/api/v1/*` errors use `request_id`, `code`, `message`, `retryable` and
optional `operation_id`, `field_errors`, `current_versions`, `retry_after_ms`.
Error messages are static safe text; request values, SQL errors and credentials
never enter them. Required terminal clock and retry headers survive normalization.
The common error envelope covers all `/api/v1/*` families; specialized terminal
success/effect semantics remain unchanged. Cursor expiry uses 410 `cursor_restart_required`: restart the
list while preserving the reading anchor. Authentication is 401, authorization
403, neutral absence 404, conflicts 409, size 413, validation 422, unsupported
required contract 426 and capacity/unavailability 429/503.

The shared cursor codec uses random process-local keys, authenticated encryption
and a versioned domain separator. It binds principal, authorization partition,
scope, normalized filters, sort, page size, resource and snapshot, plus position
and validity interval. A restart invalidates existing tokens. Every consumer must
still recheck current authorization and snapshot membership; tokens grant no access.

## Publication and verification

Generate the strict schema with
`cargo run --locked -p aq-server --example dashboard_actions_contract`;
append `-- --check` to verify drift. Run `python3
protocol/dashboard-actions/v1/check.py` for schema, byte bounds, fixture replay
relationships and read/action version compatibility. Test-only adapters never
enter the production schema. Dashboard consumer pinning belongs to G3/G8.

The action payloads retain the native write fields, including `operation_id`,
which must equal the outer operation ID. `expected_versions` contains exactly
one key (`project`, `decision` or `conversation`), matching both the subject's
revision string and the payload's expected revision. The subject type uses that
same key and its ID is the actual project, decision or conversation UUID.

- `task.create` uses `human-task-create-1`: title at most 512 characters, body
  32768 UTF-8 bytes, acceptance criteria 8192 bytes, at most 16 distinct labels,
  and `draft` or `active`. The Graph task ID equals the operation ID. A confirmed
  operation references the validated Graph change set and exact task revision.
  Native human requests retain their original byte-sensitive request identity.
- `decision.resolve` includes the native resolve fields and `expected_subject`,
  the exact review subject artifact (`id`, `version`, `digest`). Both the decision
  revision and immutable subject are bound. The existing protected review owner
  checks these pins under its authority locks and retains the final receipt.
- `supervisor.message.send` keeps the caller's separate `message_id`, an expected
  conversation revision, a nonempty body of at most 16384 UTF-8 bytes, and empty
  `context` and `attachments` arrays. Confirmed means durable mailbox enqueue;
  native receipts still say host delivery `not_attempted`, with no agent receipt.

## Typed task and knowledge kinds

Each typed payload carries `operation_id` (equal to the outer ID), `project_id`
and `expected_version`, the decimal Graph node version that must also equal the
subject revision and the single `expected_versions` entry (`task` or
`knowledge`). `knowledge.author` instead uses `expected_project_revision` with a
`project` subject. A stale version returns a `conflict` whose operation carries
`current_versions` with the live Graph version; nothing is written.

- `task.edit` patches title, body, acceptance criteria or labels.
- `task.reprioritize` writes Graph metadata `aq:priority` (integer in
  -1,000,000..1,000,000, higher first) and preserves other metadata keys.
- `task.dependency.add` adds a `blocks` edge from `blocker_id`;
  `task.dependency.remove` removes `edge_id` at `edge_version`.
- `task.cancel` requires a `reason` and records a B3 native cancellation for the
  attempt holding the task; with no holding attempt it is `rejected`.
- `task.retry` asks B4 to assess the task; only an admissible replacement
  records the predecessor's replacement release (receipt `replacement_release`),
  after which the scheduler starts the replacement attempt. An unsettled or
  exhausted predecessor is `rejected`. Neither kind writes Graph task lifecycle.
- `knowledge.author` creates an unverified note (default content type `note`)
  whose Graph node ID equals the operation ID.
- `knowledge.correct` patches title, body, labels or content type.
- `knowledge.verify` requires a `reason` and 1 to 64 Graph `Source` evidence
  objects with verdict `verified` or `disputed`. Graph may answer `forbidden`,
  which is `rejected`.
- `knowledge.retire` requires a retirement reason and an optional successor;
  `knowledge.restore` restores an earlier `revision`.

Graph writes use the idempotency key `aq-dashboard-action:{operation_id}` and a
confirmed operation references the Graph change set and the exact new revision.

Legacy task-create, decision-resolve and supervisor-send HTTP handlers enter the
same action admission path and retain their native response envelopes. A legacy
message/decision handler racing an admitted operation before its native commit
can return a retryable unavailable response. Preserve the original ID and look
it up. Operation lookup reconciles existing native receipts without dispatch.
Explicit native task recovery retains its original fixed Graph key and bytes;
a later union lookup reflects its receipt. Native supervisor requests keep their
128 KiB JSON envelope limit, including escaping overhead; union HTTP requests
retain their 64 KiB envelope limit. Both enforce the native UTF-8 body bound.

Bootstrap remains `dashboard-read-1`. Its separate `contracts` entries advertise
both schema digests and versions. For the current owner, `pending_operations`
contains at most 100 own pending/unknown IDs, ordered by admission time then ID.
This includes unresolved pre-upgrade human Graph journal rows, deduplicated
against union history. Looking up a native-only task validates native evidence and
current access, imports admission metadata, and reconciles without dispatch.
Terminal, retired and other principals' rows are excluded. Truncation reports
partial coverage with `pending_operations_truncated`; discovery never grants
permission to inspect a reference. Bootstrap and lookup recheck current access.

Named acceptance tests:

- `cargo test --locked --workspace unknown_effect_operation_is_recoverable_by_lookup`
- `cargo test --locked --workspace action_union_rejects_unknown_kind_with_envelope`
- `cargo test --locked --workspace bootstrap_lists_pending_operations`
- `cargo test --locked -p aq-server --test dashboard_task_knowledge knowledge_correct_with_stale_version_conflicts`
- `cargo test --locked -p aq-server --test dashboard_task_knowledge task_retry_creates_replacement_attempt`
