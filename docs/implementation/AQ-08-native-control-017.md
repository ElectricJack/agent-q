# Native worker control replies: admission0.1.7 candidate

Status: design candidate before implementation/publication. Frozen0.1.6 and its
archive remain unchanged. This document specifies the missing native control
replies; it does not claim poll/accept/prepare/grant/renew handlers are installed.
The source022 checkpoint owns only source preparation. Migration025 is reserved
for the actual producer's required storage and typed inline Source observation.

## Ownership and routes

All requests run on the existing authenticated worker listener. Headers are
checked with the opaque `AuthenticatedNode`; the current credential hash and
revocation are rechecked after locking the node. Boot/runtime/configuration,
project/policy/catalog/registry, exact sealed assignment and final clock fences
remain actual daemon checks. No request chooses a user principal, another scope,
server grant ID, prepared receipt owner or source retention fact. Scheduler
selection uses the installed project-policy principal, never an impersonated user.

The native routes use existing raw frozen request structs:

| Route | Request | Positive reply |
| --- | --- | --- |
| POST `/v1/worker/profiled/poll` | `transport::Poll` | bounded `NativePollPage` |
| POST `/v1/worker/profiled/accept` | `transport::Accept` | preparation delivery |
| POST `/v1/worker/profiled/prepare` | `transport::Prepare` | initial grant delivery |
| POST `/v1/worker/profiled/renew` | `transport::Renew` | `RenewedAuthority` |
| POST `/v1/worker/profiled/reconcile` | `transport::Reconcile` | bounded current-authority observations |

Started/Stopped/material/inline-artifact owners keep their existing typed routes.
Post-stop result recording remains the separate root-owned results endpoint.
Native source status/chunks use the existing independently versioned
`TransferManifest` plus binary body through a current-credential native adapter;
legacy source download/upload/finalize authentication is not reused as native
qualification. New native source writes must have the same current-node fences.
No arbitrary filename, path, socket, URL or shell hook is caller-selected.

## Fresh delivery versus immutable receipt

Preparation and initial grant operation receipts remain immutable. Exact replay
returns the original plan/grant identity and digest, not a new mutation or a
renewed deadline. A live delivery wrapper supplies a fresh conservative
`DeliveryClock`, clamped to that original deadline; it must not copy a historical
clock and present it as current. Expired, cancelled, released or unknown execution
authority produces a non-runnable refusal/current-state observation instead.

Freshness correlation: each physical HTTP transmission sends a nonnil
UUID `aq-delivery-nonce`, echoed in a typed delivery stamp outside the canonical
mutation digest. The operation ID remains stable across retries. The worker
validates the nonce and conservatively charges that nonce's actual monotonic
send/receive span, preserving every earlier known local terminal deadline;
retimed cached replies cannot create a fresh permit. The driver retains original
operation/owner/journal/expiry history across restart and rejects lower revisions
or same-revision changed authority. This freshness header is not authorization.
A retry cannot revive an expired reservation or grant. First-operation send
anchoring may only shorten a deadline as an explicit additional conservative
policy; it is not mixed silently with the fresh nonce interval. The daemon
generates the stamp after its final current-authorization/authority lock, never
from a caller TTL.

## Bounded polling and recovery

`NativePollPage` preserves frozen poll correlation/identity/time/cursor semantics
but contains `startup::NativeOffer`, not the compatibility `OfferedLaunch` list.
It carries at most eight offers and32 cancellations, never more than the requested
offer limit, and at most4MiB encoded bytes. Each offer is scoped to the exact
worker node/boot and uses the common sampled delivery time. Offers/cancellations
have unique attempt identities and cannot contradict each other in one page.
Empty polling is read-only and creates no permanently retained mutation row.

