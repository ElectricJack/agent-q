# Human profile and terminal API v1 (CON-02)

This publication defines **future** authenticated AQ handlers and CLI outputs.
It ships no handlers, resolver, relay, service, daemon command or grants.
`aq-session-api` 0.1.0 owns these human shapes; `aq-session-protocol` 0.1.0 owns
shared central/remote session and terminal identity and worker wire semantics.
The generated `schema.json` and examples are drift checked independently.

Planned mappings (route spellings are reserved contracts, not existing endpoints):

| Contract | Future route | Future CLI |
| --- | --- | --- |
| profile_validate | POST /api/v1/profiles/validate | aq profile validate |
| profile_explain / profile_explanation | POST /api/v1/profiles/explain | aq profile explain |
| watch | POST /api/v1/terminals/watch | aq session attach (watch default), peek |
| acquire_control | POST /api/v1/terminals/control | aq session attach --control |
| input / resize | POST /api/v1/terminals/input or resize | attached controller |
| resolve_input | POST /api/v1/terminals/input/resolve | explicit controller recovery |
| message | POST /api/v1/sessions/message | aq session nudge |

Every request is authenticated. The server derives the principal, scopes the
response and independently checks current tenant/project/session permissions,
revision, policy, execution authority and expiry. Clients cannot choose a principal;
unknown mutation fields are rejected. IDs and structural validation never imply
ownership or grant authorization. Graph API-01 access profiles remain distinct
from agent/intelligence/execution recipes and retain their own digest binding.

Watch authorizes output only. Message submits a durable mailbox delivery intent,
never terminal keystrokes; nudge defers while human control is active unless the
controller explicitly yields. Controller acquisition is exclusive and expiring;
handoff waits for the higher fence acknowledgement or old permit expiry. Input
requires the exact terminal/session identity, current lease, expected controller
generation and session revision. Remote identity includes node/boot/attempt and
assignment generation; central identity uses central authority without a fake node.

Operation IDs are stable UUIDs. The future daemon must durably bind each to the
canonical request digest, rejecting changed-byte reuse and returning original
receipts for identical retries within its declared retention window. Current
permissions are checked again on retry. Queued/received/injecting/applied/
delivery_unknown/expired/rejected remain separate worker dispositions. Persist
injecting before calling the PTY. Never replay unknown input; pause later input
until explicit resolution with a fresh authorized snapshot. Resolution records
acknowledgement and cannot assert that unknown input was applied. Resize is a
separate sequence-ordered idempotent setting. Expired input is never replayed on
reconnect. Input cannot renew execution or replace cancellation/domain decisions.

Default input expiry is at most two seconds from admission; permits at most 15
seconds and never past execution authority. The daemon validates these against its
own clock, including skew/conservative expiry; clients do not extend them. Input
is 1..4096 bytes, pending input at most 32 KiB/session. Output/watch use the shared
protocol's bounded snapshot/sequence/epoch/gap semantics. Watch subscriptions have
independent expiry and limits (4/session, 32/node). Last-observed snapshots never
claim process liveness; new epochs reset rendering and unavailable history is a gap.

Decode through `aq_session_api::decode`: max document 256 KiB, nesting 24,
strings 4096 UTF-8 bytes, arrays 128 (literal input byte arrays 4096), dimensions
1..1000. JSON Schema maxLength counts Unicode code points; runtime byte limits are
additionally authoritative. Bounds are ceilings, not measured deployment capacity.

Profile explanation is bounded, caller scoped structured provenance: immutable
module order, requested/effective/denied operations, source revisions, omissions,
context byte/token estimates, concrete candidates and observed selection inputs.
An absent token estimate means unknown; tokenizer version identifies estimates.
References identify credential profiles/accounts, never secret values. Required
omissions make resolution invalid. Availability observations expire and cannot
authorize a provider; stale/unknown, unavailable, unsupported and policy denial
are distinct. A selected candidate must be eligible in the immutable snapshot.
This crate defines output shapes; AQ-18/20 implement and validate these domain rules.
Terminal text is sensitive untrusted content; rendering and independent output
authorization remain relay/UI responsibilities, and redaction cannot promise
arbitrary agent output contains no secrets. Raw input is not general audit text.

Rebuild: `cargo run -p aq-session-api --example publish-aq-session-api > protocol/session-api/v1/schema.json`.
Test: `cargo test -p aq-session-api` (no database, provider or sibling checkout).

Structural relational checks (not fully expressible in the schema) reject nil
identities, zero required revisions/times, malformed SHA256 digests, empty/control
references, duplicate module/candidate/permission identities, effective operations
outside the requested set, and valid explanations without one eligible selection.
Context byte totals use checked arithmetic and cannot exceed 16 MiB. Unknown input
requires a paused receipt. Watch coverage is explicitly complete, gapped or unknown;
complete coverage cannot accompany truncation or gaps. JSON Schema includes
`x-maxUtf8Bytes` for consumers to enforce alongside code-point `maxLength`.
Scope references are bounded logical `kind:identity` strings, not authorization.
The daemon must resolve and authorize their actual meaning.

Input resolution additionally carries `fresh_capture_request_id`: the daemon must
verify its server-owned registry binds this post-uncertainty capture request to
`fresh_snapshot_id`, exact terminal epoch and current controller generation. A
historical snapshot or caller-chosen nonce alone cannot clear uncertainty. This
crate cannot verify that registry or current authority offline.
