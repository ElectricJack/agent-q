# AQ-13 first durable bridge: exact manual admission and scoped message.send

This slice extends the reviewed AQ12 internal owner, without treating an engine
queue ACK as a domain effect receipt. The exact runner/Postgres0.1.1 source
publication adds `admit_once`; original0.1.0 source remains immutable. No wildcard
commands, new selector, direct graph completion, worker credential delegation or
central supervisor session is introduced. This design precedes migration023 and
server mounting;022 belongs to native registration and must be installed first.

## Owners and immutable boundaries

The qualified engine is the sole workflow-state owner. AQ owns authenticated
namespace/grant/admission/effect/event bridge records under migration023, using
its existing protected_storage keyring for protected inbox/outbox bytes and the
existing AQ11 mailbox service for message admission. The engine never reads AQ
tables. AQ never edits engine State, receipts or runner run IDs. One explicit
stable namespace ID/generation is distinct from each thread's ephemeral runtime
incarnation; restart must not change an effect/admission idempotency key.

Manual admission's engine request key derives from namespace ID/generation and
AQ request UUID. AQ first commits the exact canonical request and digest, bound
to authenticated owner, immutable grant ID/revision, expected activation revision
and derived Authority. The engine's single CAS creates run plus immutable key
receipt; historical exact replay creates no new authority/run/CAS. AQ ACK is a
later transaction. Crash after engine commit and before AQ ACK retries the same
retained request and returns the same run. Expired/revoked authority may inspect
that historical receipt under separately authenticated owner recovery; it cannot
create a new admission or message. No receipt around raw `admit` is used.

New admission checks a locked current grant and current users.role=owner after
lock waits. Host-derived Authority actor identifies the immutable grant/revision;
scope is the registered semantic scope and capabilities are the intersection of
the fixed catalog and grant. Actual workspace/project/task/session restrictions
remain typed AQ metadata, never inferred from a free-form scope string. The
engine atomically checks expected activation revision and engine authority
ceilings/revocations. Because engine/AQ are separate commits, queued workflow
admission is not permission for a later effect: dispatch independently rechecks
the current locked AQ grant, expiry and owner role before any message write.

## Migration023 records and bounded retention

| Table | Stable identity and retained data | Mutation boundary |
| --- | --- | --- |
| playbook_namespaces | namespace UUID + positive generation, name, workspace/project binding, configured catalog/source digest, owner user, finite count/byte counters | owner configuration; row lock serializes budget and idempotency admissions |
| playbook_grants | grant UUID + immutable revision, namespace/generation, owner user, finite allowed task/session recipients/capabilities, expires_at, revoked_at | authenticated owner; revocation and dispatch lock this row in consistent order |
| playbook_admissions | PK(namespace,generation,request UUID), canonical request digest, grant/revision, expected activation revision, sealed exact engine request, pending/engine_accepted/refused/cancelled/unknown, immutable engine receipt when known | insert before enqueue; exact replay only; changed bytes conflict |
| playbook_effects | PK(namespace,generation,runner Intent.id), exact command fingerprint/input digest/grant binding, original operation UUID, retained owner receipt/disposition | domain admission and bridge receipt/outbox are one AQ transaction for message.send |
| playbook_cancellations | PK(namespace,generation,runner cancellation key), exact intent/payload digest, accepted/refused/too_late/confirmed/unknown | no-effect tombstone before send or explicit domain confirmation; cannot erase queued mail |
| playbook_outbox | namespace/generation + fixed producer/epoch/event ID, canonical event digest, sealed exact envelope, engine acceptance receipt/cursor and bounded attempt metadata | owning effect + event commit together; engine acceptance ACK is separate |

