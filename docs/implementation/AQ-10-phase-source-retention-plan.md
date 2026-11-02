# AQ-10 phase source-retention HTTP qualification plan

Test-only follow-up to B1 `6eb031e`. No new DDL, public route, daemon-to-worker
library dependency, legacy Runtime conversion or general source framework.

Reuse existing `source_retention::Runtime` worker HTTP begin/chunk/status/finalize,
actual common SourceRetention authority, immutable `TransferManifest` and
`CentralRetentionReceipt`, `load_confirmed_locked` and
`result_retention::inspect_current`. There is no legacy-origin or legacy
HostStop feature gate in these paths on source inspection; actual qualification
must prove that assertion for both nominal phase families.

A narrow test-helper seam allows caller-prepared `presence::Case` to enter the
existing Frozen/Refresh fixture **before offer admission**. Source repository,
base, policy and retained handle are fixed from real Git owners before issuance;
no frozen offer/manifest/binding is rewritten afterward. Existing fixture entry
points continue unchanged. Actual phase issuer and terminal owners produce the
retained records; host/proof/material/Graph facts remain explicitly synthetic.

Two explicit opt-in, ignored-by-default cases run serially with pinned aq-git and
Git executable identities, private disposable roots/DBs and only a tiny local
seed. Reuse the coordinator/worker transfer pattern already qualified in aq-git
`tests/scenario.rs` and server `native_initial_grant/published/actual_git.rs`:

1. Register/sync a private central repository; retain its actual base checkpoint.
   Configure the existing phase fixture to that actual source before offering.
2. Start a separate worker-mode Git owner; import the actual central base export
   through the existing immutable manifest/bundle format and create its workspace.
   This fixture bootstrap is distinct from the checkpoint upload under test.
3. Issue/record actual nominal Started/Stopped records (synthetic host evidence),
   make a labelled fixture edit and use actual Git checkpoint/export to retain the
   changed source. No agent success or physical-stop qualification is inferred.
4. Upload exact canonical manifest+bundle through the actual worker router using
   immutable begin/chunk/finalize operation IDs and the original attempt/session/
   generation/spec/source identity. Exact retries/status recover the same receipt;
   partial transport cannot produce retention. Changed identity/bytes are refused.
5. Pin the real returned `receipt_id` in unchanged B1 `ResultReport`, reload actual
   result and confirmed import owners, then verify a current actual
   `retention.inspect` observation outside SQL locks. No caller receipt projection
   substitutes for the actual stored import acknowledgment.
6. Stop/remove the disposable worker Git volume; verify central changed bytes and
   exact current retained transfer survive. Explicitly release the central transfer
   and require current retention observation refusal while historical owner facts
   remain distinguishable. Task/grant/hold/effects/release/Graph facts must not gain
   authority from transfer or report recording.

The fixture is not a physical native-agent run. In particular it does not claim
that synthetic terminal DTOs establish kernel stop or that the new native0.5
producer has executed this sequence. Its role is to close the real server/Git
retention join before that producer's separately qualified composition.

Root owns build-slot authorization. Runner owns coordination of the single cold
Git lane; default Git minimum-free policy is retained with no override. Source
and no-run checks may proceed first. Actual Git owners run only after Root's
maintenance/pin handoff; preflight refusal is reported, not hidden by lowering
limits. Known previous proof used aq-git `c2ff26f5…` and Git `eb54b535…`, whose
actual reported version is2.43.0, not2.56. New evidence must identify exact chosen
executables/version and cannot inherit a version label from an older test name.

## Exact request-credential correction

Source audit found the preexisting HTTP path authenticated a Node snapshot, then
only checked UUID/revocation after waits; a rotated credential could outlive its
admission snapshot. Root approved this as a concrete pilot fix. Existing opaque
`AuthenticatedNode` is retained and checked by `lock_current_node` before the
unchanged trusted UUID SourceRetention authority. The machine-only wrapper is
used at every HTTP transaction boundary; internal confirmed reads/admin UUID
owners remain reusable without HTTP credentials. Lock order stays Node→attempt→
transfer. Legacy base HTTP reads gain the same current-credential fence.

Finalization now reacquires the existing predispatch transaction after file
validation **before** deciding either rejection or dispatch. Known invalid bytes
can only be rejected under the original still-current principal; uncertain
imports retain their dispatch/bytes/charge protection. Physical import results,
including failed replies, remain pending until the post-I/O principal fence.
Old credentials are refused; a new current credential for the same node can
resume/replay the identical admitted transfer and actual import operation. No
credential becomes persisted authority and no DTO/schema changes.

Two focused default tests prove an observed node-lock wait followed by rotation
cannot create an intent, and rotation during a held import reply cannot commit a
receipt or discard uncertain bytes; the current replacement key recovers exactly
the same import operation. The latter uses an explicitly synthetic socket owner
for its auth/recovery boundary. The two opt-in real Git phase cases independently
qualify actual retained bytes/receipts. The existing slow-import revocation case
now expects current-auth denial even when the held peer returns an invalid reply.
