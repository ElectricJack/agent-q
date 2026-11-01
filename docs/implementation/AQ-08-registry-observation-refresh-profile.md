# AQ-08 registry observation refresh profile: proposal before implementation

Status: design proposal only, based on `1111441`. No source, package version,
SQL, runtime configuration or activation is changed by this document. The first
implementation boundary is the pure admission contract. Runner owns subsequent
server/worker/terminal adoption and actual static qualification.

## Actual boundary

The accepted-preparation plan already records the limitation at
[`AQ-08-accepted-preparation-v2-plan.md:177`](AQ-08-accepted-preparation-v2-plan.md#follow-up-gate-independent-availability-refresh).
Frozen admission 0.1.9 `preparation_v2/facts.rs` requires the current Registry
fact to equal `proposal.choice.registry` in full. The server's
`profiled_execution/phase_execution/current.rs` independently requires that
same equality, and `facts.rs` correctly emits the actual current registry ref.
Changing only current.rs would therefore either still fail or require a false
fact to get through the published contract. Neither is an implementation fix.

`intelligence/registry.rs::configuration` already hashes immutable configuration:
configured revision, credential placements, qualified harnesses, authorized
credential definitions/state and private fingerprints. Availability mutations
advance `intelligence_registry.revision` without changing that config digest.
The actual artifact is currently `{id: aq.intelligence-registry, version: <aggregate
revision>, digest: <actual config_digest>}`. This profile does not redefine that
artifact or manufacture a new configuration digest.

The original anchor means the actual **proposal.choice.registry**, retained at
proposal publication. It must not be reconstructed from
`ResolvedSelection.registry_revision`: that earlier selection can precede
unrelated observations. Preserve the original ResolvedSelection bytes/digest,
proposal ref, later registry reads and observation records independently.

Runner's source-only case in
`tests/native_initial_grant/published/actual_git/worker_assembly/phase_assessment.rs`
verifies real Prepared journal/material/source/host state, then creates a new
authorized synthetic operator assessment with its own operation, sequence and
time. It preserves the earlier observation operation reply. It is explicitly
a deterministic static-fixture assessment, not provider health. That legitimate
mutation exposes the old exact-registry limitation.

## Negotiation and wire boundary

Add `aq_admission::preparation_refresh_v1` in a new immutable
**aq-admission 0.1.10** package. Keep `preparation_v2`, all old public schemas,
contract artifacts, feature enums, digest domains, errors, clocks and old
archives/checkers unchanged. New types do not accept an old negotiation as the
new profile, and old types do not gain refresh semantics.

The new exact artifact ID is `aq.admission.preparation-registry-refresh-v1-contract`,
artifact version 1. Its actual digest is generated from its own closed schema and
manifest; never copy the 0.1.9 contract digest. New schema URN:
`urn:aq:admission:preparation-registry-refresh:1`. Its manifest names actual
publication 0.1.10 and the independently frozen underlying transport families.

The new namespace has its own `PhaseFeature` and `Negotiation`. Required feature
strings are `accepted_preparation_v2`, `fresh_initial_confirmation_v2`,
`independent_execution_delivery_v2`, and `registry_observation_continuity_v1`.
They do not extend the frozen old enum. Reject unknown, duplicate, missing or
wrong-contract negotiation. Negotiate before persisting a proposal and require
the same exact profile through proposal, acceptance, preparation, confirmation
and execution delivery. There is no mid-attempt profile upgrade, retry fallback
or deserialize-old/change-tag conversion.

Top-level `schema_version:2` remains the **accepted-preparation transport family**,
not a declaration that the new document satisfies the frozen 0.1.9 schema. New
Schema/Document/DTO names are separately versioned. Preserve current top-level
field paths for original operation/identity/binding, accepted digest, grant and
times. Frozen migration 32 explicitly requires these version 2 paths; moving to
version 3 would require a new reviewed storage boundary. The new exact contract,
features and profile-qualified owner distinguish the new meaning. A decoder must
dispatch by exact contract before full typed validation, never by version 2 alone.
All decoding remains closed and byte-bounded; ambiguous/duplicate fields refuse.

Public API mirrors the phase family in its new namespace:
`Negotiation`, `Proposal`, `AcceptRequest`, `AcceptedReceipt`, `AcceptedDelivery`,
`PrepareRequest`, `FreshConfirmation`, `ExecutionReceipt`, `ExecutionDelivery`,
`Document`, `decode`, `schema`, `contract_manifest`, `contract_artifact`.
`ChoicePins`, resource/time/data helpers and original transport bindings retain
their existing meanings. Keep unchanged shared rules in a private validation
core with borrowed typed views; do not synthesize old receipts/facts to invoke
old validation and do not maintain a second timing/source/material algorithm.
Freeze actual0.1.9 outputs/errors before any such factoring.

## Registry continuity data and rules

The new accepted lease additionally retains `registry_at_accept`: an actual
locked owner read immediately before acceptance commit, not a caller guess.
The new Registry fact value is:

```text
Registry {
  selected_registry: ArtifactRef,   // exact immutable proposal.choice.registry
  accepted_registry: ArtifactRef,   // exact accepted lease.registry_at_accept
  preflight_registry: ArtifactRef,  // actual first locked Prepare owner read
  current_registry: ArtifactRef,    // actual final locked issuing owner read
  credential_profile: ArtifactRef,
  credential_revision: u64
}
```

Require the qualified owner ID `aq.intelligence-registry`, identical actual
configuration digest across all four refs, canonical positive decimal revisions
no greater than signed64 max, and:

```text
proposal.choice.registry == selected_registry
lease.registry_at_accept == accepted_registry
selected.version <= accepted.version <= preflight.version <= current.version
credential_profile/revision == original choice and actual original material
FactBody.revision == numeric current_registry.version
```

The preflight/final refs are separately produced owner reads around existing
bounded external Graph/retention/private-file work. Do not invent an earlier
record when one was never read. Retain both in the new provenance/confirmation
so final revision rollback cannot hide behind a still-greater-than-proposal
revision. The owner must also reject observation revisions newer than its
authenticated current aggregate revision. These checks detect rollback against
actual retained/observed floors; pure data cannot detect an externally restored
whole database without an independent watermark. No such claim is made.

All nine fact kinds remain mandatory. The other eight retain their exact
association checks. Registry facts, confirmation, proposal, accept/prepare and
execution receipts use new `aq.phase-refresh.<record>.v1` digest domains, avoiding
old-record impersonation. Original selection/choice identities remain original;
no re-selection, restamped selected model or new credential is authorized.

This bounded first profile refuses **any configuration digest change**, including
unrelated configuration changes. It only tolerates aggregate revision advances
with identical actual configuration. It does not optimistically project a
per-credential configuration hash from a global registry.

## Actual owner requirements and time limits

At publication, acceptance and final issuance, the registered runtime, exact
profile qualification, full authenticated node/boot/principal/project/attempt
and immutable original policy/choice/material associations still fence. The
new profile has separate policy/ExecutionQualification artifacts and an explicit
registry-continuity adapter set. An installed old runtime, capability bit or
caller-requested new contract alone cannot select the new path.

Current `accepted_preparation/policy.rs` chooses by runtime/qualification and
forbids duplicate runtime IDs. The new profile must use a distinct definition
namespace and exact lookup key `(contract, runtime, qualification)`; it cannot
reuse first-match lookup or broaden the old parser. Apply that exact key to
proposal policy, execution qualification and terminal qualification selection.
Duplicate matching definitions refuse. Supporting two profiles requires genuine
installed/qualified support and explicit caller negotiation, not automatic
selection of the newest definition.

Resolve current relevant observations through the existing health owner: latest
sequence per exact scope/source, all independent negative restrictions, cooldown,
credential revision/provider/account/endpoint/model/node and source expiry/age.
A later positive from another source cannot clear a retained matching negative.
An unrelated observation may advance the aggregate version; it must not make a
newly fresh matching positive invalid solely because of that aggregate advance.

No TTL or preparation-window change: 128KiB document cap, maximum preparation
120s, execution lease 120s and confirmation observation 30s remain. Actual source
age remains the selected policy's bound and expiry remains capped by the actual
observation's original received/expiry times, inventory/material/source retention
and original accepted lease. A fresh wrapper does not restart source age.
Continue the genuinely previously-unissued lineage/budget constraints, no cost
accounting substitution, no old proposal/material/input-retention extensions,
no double grant and exact immutable operation replay with independently bounded
delivery clocks. Cancellation/revocation/principal/config changes still refuse.

## Storage, terminal and package ownership

No new SQL table/column or migration is proposed. Existing proposal JSON and
phase execution request/receipt/provenance BYTEA slots can retain the new profile
within unchanged byte limits. Keep migrations 30/32/33 and all old history exact.
Version-aware owner readers return a typed profile variant and validate that
actual variant; they never project a fake0.1.9 receipt. Unknown contract refuses.
Actual SQL qualification must prove this against current frozen guards before
activation; a pure schema result is not that qualification.

Migration 33 also fixes terminal `schema_version:1`, protocol
`aq.phase-terminal.v1`, origin/initial-authority family `phase_execution_v2`.
Keep these family literals and actual phase-owner row associations. The new
terminal bridge must still emit the real new execution/accepted receipt digests
and complete actual retained Prepared checkpoint. Do not invent a legacy grant,
old receipt or new origin string that frozen 33 cannot retain.

Proposed package boundaries, subject to Root's version approval:

| Owner | Change | Frozen boundary |
| --- | --- | --- |
| This lane, agent-q admission |0.1.10 new pure profile +actual old corpus/common core +schema/examples/archive |All 0.1.9 modules/artifacts/archive meanings unchanged |
| Runner, terminal protocol owner |0.1.1 exact dependency on admission 0.1.10, additive refresh Context/negotiation/profile artifact |Old0.1.0 schema/artifact/API remain byte-identical |
| Runner, server |Exact new package pins; explicit proposal/policy/accepted/phase/history/profile dispatch and actual locked refs |Existing old path remains strict; no new SQL or canonical enablement |
| Runner, worker/bridge |New immutable vendored archives/source proofs, consistent exact dependency identity, explicit profile journal/host/terminal handling |Old0.1.9/terminal 0.1.0 archives and historical replay untouched |

The new terminal negotiation gets its own exact contract/features/digest domains
and typed refresh Context while retaining the family wire paths above. Legacy
Context continues accepting only frozen old phase types. Its new manifest names
actual admission 0.1.10/profile compatibility; never masquerade an expanded
implementation as the old 0.1.0 contract. Worker journal format 10 currently has
old concrete phase types. Runner recommends fresh format-11 roots for the first
new profile, with mutually exclusive typed old/new phase history and explicit
old-format refusal/inspection rather than automatic live-state migration; Root
must settle its bounded recovery policy. Runner must introduce an explicit versioned profile
representation/state-format boundary and old-reader refusal or compatibility
proof; replacing those types silently under 10 is not adoption.

Server exact pins and Cargo.lock may be changed narrowly in this lane only when
needed to compile the new pure package; phase-terminal dependency/package changes
are coordinated with Runner to avoid two nominal admission versions. Pure-package
workspace resolution will also need a coordinated terminal 0.1.1 manifest/dependency
bump, even before its new Context code: the old exact path requirement 0.1.9 cannot
resolve the main admission crate once it becomes 0.1.10. Do not relabel a changed
terminal manifest as a newly reproduced 0.1.0 archive. Runner owns
actual runtime/terminal/installation artifact and emitted-origin fixture pins,
HTTP adoption, process qualification and activation documentation. No provider
credential or real model launch is part of this initial contract work.

## Qualification before adoption

1. Capture actual untouched0.1.9 schema/artifact/manifest, representative complete
   phase documents, digest/time results and invalid-case error strings before
   shared-core edits. New package preserves every old vector and checker.
2. Pure positive: same configuration, actual selected/accepted/preflight/current
   refs with increasing aggregate revisions, unchanged selected credential and
   a genuinely fresh Ready fact validate. Equal current/original also validates.
3. Pure refusals: wrong config/ID/credential/profile/negotiation; leading-zero,
   zero, overflow or decreasing revision; preflight rollback even if still above
   original; false accepted/original binding; unavailable/cooldown/unknown, stale
   source or wrapping its old expiry; changed principal/material/context/pins.
4. Actual owner: new assessment after Prepared with old operation reply intact
   grants once under explicit new profile. Matching newer negative/cooldown,
   config/credential rotation, principal change, registry rollback, boot/runtime
   change and cancellation refuse. Unrelated observation updates do not poison
   another still-eligible offer. Include before/after external-read races.
5. Actual frozen SQL and typed history round-trip: new receipt/provenance retains
   truthful refs; response-loss replay preserves bytes/expiry; old history remains
   old and no cross-profile operation reuse occurs. Terminal Started/Stopped/
   closure correlate the real new receipt and retained checkpoint, not a facade.
6. Publish immutable standalone 0.1.10 closure with source byte proof; separately
   qualify terminal 0.1.1 and worker/server exact pins. Rerun bounded deterministic
   static processes with explicit profile name, no provider availability claim
   and no TTL widening. Production runtime qualification remains independent.