Initial bounds are conservative and independently enforced:64KiB admission
plaintext; existing16KiB mailbox body;256KiB engine envelopes;4096 retained manual
admissions,8192 retained effects/cancellations,16384 retained outbox events and
64MiB combined protected retained bridge bytes per namespace. Config may lower
these bounds, never remove them. Namespace row locking serializes check+insert;
byte accounting derives serialized/encrypted server bytes, not caller hints.
Exact existing replay is checked before consuming new capacity. Retained keys
and uncertain effects are never evicted for quota. Full quota refuses new work;
no pruning/epoch reset/online migration is qualified in this slice. Counters are
bounded SQL row updates, not unbounded aggregate scans on every command.

Use existing protected_storage sealing with domain-separated AAD binding table
format, namespace ID/generation, request/effect/event immutable ID and canonical
digest. No provider, worker, node or Cloudflare credentials enter workflow or
inbox records. Key/source/catalog pins are explicit config, not discovered sibling
paths. Key absence/authentication failure holds recovery rather than reseeding.

## First real command: message.send

A fixed typed contract accepts only AQ11's task or exact immutable session
recipient, thread ID and <=16KiB nonempty body. Operation/message IDs are derived
from the stable namespace + runner Intent.id, not workflow-chosen sender IDs or
new UUIDs on retry. Sender is the authenticated granting owner; the bridge audit
binds the originating workflow/grant. There is no fabricated worker/supervisor
principal. Scope must match the namespace/project and granted recipient set.

Add a narrow crate-visible AQ11 service entry accepting the caller's transaction,
typed request and derived owner. It uses existing mailbox validation, recipient
identity, sender operation lock, message cursor and receipt. The mailbox remains
the sole mail owner. In the same AQ transaction, validate current grant/owner,
check immutable effect identity, call that service and retain the bridge receipt
plus fixed-producer outbox event. No external network call occurs under those
locks. Recheck expiry after lock waits and immediately before commit. Receipt
`queued` means durable mailbox acceptance; node/harness delivery and human read
remain their existing separate dispositions.

The runner contract is Reconcilable. `authorize` is a current-grant preliminary
check for fresh dispatch. `dispatch` rechecks locked current authority and exact
payload before writing. `reconcile` has an explicit historical recovery mode,
looks up only the retained original operation and never creates a new mailbox
write if absent. It cannot renew a grant or widen scope. `cancel` persists its
stable separate key; before first owner admission it can record a no-effect
cancellation tombstone, while already queued mail is too_late (no unsend fiction).
DB/unknown outcomes remain unknown until exact retained lookup resolves them.

`operation.get` initially exposes finite tagged bridge/mailbox receipts under
current owner visibility; no arbitrary endpoint/service proxy. Remaining context,
work and operation variants are explicitly unavailable until their owning typed
services are added. Never broaden read capability into write permission.

## Driver, event delivery and mount gate

The synchronous engine/Host remains on the dedicated bounded owner thread. AQ
service async work uses a separately configured bounded pool and explicit SQL
statement/lock deadlines through a captured runtime handle, never the execution
control pool. No authority transaction spans an external effect. Driver quanta,
manual admission replay, pending outbox pages and recovery are bounded; control
is checked between calls and cannot interrupt one blocked call. A timed-out
caller does not replace the owner or invent a retry key. Recovery mode must be
explicit so the runner's authorization gate permits only historical reconcile,
not renewed dispatch. Only fixed real contracts may be installed/activated.

Events use a fixed authenticated producer and stable explicit epoch/event IDs.
`admit_event_in_epoch` commits the event/frozen destinations atomically; `fanout`
uses stable destination→run identities. Lost ACK retries the exact event. Engine
acceptance is distinct from destination/run completion; blocked fanout remains
visible/pending. Explicit epoch retirement and disposal remain unavailable.

Coordinate a narrow optional protected-config/startup/router mount with root
after module and disposable API tests pass. No mutation route is mounted merely
because a crate/module compiles. Root owns ordered021/022/023 metadata union.
Tests must cover current owner/grant scope, expiry after a lock wait, revocation,
changed-key conflicts, expected revisions, quota concurrency, cancellation race,
mailbox/outbox atomic rollback and engine-commit/AQ-ACK loss/restart. Actual HTTP
listener isolation remains required. PB04's25ms budget remains open independently.
