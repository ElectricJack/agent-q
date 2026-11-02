# AQ-10 B1 phase result-origin bridge

Base: accepted refresh/terminal union `e510e7d`. Migration0038 is reassigned by
Root to execution result origins; the unmerged wait-owner38 candidate remains
isolated and must later use0039 or newer. Frozen migrations1–37 stay unchanged.

The first slice records claims from actual Frozen and RegistryRefresh phase
executions through the existing report/head/outcome owner. It does not evaluate
eligibility, release a task or resource, or complete Graph work.

## Typed historical owner

An internal non-Serde `ResultOrigin` dispatches actual retained Legacy,
PhaseExecutionV2 or PhaseRefreshExecutionV1 history. For phase families, decode the
correct nominal outer Started/Stopped requests and acknowledgments, validate them
against actual execution/accepted/proposal/registration history and Prepared
checkpoint, and verify exact canonical inner/outer/proof bytes, receipt digests,
original runtime/qualification and retained terminal qualification. Nominal phase
proof admission uses the actual phase terminal owner and its closed terminal
qualification, not the legacy transport HostStopEvidenceV1 flag. Accepted but
unissued work is unsupported. No legacy Prepare/Started/Stopped row is invented.

Existing shared `ResultReport.stop` and `result_record::Receipt.stopped_receipt`
keep their published meaning: the actual shared evidence/acknowledgment nested in
the validated phase wrapper. Separate server-owned pins retain the nominal outer
origin, so no old DTO is reinterpreted as phase authority. Historical reads use
the same verifier as writes. Current machine authentication remains mandatory
for recording; original retained history grants no new execution authority.

## Storage delta

Extend immutable `profiled_result_reports` with three bounded owner-derived pins:

- `result_origin_kind`: `legacy_prepare_v1`, `phase_execution_v2`, or
  `phase_refresh_execution_v1`; existing rows default to legacy.
- `origin_execution_receipt_digest`: actual issued phase receipt digest, otherwise
  NULL.
- `origin_stopped_ack_bytes_digest`: SHA256 of the actual canonical outer phase
  Stopped acknowledgment, otherwise NULL. This is explicitly a storage-byte digest,
  not the domain-separated protocol receipt digest or a signature.

Add a generated phase-attempt FK where useful to pin the actual issuance owner.
Schema constraints and a qualified transaction-local writer fence bind the exact
node/attempt/execution/stopped origin and reject cross-profile/mixed metadata.
Refuse preexisting unqualified phase-report corruption on migration rather than
backfilling it as legacy. Existing report quotas/correction/atomic-head guards
remain. Origin metadata is separately finite (64 reports, bounded kind/two hashes).

Replace only the behavior of the existing shared phase refusal trigger function:
allow actual qualified phase report admission while keeping phase
`profiled_verified_releases` unconditionally closed. No release marker is minted.
The38 startup guard pins changed/new definitions, keys/actions/nullability/trigger
state and retained origin consistency. The old33 profile guard remains intact for
prefix databases; no frozen migration is edited.

## Verification

Actual disposable Frozen and Refresh issuers and terminal owners must produce the
source records used by tests. Qualify Recorded admission, exact replay after
expiry/cancellation/configuration retirement, correction race/head/quota,
historical read, changed origin/profile/digest/proof/qualification refusal,
credential rotation after an observed lock wait, missing issuance/stop, SQL bypass
and atomic rollback. Preserve phase release refusal and existing legacy result
regressions. No kernel/provider/host run is inferred from synthetic proof DTOs.

Human Results discovery remains explicitly legacy-only until the revised reader
and coverage contract have paired tests. Actual source upload, agent result claim
surface, check execution, B2 eligibility and C release/Graph delivery are separate
bounded integrations, not new general policy/workflow frameworks.
