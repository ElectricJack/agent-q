# AQ-08 scoped installed-material presence

This additive delivery plan follows native renewal `c5438d3` and its four-test
ordering followup `202b9f8`. Migration029 is reserved;028 remains immutable.
The new pure contract is part of the jointly unpublished admission0.1.9 with
accepted-preparation v2. Existing0.1.8 schemas, fixtures and wire modules retain
exact bytes. Root owns the combined package pin after both candidates freeze.

## Separate facts and authority

A node-wide inventory saying that credentials are provisionable for a new
admission is different from a current private installation for one already-issued
grant. A scoped presence fact cannot make any credential globally installed or
available, satisfy new placement/provider availability, grant/renew execution,
change choice, adopt a new boot, or release a claim.

Two negotiated APIs are proposed on the machine listener:

- Inventory v2 reports fresh qualified harness/model support and explicit
  `ProvisionableOnly` server-material profile/revisions. It has a node-wide
  sequence and finite TTL. Its immutable semantic sidecar names the actual
  snapshot revision, node/boot/runtime and installed presence qualification.
- Presence reports name one exact attempt/binding, original grant ID, owner,
  admitted lease and retained installation receipt/private binding. They have
  an independent per-attempt sequence and finite TTL. They name the actual v2
  inventory receipt current when a new report is accepted.

A later compatible v2 inventory revision preserves unrelated still-fresh scoped
facts. Compatibility requires the same node/boot/runtime qualification and
presence protocol qualification, and continued exact provisionable profile/
revision plus harness/model support. A newer legacy inventory, absent sidecar,
unsupported protocol, explicit incompatible inventory, or removed profile fences
continuation. Old legacy `available=false` rows are never relabelled.

Renewal requires both fresh compatible current inventory and a fresh exact
scoped Present fact. Existing current machine/runtime/project/catalog/principal,
registry/credential, central private fingerprint, provider-negative and task/source
checks remain mandatory. The selector does not consume scoped presence facts.
A Presence report does not renew execution or reissue retrieval permission.

## Qualified producer and clocks

Existing protected `ProfiledConfig.definition_bodies` retain a typed presence
qualification definition, its exact runtime/qualification and adapter/evidence
references. All required actual Definition bodies must exist and match digests.
Unknown/missing bodies refuse; a similar capability string is insufficient.
This avoids a new configuration/credential registry or implicit qualification.

The qualified worker producer must acquire the exact opaque live permit, then
reopen its owned private material using nofollow/regular/owner-only bounds and
validate the actual bytes against the original binding and lease fingerprint.
Its canonical observation binds logical boot, actual kernel boot, private
installation record, permit acquisition, read completion and send CLOCK_BOOTTIME
intervals. It checks freshness and local permit liveness before publication.
No private path or secret bytes appear in the report.

A new Present report requires the server's current grant revision. Its authority
anchor names the actual retained initial-grant/renewal operation and versions.
The server derives original issuance time from that immutable receipt; it never
trusts a caller wall timestamp or turns a cached sample fresh on HTTP arrival.
Expiry is at most the minimum of receive time plus finite TTL, actual original
issuance plus the qualification's maximum observation age, and original hard
deadline. Therefore a delayed/lost request cannot obtain an unrestricted fresh
TTL. Exact operation replay preserves original observation/deadline and never
updates the latest pointer. No separate challenge is required for this approved
first profile; the actual qualified worker producer remains a separate gate.

## Negative dominance and concurrency

The latest sequence/state is retained even after TTL expires. Missing or Unknown
supersedes earlier Present only for its exact node/boot/attempt/grant/lease.
Expiry of a negative becomes unknown; it never falls back to an older positive.
New Present must be newer than the retained sequence, use the current grant
revision and represent a later actual same-kernel-boot read. A delayed lower
sequence cannot overwrite a newer negative.

A genuinely newer scoped Missing/Unknown may refer to an earlier acknowledged
revision of the same grant when renewal raced ahead. Its actual anchor must
exist; future revisions, other grant IDs, boots, owners, leases and attempts
refuse. Such negatives never grant authority. A later positive still requires
current permit acquisition and an actual later private-byte observation.

Inventory updates and presence updates do not invalidate every attempt's facts.
Per-node inventory and per-attempt heads are separate, and compatibility is
checked at consumption. This prevents one reporting attempt from making every
other attempt on a busy node ineligible.

## Storage, locks and bounded work

Migration029 will own immutable inventory semantic receipts, immutable scoped
operation receipts and bounded latest pointers. Exact replay is checked before
quota/new-current eligibility. Initial bounds:16 harness qualifications,
32 provisionable profile references, one attempt/lease per report,64KiB request,
4KiB receipt,1–30s TTL and at most30s observation age. Row/byte quotas include
incoming canonical/uncompressed JSON plus declared overhead; no pruning of
unknown intents or negative heads to gain authority.

The machine routes use a reserved bounded admission/PG lane; they cannot consume
bulk preparation slots indefinitely. Worker private bytes are read by the
qualified producer, not accepted as caller-supplied secret material. Current
central material observations, where required for v2 provisionability, occur
outside SQL using the bounded existing owner. Final transaction order remains
node → exact attempt/hot → current authorization/project/catalog/registry →
lease/retained receipts → deterministic presence head/operation rows. Inventory
owner follows node → registry → inventory/head. SQL derives revisions.

After every lock/quota wait, check current authentication, runtime/global config,
exact grant/owner, cancellation/closure and fact deadline before commit. No
missing-table fallback and no generic SQL privilege guarantee is implied.
Application-owned rows and their immutable guards are tested directly.

## Staged delivery and checks

1. Freeze pure0.1.9 DTOs, bounded schemas/fixtures and package tests separately
   from server owners. Coordinate Cargo/version edits with the phase owner.
2. Implement actual inventory-v2 and scoped presence owners using029, exact
   qualification definitions and original receipt anchors; qualify disposable PG
   and mounted machine HTTP. Current legacy inventory bytes remain unchanged.
3. Integrate renewal consumption only after exact semantics are covered. Test
   issuance-age/network delay, immutable replay, negative dominance across grant
   revisions, compatible inventories with simultaneous attempts, legacy/
   incompatible fences, actual lease/receipt/kernel boot mismatch, current
   source/principal/private checks, quota and post-lock expiry rollback.
4. Independently qualify the worker's actual private-file revalidation producer
   and extracted package before claiming live installed-presence support. No
   provider call, real model execution or deployment is needed for daemon tests.

The current newer-negative policy stays unchanged until the reviewed typed
inventory and scoped-presence owners are present. AQ-08 remains in progress.
