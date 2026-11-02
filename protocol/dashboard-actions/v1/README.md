# dashboard-actions-1

This server-owned contract adds `POST /api/v1/actions` and
`GET /api/v1/operations/{operation_id}` to the authenticated human router.
All mutations require `X-AQ-Request: 1` and the existing same-origin policy.
Production registers **zero kinds**; `dashboard.actions@1` remains disabled.
The request union is therefore empty in this release. This is infrastructure,
not an enabled task, graph, playbook or supervisor action.

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
Retention is published on the disabled action capability. Cleanup is explicit;
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
append `-- --check` to verify drift. The production request schema rejects every
kind until a later task registers and publishes it. Test-only adapters are never
included in this schema or the production registry. Action fixtures and dashboard
consumer migration belong to G1b; this package does not claim those deliverables.

Named acceptance tests:

- `cargo test --locked --workspace unknown_effect_operation_is_recoverable_by_lookup`
- `cargo test --locked --workspace action_union_rejects_unknown_kind_with_envelope`
