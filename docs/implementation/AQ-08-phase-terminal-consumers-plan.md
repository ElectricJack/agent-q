# Phase terminal consumers over actual initial authority

Status: design proposal only. No new route, capability, migration or runtime owner is activated by this document.

This follows the state32 issuer (`e934dd4`, `53916d0`, `3af7741`) and its [bounded qualification](AQ-08-phase-execution.qualification.md). The next slice must allow an actual phase execution receipt to reach a matched physical terminal fact without manufacturing a legacy preparation plan, InitialGrantDelivery or GrantedLaunch. Production activation remains closed while these consumers, their qualified worker producer and the writer boundary are incomplete.

## Current owners and scope

`initial_authority::InitialAuthorityHistory` distinguishes legacy initial Grant, accepted no-launch origin, and actual `PhaseExecutionHistory`. The phase variant contains the retained request/ExecutionReceipt, accepted history, installation receipt and actual original registration/provenance. Legacy renewal deliberately refuses this variant. Started currently refuses every accepted phase origin before its legacy path.

Current Stopped validates a retained Started request/receipt and the exact hot grant. It does not load a legacy InitialGrantDelivery. Its original-registration, guardian/pane, journal and historical expiry rules must remain intact, with phase origin acceptance made explicit. Granted-no-Started closure and renewal currently load the legacy OriginalGrant; closure's trusted `GrantedContext` contains a legacy GrantedLaunch. These are separate adaptation boundaries.

The first functioning profile should include Started, matching Stopped, and granted-no-Started closure. It advertises no renewal. The host guardian stops at the original independently sampled execution expiry/hard cap. A closed launch or verified physical stop continues to hold task, source, result and external-effect claims until their owners settle them. It is not ordinary success or failure.

## Portable contract proposal

Prefer a new `aq-phase-terminal-protocol` package, version 0.1.0, exactly depending on frozen `aq-admission =0.1.9`. Keep all existing admission 0.1.7/0.1.8/0.1.9 archives and module bytes unchanged. This avoids mutating a published Feature enum or implying that existing HostStopEvidenceV1 peers support phase history.

The bounded profile is `aq.phase-terminal.v1`. A generated schema, contract ArtifactRef, request/receipt fixtures and standalone extracted-package tests cover four finite document families:

- Started: exact phase authority reference plus the existing `started::Request` body.
- Stopped: the same authority reference plus the existing `stopped::Request` body.
- Granted-no-Started closure: the same authority reference plus the existing `granted_closure::Request` body.
- Historical readback: typed receipt identities and retained terminal receipts, with no delivery clock or live permission.

An authority reference identifies `PhaseExecutionV2`, the actual execution receipt digest and accepted receipt digest. Its binding/grant associations come from the original body and are checked against actual loaded history. Negotiation names the exact contract and finite `StartedStoppedClosureV1`/`OriginalLeaseNoRenewalV1` semantics. It conveys no authorization. Every request and acknowledgment is bounded to 64KiB; vectors remain those of the existing finite bodies. Changed operation-ID payloads or origin digests conflict. Unsupported peers refuse without a legacy fallback.

The existing Started/Stopped/closure DTOs are reused as bodies. The phase closure owner calls the existing Request's structural validation and validates its associations directly against real phase history. It must not construct a legacy GrantedContext to satisfy an older validator. Acknowledgments are historical facts, not permits.

The first server qualification can mount these documents only in explicit test routers. Intended future routes are `/v1/worker/profiled-v2/started`, `/stopped`, `/launch-closure` and a bounded terminal-history read. Existing legacy routes retain their phase refusal. Production mounting and actual worker advertisement follow independent qualification.

## Trusted internal authority view

Add a non-Serde borrowed view constructed only by `initial_authority`'s actual locked SQL loader. Each view retains its typed origin and actual receipt identity. Accessors return binding/grant, owner IDs, original operation/request identity, original versions, first issuance, hard deadline, actual material/installation records and original registration identity. Each value comes directly from its variant.

Legacy receipt identity remains explicitly the stored legacy `prepare` record and its actual canonical response identity. Phase identity is the actual state32 ExecutionReceipt kind/digest. A shared view does not convert either representation or synthesize a delivery clock. Historical material validation uses actual initial issuance time; retrieval expiry never becomes installation expiry.

No accessor grants current authority. Started separately checks current authenticated node/boot, exact runtime qualification and original incarnation, current open/hot grant, cancellation/revocation and original execution/hard expiry after waits. Stopped and no-Started closure preserve historical proof acceptance after expiry or retired launch eligibility, under an explicitly installed historical verifier and current authenticated/recovery policy.

## Paired owner behavior

Started records a qualified genuine Running observation, exact original grant/owner/manifest, prepared journal predecessor and original runtime incarnation. First admission requires a still-live grant. Exact receipt replay remains a historical acknowledgment and cannot restart or extend anything. A later cancellation/expiry never causes a new permission sample.

