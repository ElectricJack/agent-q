# Central reservation owner API, version 1

This is an internal owner interface, not an HTTP or agent command surface. It is
frozen before source implementation. All operations accept an independently
trusted current human context and requery the exact current Owner under lock.
Request data never supplies that context. Project selection is irrelevant. Wire
artifacts describe canonical retained command/receipt data, not execution grants.

The contract literal is `central-reservation-1`. UUIDs are nonnil. Integer
revisions/cursors are bounded by signed PostgreSQL BIGINT; zero is allowed only
for initial actor revision, scan-after, or an empty pinned mailbox head. Digests
are lowercase unprefixed 64-character SHA-256. Closed integer-only canonical JSON
uses UTF-8, compact serialization and recursively sorted object keys. Reject
unknown/duplicate keys, fractions, invalid Unicode/NUL and oversized input.
Canonical command bytes are retained, not the caller's whitespace. The request
digest additionally binds the authenticated initiating human ID/email/issuer/
subject and operation kind, actor and conversation. Raw body hashes never stand
in for mailbox payload digests. Requests contain no message bodies or secrets.

`LocalPolicy` is actual retained configuration:

```json
{"schema_version":1,"kind":"global_supervisor_reservation","actor_id":"00000000-0000-0000-0000-000000000001","conversation_id":"00000000-0000-0000-0000-000000000002","enabled":true}
```

The conversation UUID must resolve to the existing singleton. Actor UUID creates
a real durable actor, not a placeholder task/session. Fixed installed limits are
16 messages, 65,536 raw body bytes, 128 examined mailbox rows, 4,096 lifetime owner
operations and 64 MiB reserved logical bytes. Policy bytes are at most 4 KiB.
No profile/catalog/credential references are accepted or invented at this stage.

The internal methods and retained requests are:

| Method | Request fields | Meaning |
| --- | --- | --- |
| configure_actor | operation_id, actor_id, expected_actor_revision, policy | Register the unique actual actor with expected revision 0, or update actual policy under exact actor revision; retain current approving human provenance. Actor/conversation mapping is immutable. |
| preview_batch | actor_id, expected_actor_revision, scan_after, optional through_cursor | Pure bounded query of real eligible mail under current enabled policy; return exact metadata, upper cursor, coverage and selection digest. No reservation or receipt write. |
| reserve_batch | operation_id, reservation_id, actor_id, expected_actor_revision, expected_policy_revision, scan_after, through_cursor, selection_digest | Independently rescan under lock and reserve exactly matching current source refs; caller cannot supply arbitrary member refs or bodies. operation_id differs from reservation_id. |
| lookup_operation | actor_id, operation_id | Pure lookup of immutable original result; return current reservation view separately. Never dispatch, re-reserve or rewrite the request. |
| inspect_actor / history | actor_id, bounded page cursor/limit | Shared current-Owner view of current actor/policy/reservation and retained metadata/history. No profile or runtime readiness inference. |
| cancel_reservation | operation_id, actor_id, reservation_id, expected_reservation_revision | Atomically retain explicit cancellation before plan/host intent, release only this active reservation, keep all original associations/receipts. |

Mutation request input is capped at 16 KiB; result at 64 KiB. Page limit is at
most 16 metadata records, with an encoded 64 KiB cap and truthful continuation.
A future generation, dispatch, host-input, evidence, Run, profile or arbitrary
context field is rejected. Unsupported new operation/history kinds fail closed.

`preview_batch` returns actor/policy revision, conversation ID, scan-after,
through-cursor, scanned-through cursor, selected exact MailRefs (message UUID,
mailbox cursor, original payload digest), selected raw body-byte count,
selection_digest, `coverage: complete|partial`, `has_more`, and an explicit status
`ready|no_eligible_mail|reservation_outstanding|disabled`. A partial scan is never
reported complete/empty. The scan cursor is not an acknowledgement or proof of
processing. Selection digest binds all of these source facts and installed
policy identity. Reserve re-queries; a digest supplied by a caller proves nothing.

Immutable mutation receipts identify contract, actor/conversation, operation ID,
initiating human user ID, request digest, resulting actor/policy revisions and a
closed disposition. Configure returns `configured`; reserve returns `reserved`
with the actual immutable reservation/batch metadata or `not_reserved` with a
reason; cancel returns `cancelled_before_plan` or `not_cancelled` with a reason.
Retained refusals include revision_changed, policy_disabled,
reservation_outstanding, selection_changed, no_eligible_mail and
reservation_not_current. Capacity/shape/current-authorization failures perform
no admission; lookup after an ambiguous transport/database error is mandatory.
Reused op identity under different bytes/kind/initiating principal conflicts.
Original successful receipts remain immutable after cancel/policy change.

Current reservation view has ID, actor/conversation, revision, original reserve
operation, exact source/batch metadata and state `awaiting_prepared_plan` or
`cancelled_before_plan`. It explicitly reports:

```json
{"runtime":"unavailable","prepared_plan":null,"session":null,"execution_history":"unavailable","history_scope":"reservation_only"}
```

Those fields distinguish a real reservation from a running supervisor. They do
not change API-07's separately frozen human conversation contract. There is no
recovery POST or resend after lookup: identical retained mutation retry returns
its original receipt; deliberate changed admission needs a new operation. An
unknown response never authorizes a new operation or automatic cancellation.

All current Owners can inspect shared actor/reservation state, including a
previous Owner's admission result. Replaying a mutation still requires the
original initiating identity binding. Policy disable blocks new reservation,
not authorized lookup or cancellation. An expired/revoked/different human
identity never gains access by knowing IDs or a valid digest.

Future execution admission requires an authenticated prepared-turn owner,
versioned profile/HostPlan bridge and complete all-generation UUID/cursor/operation
history under one CAS transaction. There is deliberately no method that accepts
a caller-supplied MailHistoryFence or starts a host. A generation-zero core fence
is invalid. This API cannot emit one or advertise central execution capability.
