# Keyed manual admission: additive0.1.1 design

The0.1.0 source archive and public package bytes remain unchanged. Change runner
to0.1.1 and its dependent Postgres/CLI packages to0.1.1; model/compiler remain
0.1.0. Publish/qualify a new exact source archive. No registry publication or
PB04 performance closure is implied.

Add `Engine::admit_once(KeyedAdmissionRequest, trusted_now_ms)` and a read-only
`admission_receipt(key)` lookup. The request carries a namespace-local opaque
key (1–256 UTF8 bytes, no NUL), binding and entry (1–512 bytes), positive expected
activation revision, exact input and host-derived Authority. Authority actor/scope
are1–512 bytes; <=64 capabilities each1–128 bytes. Canonical request bytes are
bounded to1MiB, then the selected artifact's existing schema/payload/closure and
protection rules apply. The key/digest/first accepted run/catalog/activation
revision and time form an immutable engine-owned admission receipt.

Within the same namespace CAS, first admission checks the expected activation
revision, calls/execution enabled controls, current engine execution revocations,
exact Authority ceilings and selected dependency closure, then creates the run
and receipt. No AQ domain tables participate in this transaction. Same key and
changed canonical bytes conflicts, including different Authority or activation
revision. Exact replay returns the historical receipt **without CAS, resolving a
new activation, granting authority, creating a new run, or advancing execution**.
It can succeed after revocation or a quota refusal solely as a historical lookup;
the authenticated host must separately authorize access to the old receipt.

Receipts are retained in namespace state under existing protected storage, with
no implicit garbage collection or producer-epoch reset. Combined ordinary
receipts + keyed admission receipts count against existing `max_receipts`; all
receipt bytes count toward `max_state_bytes`. Exhaustion refuses new admissions
atomically. Exact replay still works at capacity. Root/run GC cannot remove the
immutable key receipt or make the key available again. This initial profile
retains keys indefinitely and fails closed when full; no migration/pruning or
unbounded throughput claim. Original input is retained only in the existing run,
not duplicated in the receipt. A historical receipt does not promise that GC has
retained the run or its full output.

AQ's durable inbox/outbox is a separate commit boundary: it persists the exact
request before calling this API, retries only that immutable request, then ACKs
the returned receipt. Crash/lost response after engine CAS repeats the same key
and gets the same run. A caller timeout or AQ inbox receipt around raw `admit`
cannot provide this property. CAS failure before commit creates neither run nor
receipt. An unknown backend response is reconciled using the retained request,
never a newly generated key or refreshed authority.

Required tests: exact replay and changed bytes/Authority; activation CAS and
reactivation historical replay; quota/replay/GC retention; engine revocation
before first admission versus historical lookup; backend CAS refusal before
commit; backend response loss after successful commit + restart; protected actual
PostgreSQL persistence, key mismatch and owner restart. Existing0.1.0 behavior and
workspace suites remain required. Protection does not turn serialized Authority
into authenticated AQ authority: the host owns that live authorization boundary,
and downstream effects recheck current grants independently.


## Writer compatibility fence

Postgres0.1.1 migration003 adds a monotonic per-namespace writer format. New
acquire/CAS/release transactions set exact namespace + format markers locally
for that transaction; format1 namespace writes require both. Markers expire at
commit/rollback and cannot authorize another namespace. A trigger rejects format
downgrade or namespace deletion. New Memory/Postgres CAS also verifies that every
existing admission receipt remains exactly unchanged. The schema version rejects
a fresh0.1.0 constructor; the transaction marker also fences a previously opened
old connection from reacquiring/writing a newly enrolled namespace, even where
encrypted snapshot bytes hide the new field. This is a qualified-writer format
fence, not authentication against a malicious DB owner who can alter triggers.
Retire old owners before upgrade; mixed-version/online upgrade is not qualified.
The unchanged old archive probe explicitly holds an old0.1.0 connection across
new admission and tests warm reacquisition refusal plus cold constructor refusal.