Reconciliation has at most32 current-authority observations and a4MiB envelope.
Caller-listed identities must match exactly; discovered held attempts are only
those owned by the authenticated node/boot. Unknown/not-owned requests receive a
non-runnable scoped refusal, not another tenant's binding. Current-state variants
are exclusive: offered, preparing, granted, cancelling/stop-required, stopped,
never-granted/released or unknown. They report immutable owner IDs, revisions,
hard deadline and original receipt references as applicable. They never infer
Started/Stopped from a journal revision or refresh a deadline. Reconciliation is
an observation, not permission to create/restart a host or inject terminal input.
A separate successful renewal operation is required to change running authority.

Worker journal absence/conflict, changed boot, expired local permit or ambiguous
physical host state stops new launch work and reports unknown recovery. A grant
that was issued but never produced Started is not NeverGranted; it remains held
until its separately qualified no-launch/stop recovery owner can prove disposition.

## Installed material and execution renewal

A `MaterialLease` expiry ends permission to retrieve that exact private material.
It does not erase an already verified admitted private binding or silently issue
execution authority. Initial `GrantedLaunch`0.1.6 retains its original rule: every
listed lease covers the initial grant expiry. A new `RenewedAuthority` can extend
the same running grant beyond the original retrieval window only after the daemon
rechecks current machine/agent scope, registry/profile revision and exact material
fingerprint against the original admitted installation. Changed/revoked material
refuses renewal; there is no implicit fallback or new material delivery.

A renewed reply binds the exact LaunchBinding, original grant ID,
reservation/logical guardian/launch owner, current authority/record/grant revisions,
worker journal revision and immutable execution hard deadline. Its fresh clock
cannot exceed that deadline. The worker rejects out-of-order or altered replay,
charges elapsed transport time, and never resurrects an expired/stopped host.
The original local permit expires independently if a response is lost or refused.

The worker retains private installed material only under its owner policy and
live execution lifetime. On stop/revocation/hard deadline it stops all authorized
writers before deleting their private binding. A delivery receipt is historical
installation evidence, never fresh permission to execute. Central renewal checks
use actual current owner records; missing facts refuse rather than infer account
identity from a file's presence or a worker capability claim.

## Publication and implementation gate

Add7 DTOs, bounded schemas/examples and independent digest/replay fixtures without
editing earlier artifacts. Tests must cover cross-node/boot scope, unknown/duplicate
fields, overlarge replies, contradictory identities/status, checked revision/clock
bounds, old delivery expiry versus renewal, lost-reply replay and current-node
rotation/lock-wait expiry in actual handlers. Exact archive pin precedes ordinary
worker HTTP adoption. The worker driver core can independently consume the frozen
individual NativeOffer/plan/initial-grant types while this surface is finalized.

## Material receipt time provenance

The frozen `MaterialReceipt.received_at_ms` uses the conservative observed
server-time watermark `MaterialDelivery.clock.server_time_ms`. It is not a claim
of synchronized worker wall time or an exact arrival/install instant. The worker
privately journals actual CLOCK_BOOTTIME send, complete-body receive and
installation/fsync completion, and rechecks local remaining expiry before
publishing a receipt. The daemon independently checks current server time,
original lease/preparation expiry, exact scope, bytes and admitted fingerprint;
the watermark is constrained provenance only and never fresh execution authority.

The initial grant delivery additionally echoes the original Prepare operation ID.
All positive replies use `control::ControlReply` tagged `kind`/`body`; requests
remain the raw existing frozen structs. An explicit reconcile batch is complete,
contains at most its requested limit and uses no discovery cursor. An empty
batch discovers only authenticated owned attempts using the bounded cursor.

For a retransmission of the same authority/grant revision, preserve its earlier
known local deadline. A genuinely newer renewal revision may extend a still-live
prior grant, bounded independently by the immutable local hard deadline; the old
grant expiry must not accidentally cap a valid extension. If the prior local
permit has already expired, the new reply cannot resurrect it. The public
`client_renewal_deadline_ns` helper makes this distinction explicit. An equal
initial grant revision must retain the original initial absolute expiry.
