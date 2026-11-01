# AQ08 continuing native authority and historical reconciliation

Plan baseline: main `060c9b6` (state27, native offer/preparation and verified expiry owner), plus initial-grant source `54f4a7b` imported as `a858e60` in `/tmp/aq08-native-renewal`, branch `feat/aq08-native-renewal`. This is task-level policy/delivery planning, not qualification of unimplemented handlers. Frozen admission0.1.7 remains unchanged.

## Normative authority distinction

Initial admission chooses one concrete model/harness/account from fresh readiness,
context, capability, availability and budget facts. Its original NativeOffer,
PreparationPlan, material leases, selection and graph-context bytes remain
immutable. Expiring an original observation is not permission to replay selection
with a new timestamp, and is not by itself revocation of an already-running,
still-live grant.

Renewal maintains only the same held grant, LaunchBinding, reservation, logical
owner, installed credential binding and immutable execution hard deadline. It
never creates another attempt, changes model/account, grants a fallback, recharges
an attempt budget, infers Running, or replaces startup inputs. The existing
MaterialLease expiry controls retrieval only. A verified installed binding may
continue after retrieval expiry when its current authorization and exact private
material fingerprint are unchanged. No new delivery lease is issued.

A new renewal must observe the prior server permit still live after all final
locks and immediately before commit. The new absolute expiry must strictly grow
and be bounded by the original hard deadline, a finite server-chosen permit TTL,
and required new owner-observation expiry. An expired local permit independently
refuses extension in the frozen worker validator; server permission cannot revive
it. Cancellation, revocation, known stop, released claim, changed grant identity
or expired hard deadline deny renewal.

Fresh authenticated provider negatives or active cooldowns matching the exact
credential revision/account/endpoint/provider/model/node deny this first profile.
Stale or absent availability is recorded as unknown, never as available or missing
credentials. Unknown availability alone may permit maintenance of the already
held, still-live unchanged choice to its original hard deadline. It cannot admit
a new attempt or select another model. This continuation rule is deliberately
different from strict initial-admission availability policy and was approved by
the root reviewer before implementation.

Cost-capped or hard-accounting continuation remains unsupported unless a real
qualified accounting adapter supplies the required current facts. The first
renewal profile refuses such continuation rather than fabricating spent cost,
ledger completeness or provider-enforced limits. The immutable hard execution
deadline still applies; no epoch or lineage reset occurs.

## Required current facts

| Fact | Continuing policy |
| --- | --- |
| Machine and runtime | Current opaque authenticated node credential under lock; exact original node/boot/runtime/qualification still admitted and currently launch-eligible. A new framework boot cannot inherit an old live grant. |
| Project and principal | Same project binding revision and permissions/environment/profile eligibility; current configured worker membership and installed project-policy principal. Retirement or changed scope refuses. No human actor is invented. |
| Catalog | Pinned catalog remains the active digest; immutable selected definitions remain exact. No recompilation/reselection or prompt replacement. |
| Registry and credential | Current registry installed configuration; same exact profile/revision/account/endpoint/provider and node authorization. Actual bounded private-file fingerprint equals the original admitted material/installation receipt. |
| Node inventory | Fresh current inventory may advance normally; exact chosen harness/CLI/adapter/environment/model support must still intersect operator-qualified facts. Current free slot capacity is not required for an already-held slot. |
| Agent scope | Actual protected bootstrap binding exists, is unrevoked, and names the same sealed graph/profile/context identity. Reconciliation or a delivery receipt cannot mint agent credentials. |
| Task | Fresh public root-task observation retains exact expected task CAS/content revision and authorized root content. The original full context is not retimed or replaced; changing unrelated neighboring context does not silently retask the attempt. |
| Source | Same immutable repository binding/base and a fresh exact transfer-retention inspection from the existing Git owner. No re-export, fetch, reference mutation, new source observation artifact or hydration is authorized by renewal. |
| Provider health | Fresh matching negative/cooldown refuses; absent/stale evidence is explicitly unknown under the continuing-authority rule above. A current credential missing/revoked condition is separate. |
| Budgets | No new attempt or selection decision. Original hard deadline cannot grow. Unsupported cost/hard-accounting continuation refuses. |

