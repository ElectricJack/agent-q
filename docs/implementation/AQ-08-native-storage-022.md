# AQ08 native producer storage review: migration022

Status: functional source-owner candidate registered in the normal migration
chain on the isolated producer branch, not yet merged. Existing migrations010–021
retain their meanings. The private source I/O owner is checkpoint9fa3428 with
bounded chunk followupbad2cb9; its real Git export boundary is qualified separately.
The022 owner commits actual reservations/completion/linkage, not native execution.

## Existing authority and receipts

The common `execution_attempts` claim and `profiled_authority_hot` remain the only
current execution authority. `profiled_launches`, artifact links, material links,
immutable API01/profile/selection/spec artifacts, source observations and owner
journals retain their existing jobs. Native offer publication inserts those
records atomically; it does not install an independent scheduling status table.
The existing worker-operation namespace `(node_id, operation_id)` and
`profiled_execution_receipts` stores accept, prepare and renew acknowledgments.
No retained mutation row is needed for an empty poll or a read-only continuation.
A poll may publish a bounded offer, and retries discover that exact held attempt.

Migration022 is source reservation/finalization/link ownership only. Grant
validation and phase-receipt hardening below are deferred until their actual
producer needs concrete storage; they are not unused tables registered here.

Receipt publication must become append-only before native handlers rely on old
preparation plans. The existing `never_granted` corruption test that deliberately
updates a stored plan will instead assert that storage refuses mutation; malformed
initial rows remain explicit fixture-only negative cases. Enforce raw canonical
response bytes and digest alongside the existing JSON projection, a maximum
262144-byte response and finite retained per-node/global count and byte quotas.
Exact receipt readback precedes quota admission; changed request content under an
existing operation identity conflicts. Historical acknowledgment never repeats a
state transition, refreshes an expired lease, or changes current authority.

## Pre-offer source ownership

Add `profiled_source_exports`, keyed by server-owned export `operation_id`, with
an exact immutable request digest and canonical request bytes. Its scope includes
project identity/revision, the original source-admission observation artifact, repository logical identity,
repository binding generation, exact SHA1 base and the configured live retention
handle. There is no attempt, execution-spec digest, node-selected path or remote
URL in the export request. Exporting retained source is not execution authority.

The row first reserves the source owner's configured worst-case bundle plus
bounded manifest bytes under the existing global staging quota lock. That quota
must include legacy transfers, legacy bases and these pre-offer reservations.
The owner performs its finite `artifact.export` exchange and bounded nofollow
manifest/bundle verification outside SQL locks. A pending or uncertain exchange
retains its reservation and retries the exact operation; it is never treated as
an absent artifact. Finalization compares the complete original identity and
publishes actual manifest bytes/digest, verified bundle digest/size and canonical
export receipt bytes atomically. A changed finalization conflicts even if its
revision or timestamp is larger. Immutable finalized bytes are never replaced.

A bounded link from each profiled launch to this export operation records actual
use. Initial operations each link to at most one attempt, matching the existing
unique base-operation owner. This intentionally repeats disk/source ownership for
a later attempt; it is not final deduplication or scalability qualification. Only after an actual attempt/spec is sealed may the source owner derive the
existing attempt/spec-bound `TransferManifest` wrapper. The export fact is
separate from `ProfiledExecutionSpec.source_artifact`, which remains the retained
source-admission observation. It also remains separate from a future candidate
source/result artifact. Initial cleanup is conservative: no active/pending owner
is deleted and no referenced export is reclaimed. At finite row/byte capacity,
new export admission refuses while exact replay remains available.

## Original grant validation

For a later coordinated migration, add one immutable grant-validation record
per attempt when the first-grant producer consumes it. It binds the
original decision identity/selection digest, exact held never-granted attempt,
current project/policy/catalog/registry revisions, graph task/context semantic
digests, exact source object/active retention and qualified final startup byte
proof. The canonical provenance envelope is bounded; no secret bytes, private
paths or unbounded raw graph/source snapshots belong here. Actual immutable
artifacts are referenced and retained through their existing owners.

Only the final first-grant transaction inserts this record. It discounts exactly
its own current never-granted held reservation from new-attempt charging while
retaining all prior started/ever-granted attempts, generation/history and the
finite NeverGranted retry bound. The original choice cannot change. A different
model, provider, credential, harness, task/context semantics or source object
requires closing the old ungranted offer and making a new admission. An old
selection decision or this validation record is not itself a grant or a renewal.

All fresh observations and private material reading occur outside SQL locks.
The final transaction begins with node/current credential → admitted native
registration/global configuration → common attempt → hot row, then uses the
existing project → catalog → registry → source-policy owner ordering and material
lease locks. Deterministic artifact/source child locks follow their owners.
Canonical shared owner helpers define the precise subset order; the producer
must not introduce a reverse order when composing them. Refresh the clock after the final wait; expired facts refuse before
publication. Recheck common claim and exact registered runtime/boot. Insert the
verified material receipts, owner Prepared journal, grant validation and first
Hot Granted transition atomically. Never infer Running; the actual Started owner
remains the only native transition producer.

## Qualification gate

Before registering022, freeze the exact source-owner DTO and quota arithmetic,
then add SQL guards and focused tests for reservation at capacity, uncertain
export replay, changed completion, immutable receipt conflict, mixed-kind task
claims, lock-wait expiry and atomic rollback. Producer assembly must count every
byte delivered to startup, including task/context and adapter framing; a proof
for generic profile instructions alone cannot authorize the complete manifest.
Actual configured CLI/model/counting qualification and a worker consumer remain
separate required gates. No unknown window or token count is represented by zero.

## Original reconciliation versus new admission

Exact persisted pending requests return only the original opaque reconciliation
capability before new-admission policy/freshness/quota checks. This keeps accepted
unknown intents recoverable after observation expiry or policy retirement. The
physical owner must still verify the original immutable Git operation/scope and
retention; no new ID, scope, quota or offer permission is minted. Exact verified
readback returns original canonical bytes even if the I/O adapter is now absent.
New reservations require current installed global configuration, enabled exact
project/policy, actual retained source-observation bytes and freshness checked
after every SQL lock wait. An absent or changed original operation conflicts.

Before linking a new offer, the producer may obtain a fresh source-admission
observation of the same object/policy; that new artifact pins the manifest/spec.
The export request retains its original observation as historical authorization.
The link checks exact repository/generation/base/project, current Offered held
identity, typed execution spec, actual immutable export completion, fresh active
transfer retention and source/offer expiry after locks. It creates only a scoped
base wrapper, not a host journal, grant or Started event. Native wrappers are
immutable and refused by legacy source HTTP routes; actual current-credential
native fetch remains an explicit next producer increment.

The022 tests use explicitly synthetic observation/export facts to qualify SQL
ownership and atomicity. They do not relabel those fixtures as real Git receipts.
The source I/O owner's separate pinned aq-git test qualifies actual export,
manifest/bundle bytes and active retention. A combined actual-Git/SQL producer
run and actual worker hydration remain required integration evidence.
