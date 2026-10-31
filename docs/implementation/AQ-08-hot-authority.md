# AQ-08 bounded hot authority (migration 013 candidate)

The qualified migration 010 bridge remains the compatibility fallback. Its full
ProfiledState contains the immutable launch manifest, so using it for every
renewal and graph or terminal request repeatedly decodes and hashes large JSON.
Migration 013 adds a compact mutable authority row without changing 010.
It is registered after the qualified terminal migration 012.

## Sole current authority

A profiled_authority_hot row is keyed by the existing attempt foreign key. It
contains only current status, grant identity and expiry, immutable execution
hard deadline, exact guardian/launch/reservation identities, cancellation and
revocation state, and bounded stop/source-retention/effect receipt references.
The full existing state remains a historical compatibility snapshot. Once a hot
row exists, SQL guards reject changing native current authority through that
snapshot; a stale compatibility state cannot reactivate, renew or release it.

The common execution_attempts task claim remains authoritative for both kinds.
Only the verified release producer can clear holds_task, in the same transaction
as the hot terminal state. Typed receipt references alone do not establish host
stop, source retention, or effect quiescence. Missing or unknown proof holds the
claim. Inserts and updates enforce raw, uncompressed payload and receipt caps.
SQL derives all revisions from OLD and actual changed fields: a no-op preserves
revisions, renewal changes grant/record revision, and eligibility changes advance
authority revision. Callers cannot choose or decrease these revisions.

## Loading and immutable verification

AuthorityView keeps node -> attempt -> hot authority lock order, followed by
scoped authorization and consumer rows. It samples eligibility time after lock
waits. A missing hot row follows the existing fully verified migration 010 path.
A present hot row supplies all current mutable authority; immutable bindings
come from the sealed launch and its linked selection/decision records.

A process-local verified immutable cache may retain only compact extracted
bindings and digests. Keys include exact launch, manifest, spec and selection
identities/digests. It retains no mutable grant, role, credential health, deadline
extension or provider availability. Both entry count and accounted byte totals
are bounded; oversized entries are verified without admission to the cache.
Every sealed database row trusted by a hit must have an immutable update/delete
guard. A mismatch refuses authority rather than falling back to unverified JSON.

## Required qualification before adoption

Tests must cover cold verification, equivalent cache hits, changed sealed data
and digest refusal, revoked and renewed authority after a cache hit, migration
010 fallback without a hot row, forbidden compatibility updates after adoption,
SQL-derived revisions on exact replay, raw-size refusal, mixed-kind task claim
exclusion, and atomic verified release. Existing source/graph/terminal consumers
continue to use the same AuthorityView and cannot construct profiled authority.

## Current implementation boundary

The common loader now selects hot authority for adopted rows. Each cache hit
locks current node, attempt and hot row, then fetches only immutable identity and
digest columns under sealed-row guards. A cold miss retains full existing
verification. Historical source-retention reads without an AppState cache handle
perform that bounded cold verification and still use the current hot projection.
Unadopted bridge rows keep the qualified fallback; no old active grant receives
an invented guardian identity during adoption.

The cache retains at most 1024 entries, 16 MiB declared logical bytes and 64 KiB
logical bytes per entry. Accounting includes serialized facts, two serialized
key copies and a fixed bookkeeping allowance. It is not a measured allocator or
RSS bound: container overhead and concurrent request clones require extra memory.
The bounded synthetic 1000-attempt test measures second-pass hit ratio; it is not
end-to-end throughput or host/provider capacity qualification.

Adopted claims require an immutable producer-only verified-release record before
SQL can clear holds_task. No HTTP DTO writes that record and no producer is
installed yet: stop/source/effect projections alone leave the claim held. A
future internal release owner must authenticate the qualified exact host journal
and verify the actual central source/effect receipts before inserting it in the
same transaction as claim release. This is an explicit incomplete integration
boundary, not a proof inferred from record shape.

## Monotonic execution and release

SQL freezes an issued grant ID and binding, durable owner identity, cancellation
identity and known-stop receipt. Revocation cannot be removed. Finished/declined
status is final; cancelling/unknown cannot return to offered/preparing/granted.
The first grant requires Preparing with its durable owner; expiry renewal keeps
the same grant and is accepted only while Granted. Known stopped requires a
non-live status and owner, and cannot revert to running/unknown or change proof.

Granted may initially retain Stop::Unknown while the worker receives its grant
and bootstrap graph credential. The daemon does not invent physical Running
from issuance or a journal revision. TerminalActive still requires a genuine
qualified host-start observation; that producer/consumer seam remains separate.

Verified-release insertion requires a terminal known stop, exact owner and
complete source/effect projections with matching receipt digests, in addition to
the internal producer's actual receipt verification. A released common claim
cannot be reacquired. AuthorityView retains the freshly read holds_task value;
active_at refuses a released claim even under historical inspection. The focused
corruption test deliberately bypasses a hot trigger after release to write a
Granted row: both active loading and historical active_at still refuse it.