Task/Git/private-file I/O finishes outside SQL locks. Observations retain their
actual conservative start/receive times; final time checks happen after all
relevant waits. Expiry of these new facts bounds only the new permit; original
admission clocks and bodies are never rewritten.

## Components and ownership

`profiled_execution/renewal` owns bounded phase-specific fact collection, replay,
current authorization fences and the atomic hot expiry update. It consumes
`initial_grant::load_original_grant_locked(tx,node,binding,grant_id)`, whose typed
historical result contains original operation/digest, GrantedLaunch, original
Versions and hard deadline. It does not reconstruct Prepare or fabricate a
historical InitialGrantDelivery nonce to reuse a public validator.

A trusted `intelligence` continuation fence rechecks the pinned choice against
current registry/inventory/health and current policy without calling resolve or
reusing an expired Decision as fresh authority. Small SQL projections retrieve
sealed task/selection/project identity; the full original multi-MiB offer and
startup bodies are not reloaded on every renewal. The existing common authority
cache remains immutable-only. An additional compact cache is optional: correctness
comes first; any cache must key complete guarded identity/digests, have explicit
entry/byte caps, and never retain current roles, grants, health or credentials.
Cold bounded verification is not a fleet throughput claim.

`source_retention::profiled_base` gains a narrow execution-retention inspection
entry using independent bounded read slots. Existing `inspect_retained` acquires
the cold-export bulk semaphore and therefore cannot be the renewal bottleneck.
The new entry uses the same exact owner/protocol validation, not a new Git service
or caller-selected scope. External service latency remains a refusal condition.

Private reads use a bounded dedicated owner: at most2 worker threads,2 queued
jobs and4 outstanding ownership permits. A timed-out or cancelled request cannot
release its job's permit before the actual read finishes. Jobs return only digest,
size and conservative observation timing; owned material buffers are zeroized.
No shared Tokio bulk/blocking queue can consume these reserved read workers.
All regular-file/no-follow/owner/byte checks remain those of the private material
owner; no path or material value enters the wire or error text.

Renewal receives a separate admission class and2-connection PostgreSQL view, with
finite acquire/statement/lock timeouts. Reconcile uses bounded control admission,
not the renewal pool. Saturated bulk, initial Prepare or reconciliation requests
must not exhaust renewal's own admission/SQL/private-read resources. Shared node
locks and external owners can still cause bounded refusal; this is not a claim
of guaranteed third-party latency.

## Mutation and replay

Raw `transport::Renew` is accepted at `/v1/worker/profiled/renew`, with bounded body,
strict duplicate/unknown field rejection and one physical `aq-delivery-nonce`.
No caller TTL, model, source or scope fields are added. The nonce is outside the
immutable request digest and is echoed only by a fresh delivery wrapper.

The final transaction follows the common order: current node/configuration,
exact attempt/hot authority, scoped agent/project/catalog authorization, current
registry/inventory/policy, then deterministic quota/material/source/receipt rows.
All current facts and prior permit liveness are sampled again after waits. SQL
continues deriving record/authority/grant revisions. Only grant expiry changes;
its binding/ID and original hard deadline stay fixed. Receipt insertion and hot
update commit atomically; rollback cannot leave a renewed authority without its
original receipt.

A new operation requires a journal revision newer than the accepted launch/renew
heads. Renewal counters are not physical start/stop evidence and do not advance
Reserved→Prepared→Started→Stopped transition history. A delayed actual Started event may predate a later acknowledged renewal counter. It retains its original physical checkpoint identity and cannot reset that independent counter; later renewal and terminal closure use the maximum actually acknowledged revision, without inventing a newer journal digest. Exact operation replay must
match its original request digest and original versions/absolute expiry. It may
freshly stamp only the same still-current, live permit under current facts. A
superseded or expired operation returns conflict/recovery rather than masquerading
as the latest authority. A new physical send charges its own elapsed interval;
equal revision retransmission preserves the worker's earlier local deadline.

