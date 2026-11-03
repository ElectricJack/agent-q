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
  Owner items carry cursor `N` (their mailbox cursor). A supervisor reply carries
  its own cursor `A.R`: `A` is the max cursor of the batch it answers and `R`
  (positive) its reply-mailbox cursor. Items are ordered by `(N, 0)` / `(A, R)`,
  so each reply follows the owner message it answers. `limit`, `has_more` and
  `next_cursor` count replies like any other item, and `before`/`after` accept
  either cursor form. `through_cursor` stays an owner-mailbox cursor.
- POST the messages path: closed Create, all fields required. Body nonblank,
  at most 16384 UTF-8 bytes, no NUL. Whole literal JSON at most 131072 bytes.
  Context and attachments must both be empty arrays. Positive revision is a
  signed 64-bit decimal string. Operation and message UUIDs are client minted,
  stable, nonnil and different; preserve them with the original request.
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

## Supervisor History attachments (additive, for G8)

Published schema digest: `sha256:2a295e5376e0708543d2f667fc511ece4aaffb61590b9eb56ef30c5daf708ccc`
(also in `schema.sha256`; `supervisor_turns` fails if the two diverge). Pin it.

A supervisor `SupervisorItem` may carry `attachments` (0–4 items, schema
`$defs/SupervisorAttachment`). Clients must tolerate its absence; pre-attachment
items are unchanged. Every item carries the `message_id` of the reply that
produced it. Items are snapshots taken when the reply is recorded and never
change, so they remain readable after the operation is retired.

- `proposed_task_draft`: `draft_id`, `project_id`, `revision` (the payload's
  expected project version), `payload_digest` (`sha256:` over canonical JSON of
  `payload`, the same canonical form as `request_digest`) and `payload`, a
  dashboard-actions v1 `task.graph.create` payload in normalized form (omitted
  defaults are materialized; the digest covers the served payload). It is a proposal only:
  nothing is created until an Owner or supervisor submits it as an action.
- `action_receipt`: `operation_id`, `kind` (`task.create` or
  `task.graph.create`), `project_id`, `project_revision` (nullable),
  `created_task_ids`, `created_edge_ids`, `receipt_refs` (for example the Graph
  change set) and the operation's `request_digest`.

The supervisor adds them on `POST /v1/supervisor/conversation/replies` with
`attachments: [{type:"proposed_task_draft",draft_id,payload}|{type:"action_receipt",operation_id}]`
(`$defs/ReplyAttachmentRequest`, example `reply-attachments.example.json`). The
server derives every other field. Limits: at most 4 attachments, 64 KiB
serialized, unique non-nil draft/operation IDs, and drafts must pass
`task.graph.create` validation (400 otherwise). A receipt may name only a
confirmed, unretired creation operation journaled by the replying supervisor
session; anything else is 409 and nothing is recorded. Replies without
attachments keep their previous replay digest. History cursoring and page byte
accounting are unchanged and include attachments.
`history-attachments.example.json` is a complete page.
