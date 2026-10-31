# AQ-08 migration015: central never-granted closure

An offer can expire before an offline worker accepts or creates a guardian.
Waiting for a guardian NotStarted receipt strands the task. Inferring physical
stop from missing journal data is invalid. This path proves only that the exact
central assignment never authorized execution and is permanently closed.
Migration013 remains byte-identical;015 adds this distinct provenance.

## State and immutable owner record

Add native Status::NeverGranted, with grant=None and Stop::Unknown. An existing
real owner/reservation may remain recorded, but no guardian is invented. The
status is terminal and cannot return to offered/preparing/granted. Existing
known-stop/issued-grant/cancellation/revocation monotonic rules remain intact.

Add profiled_never_granted_closures keyed by exact attempt, with unique closure
operation, bounded canonical owner proof and digest, reason/time and observed
record revision. The proof binds the immutable full LaunchBinding (including
session/node/boot/generation, spec/manifest/access digests), the original offer,
material revocations, and actual central source/effect ledger observations.
The record is immutable and has no caller-writable HTTP DTO or bool proof.

The internal owner locks node -> attempt -> hot -> sealed launch, then material
leases and scoped source/effect rows. It requires a real held profiled claim,
no issued grant (the hot grant cannot ever be removed once issued), no retained
grant response, Stop::Unknown, and an eligible pre-grant state. Expiry closure
uses the actual applicable offer/preparation deadline; cancellation/input change
uses its actual owner trigger. It revokes all pending/admitted material leases.
It refuses if any task checkpoint transfer, graph intent/created record,
execution observation source/event, or handed/unknown harness delivery exists.
At most256 pending non-execution delivery records become stale; their IDs/count are retained, already-stale records do not block, and received/handed/unknown delivery remains a contradiction. Original retained base
artifacts remain with their actual central source owner; nothing claims a task
checkpoint was produced or a remote writer was physically stopped.

Insert proof, move hot status to NeverGranted and clear the common task claim in
one transaction.015 upgrades the SQL gate to recognize this exact closure beside
qualified physical-stop release. Claim reacquisition, grant creation, state
resurrection and proof mutation remain refused. Loader checks the proof digest
and exact sealed binding; active_at still refuses holds_task=false. Historical
inspection reports central never-granted provenance, never physical stopped.

## Attempts, lineage and finite retries

Only rows with this valid immutable closure are excluded from started-execution
budget charging. Legacy, unclosed, unknown-start and ever-granted attempts remain
charged/conservative. Assignment generations and history count every offer,
including closed offers; IDs/generations are never reused. Stable project/
workspace/task lineage does not change. A separate finite ungranted-offer count
(initial supported cap8) prevents offline workers or unavailable material causing
unbounded churn. No provider availability or task completion is inferred.

The existing resolver's all-row count and final predicate fence must share this
exact exclusion. A reserved own offer is not a replacement execution; final grant
eligibility requires a separate current-choice check that cannot double-charge
it or turn an old decision replay into authority. Actual grants remain gated by
fresh graph/project/catalog/registry/source/material fences.

## Required tests and exclusions

Qualify expired unaccepted offline offer, accepted-but-ungranted closure,
material revocation and fetch refusal, source/effect contradictions, raced first
grant versus closure, immutable replay/conflict and released-claim reactivation
refusal. Show an actual next placement can use a new monotonic generation with
unchanged lineage/budget aggregate, while finite no-grant retries eventually
refuse. No fake guardian proof is seeded for this path.

015 does not add project-policy decision principals (reserved017), observation
transport ownership (root016), or a physical host-start observation. Genuine
qualified Started evidence is a separate producer/consumer correctness seam;
renewal journal revision alone never promotes Running or source registration.

## Implemented preparation boundary

HotState.preparation_operation_id references the exact durable accept receipt
whose response is a valid frozen PreparationPlan. Closure checks its node, full
binding, guardian/launch/reservation and clock, and uses that current clock's
absolute expiry. An old revoked/expired material lease cannot shorten a newer
accepted plan. Missing, malformed or empty-material plans remain unknown and
refuse closure. The frozen v1 plan requires nonempty materials: installed-only/
zero-material preparation needs a separately qualified additive contract and is
not claimed as supported here. LaunchBinding and owner proof are validated in
Rust before commit, in addition to the SQL scope and digest checks.
