# Scoped material presence server owner checkpoint

This checkpoint adds actual authenticated machine inventory/report routes and a
same-live-grant renewal consumer to the frozen admission0.1.9 contract. It does
not complete AQ08 or qualify physical worker observations/provider execution.
The implementation is based on integrated state30 and adds state31; migration029,
030 and every public admission0.1.9 file retain their original bytes.

The machine routes accept raw bounded frozen InventoryV2/PresenceReport JSON and
return immutable InventoryReceipt/PresenceReceipt JSON, with no-store and no new
execution clock. Exact report replay requires current machine authentication but
never changes the latest head or extends its deadline. New Present requires the
actual current grant revision, retained installation receipt, exact owner/lease,
registered runtime/qualification and current registry; expiry is bounded by the
actual immutable authority issuance plus qualified maximum age. Delayed positives
cannot become fresh by HTTP receipt. Newer same-grant negatives may survive a
renewal race; expired negatives become unknown without fallback to older Present.

Renewal still checks current project/principal/catalog/registry, actual private
backing fingerprint, task/source observations, current runtime/harness/model,
provider negative/cooldown and original hard deadline. Scoped presence replaces
only the old installed-material exception for a fresh negotiated V2 inventory.
Legacy behavior remains separate. Reports do not mutate authority, owner journals,
source/effects, claims, graph state or retrieval leases.

State31 stores a deterministic normalized binding set on the semantic head and
replaces the SQL guard additively. It strips observation/expiry clocks, including
preparation clocks, and sorts actual binding objects. A real binding-set change
is a conservative global barrier; remove→restore requires a new private read.
Regular same-set refreshes and per-attempt reports do not advance the epoch.
The migration transactionally backfills matching current heads and has a tested
state30 collision/rollback/retry path. State30 test final-version assertions now
use the current constant; original intermediate rollback assertions remain exact.

The crate-private current_inventory_locked view validates actual normalized
snapshot revision/digest, retained operation, runtime/qualification and freshness.
Its historical inventory_epoch_locked companion validates retained inventory
provenance without supplying freshness, allowing phase initial Grant to compare
sealed and current epochs without consuming same-grant presence.

Qualification uses actual daemon SQL/router/artifact/material/offer/Accept/Grant
owners with explicitly synthetic worker boot/read observations. One public
placement slot is exercised. Simultaneous-attempt storage/owner and sustained
control-lane load remain followup qualification; no multi-slot/scalability or
remote filesystem attestation claim is made. Real worker production/adoption is
owned by the worker lane. No provider calls or external notifications occur.