Receipt storage uses the existing append-only API owner boundary: same-privilege
direct SQL remains trusted. No generic SQL immutability/cache guarantee is claimed
for application-owned execution receipts, and no unsolicited migration028 is
introduced. Finite global/node/attempt row and raw JSONB byte quotas include the
incoming request/response and exact replay is considered before quota admission.
No unknown or expired history is silently pruned.

## Installed credentials and bounded private observations

The original node inventory can truthfully describe a provisionable credential
as missing before preparation. For the exact original inventory revision only,
an actual retained matching MaterialReceipt supersedes that pre-install report.
The receipt is loaded and validated at the original issuance time; its digest is
never inferred from a Prepare request digest. A newer missing/unavailable binding,
an expired positive binding, or a missing installation receipt refuses renewal.
The inventory itself must still be fresh and support the exact harness/model.
The current inventory endpoint has no typed installed-material receipt input and
rejects a bare available=true claim for server-material credentials. Therefore
the first supported continuation remains bounded by the original inventory's
expiry. A newer preparation-only/missing report blocks renewal; indefinite
periodic continuation is not qualified until an actual receipt-bound installed
inventory producer/owner exists. This is an explicit integration limit, not
permission to reinterpret a newer negative as positive.
No report or provider-health timestamp is rewritten by this causal exception.

Private material verification uses two dedicated threads, a two-entry queue and
four owned jobs. Cancellation drops the waiter but keeps the ownership permit
until the real read and zeroization finish. Only size/digest leave the reader.
The two-second observation age bounds issuance after SQL waits; it does not turn
an observation into a two-second execution deadline. Actual current material
fingerprint, registry/principal fences and finite permit TTL remain required.

## Read-only reconciliation

Raw `/v1/worker/profiled/reconcile` consumes the frozen bounded Reconcile request
and returns ReconcilePage with physical nonce correlation. It observes current
or exact retained historical state only; it never updates a journal head, adopts
a reservation, renews, issues a grant, releases a claim or infers stop from an
input journal revision/digest.

Explicit batches return one scoped result per requested binding, up to32.
Unowned/malformed/unknown bindings get a bounded refusal without another node's
metadata. Cursor mode scans only the authenticated node's requested boot, with
finite rows and forward progress. Historical reads under a current machine
credential do not require re-enabling a retired launch allowlist; they never
permit a new boot to Accept/Prepare/Renew an old assignment.

Granted current state contains no DeliveryClock. Expired/cancelled/revoked/unknown
live authority returns StopRequired or a scoped refusal. Stopped/NeverGranted/
Released phases require their actual owner receipt artifact; unsupported release
provenance refuses rather than constructing a synthetic proof. Missing evidence
keeps claims conservative and cannot authorize a replacement.

## Qualification boundary

Tests reuse actual published→Accept→initial-Grant fixtures. They must show renewal
past original offer/selection/material retrieval expiry while every original body
and clock remains byte-identical, with the same grant ID and hard deadline.
Separate negatives cover fresh scoped health failures, unrelated account/model
observations, current credential/private-file/runtime/project/catalog/principal
revocation, task/source changes, stale newly collected facts, server/local expiry,
new-boot substitution, journal regressions, same-operation changed content,
superseded retries, quota plus incoming bytes, concurrent exact renewal, and
receipt-trigger rollback.

Hold real bulk/initial-control PostgreSQL connections and the shared blocking pool
while proving reserved renewal progress. Saturate reserved private-read ownership
and test cancellation without creating extra workers. Historical reconcile tests
cover explicit/cursor scopes, missing or wrong owners, actual Stopped/NeverGranted
receipts and the absence of any grant/clock mutation. Pure frozen client tests
continue to enforce no local resurrection and immutable hard deadline.

Focused tests and strict target lint/format precede a source freeze; source and
evidence commits remain separate. Full root assembly, actual worker permit
maintenance, configured real CLI/model/counting facts and throughput qualification
remain independent acceptance gates. No provider call or paid execution is needed
for this component implementation.