Stopped requires the matching actual Started record, exact guardian nonce/process birth, kernel boot/pid namespace, pane/server birth and qualified ECHILD proof. It records physical stop after expiry/cancellation without requiring current launch eligibility. It cannot release task/source/effects or mark a task successful.

A physical start whose first acknowledgment was lost can replay an already committed Started receipt. If no Started receipt was committed and the grant is no longer admissible, the qualified no-Started closure path must accept the appropriate actual host-stop/driver-intent proof instead of inventing Started. The existing suppression and host-stopped evidence shapes remain distinct. Missing or ambiguous local history stays held.

The phase closure context is a new internal version containing the typed phase receipt identity, original operation/request identity/versions/hard deadline, actual registration, prepared journal and verifier. It references the immutable state32 record rather than duplicating its potentially128KiB body into the64KiB context column. Legacy context bytes remain readable without rewriting. Historical readback rechecks actual referenced records and does not depend on a current worker launch allowlist.

Renewal, scoped material presence and result ingestion need an explicit origin audit. They must consume the typed common view or keep an explicit phase refusal. Enabling Started/Stopped must not silently enable another consumer merely because a generic hot-state predicate now passes. B1 result recording, B2 review/settlement, source release and executed checks remain separate qualification; no eligibility boolean is supplied by terminal receipts.

## Concrete mixed-writer boundary

Propose additive migration033, subject to root reservation. Metadata33 alone cannot fence an already-open older daemon. The database must reject phase terminal writes from old code while preserving legacy writes.

Add explicit origin kind/digest and bounded phase-envelope identity to Started and Stopped storage. Preserve the existing original body bytes/digests for their existing validators; retain the additional envelope bytes/digest separately so immutable ID replay also binds the new origin/negotiation. Charge these actual additional bytes in the existing global/per-node retained quotas. Missing current-profile columns, trigger or function refuses startup; no automatic repair.

Before accepting a phase terminal insert, a SQL trigger resolves the actual state32 grant and requires exact node/boot/attempt/grant/receipt associations plus a transaction-local writer marker bound to that receipt. The new owner sets the marker only after loading actual history and qualifying the request, using `set_config(...,true)` or equivalent SET LOCAL. A marker is not authorization and is never accepted from an HTTP body. It is not sticky pooled-session state.

An older writer omits the new origin and marker and is rejected. Its entire Started/Stopped transaction rolls back, including journal/hot changes. A new legacy path also cannot label a phase row as legacy. Direct database administrators remain trusted; this is a compatibility fence, not remote attestation or protection against a database owner.

Extend the closure SQL guard with a separate strict phase branch resolving the actual phase record and typed context. Keep its legacy branch unchanged. The terminal hot-state fence continues to prohibit resurrection or owner/grant replacement. No caller-provided `kind` or digest suffices without its actual owning row.

Migration installation must refuse any preexisting phase terminal records that cannot be qualified as having been produced under the new origin protocol. It must not silently relabel, delete or quarantine them. Existing state32 deployments are not assumed to contain such authorized records; the experimental issuer remained unmounted. An explicit operator recovery procedure would be a separate task if such data exists.

Qualification includes an old-style connection opened before migration, followed by attempted phase inserts after migration; refusal must leave hot/journal/terminal state unchanged. It also checks a fresh transaction on a pooled connection cannot inherit the previous marker, and wrong receipt markers refuse. Deployment excludes unsupported old writers even with this database defense; no mixed deployment is advertised until the exact retained old binary/profile is tested.

## Qualification and ownership proposal

| Area | Proposed ownership |
| --- | --- |
| New pure terminal contract, schemas, fixtures and archive | This lane; coordinate Cargo/package window first |
| `initial_authority.rs`, new `phase_terminal` module, narrow Started/Stopped/closure adaptations | This lane after design review |
| Migration033, Store/metadata/Docker registration and origin quota accounting | Reserve with root before edits; preserve SQL028–032 bytes |
| Worker phase grant journal/host/proof producer and exact runtime publication | Root/worker lane; no source overlap |
| Dedicated actual daemon/worker/host process probe | Coordinate after both endpoints freeze |
| Canonical task graph, production activation/deployment | Root; not implied by this plan |

Stage A qualifies the pure contract, typed internal view and database writer fence with phase terminal admission still disabled. Stage B qualifies the paired owning functions and no-Started closure in explicit test routers. Stage C joins the actual worker durable grant/host producer, then qualifies exact extracted artifacts and real local host proofs. Optional production route/startup changes follow a separate reviewed activation step; synthetic peers never become production registrations.

Tests cover exact origin/replay, missing/corrupt actual history, wrong node/boot/generation/grant/receipt, late exact acknowledgments, cancellation/expiry races, post-lock expiry, journal regression, receipt/hot/journal rollback, changed guardian/pane/kernel identity, false/missing ECHILD, closed-launch resurrection, bounded quota refusal, actual grant-unknown recovery and no-Started suppression/stop paths. Normal genuine Started→Stopped and pre-Started closure must both work before a capability is exposed. No renewal, provider calls, task completion, live notifications, source cleanup or no-double-billing promise is part of this slice.
