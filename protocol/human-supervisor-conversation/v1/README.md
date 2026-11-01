# human-supervisor-conversation-1

Current authenticated Owner only; session inspection ACLs do not grant global
conversation access. Protected admin routes retain normal CSRF rules.
Capabilities: `supervisor.history.read@1`, `supervisor.message.enqueue@1`. The
legacy live-supervisor capability remains false. Runtime is always explicitly
unavailable with null binding in this foundation.

- GET `/api/human/conversations/global`: Discovery, stable conversation UUID and
  definition revision across restarts. It creates no message or session.
- GET `/api/human/conversations/{id}/messages`: History. Default newest 16,
  chronological output. Optional `limit` 1..16; `before` positive decimal cursor
  for older records OR `after` nonnegative decimal cursor for newer records.
  Both are exclusive. `next_cursor` is the next before/after value when has_more;
  `through_cursor` records the observed latest admitted cursor. Maximum 256 KiB
  encoded response; a page may be shorter to fit. No reply is invented.
- POST the messages path: closed Create, all fields required. Body nonblank,
  at most 16384 UTF-8 bytes, no NUL. Whole literal JSON at most 131072 bytes.
  Context and attachments must both be empty arrays. Positive revision is a
  signed 64-bit decimal string. Operation and message UUIDs are client minted,
  stable, nonnil and independent; preserve them with the original request.
- GET `/api/human/conversations/{id}/operations/{op}`: pure original-actor receipt
  lookup, 200 Receipt or 404. A 404 is not proof an earlier request has finished.

POST 200 means queued in the durable AQ mailbox, never delivered or reasoned.
POST 409 with a typed not_queued Receipt is a retained revision/message-ID refusal.
Its original operation cannot later enqueue. Generic admission/auth/error bodies
are not receipts; preserve ambiguous requests and look up their operation.
An identical request returns its immutable receipt; changed bytes that change the
canonical request conflict. Reformatting equivalent JSON does not change identity.

`request_digest` is sha256:lowercase hexadecimal over canonical JSON of the closed
Create object alone: recursively sorted object keys, compact UTF-8 JSON, integer
values only. It is not a hash of literal transport bytes. `body_digest` hashes the
literal decoded text UTF-8. The owning mailbox additionally binds authenticated
sender and exact conversation into its private operation digest. UI validates
conversation/operation/message/actor/digests before accepting a receipt. A queued
receipt's cursor points to the actual history message; never infer from body text.

History authors are server-derived human UUIDs. Every message has the stable
conversation UUID as thread_id. Delivery remains not_attempted and agent receipt
null; no runtime/worker fallback, external notification or provider call occurs.
Retention is bounded to 4096 operations and 64 MiB reserved bytes; capacity refuses
new operations while history, lookup and exact replay remain available.
