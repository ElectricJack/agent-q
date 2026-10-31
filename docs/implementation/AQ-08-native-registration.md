# AQ-08 native registration ownership inventory

The common execution_nodes state currently contains real incarnation identity,
capability IDs, worker version and a replacement-boot permit (`Registered` in
execution.rs). It is not an ExecutionSpec or attempt state. Native registration
will add an optional typed native record containing the exact independent
admission Register request and its server-owned qualification reference/revision.
Absent metadata retains existing serialized legacy bytes. No fake harness or
legacy execution capability is inserted to satisfy native admission.

## Decoder and writer inventory

- execution.rs registration(): decode bounded common identity plus optional
  native metadata. This helper alone confers no grant or namespace permission.
- active(): every legacy fixture offer/poll/accept/renew caller must refuse
  native registration before reading capabilities. Shared machine auth remains
  separate from both execution namespaces.
- execution_register(): refuse overwriting native metadata even for the same
  boot. Exact historical registration receipts may be inspected under current
  auth but never reinstall a legacy registration over a native owner.
- execution_reconcile(): refuse native records before changing replacement_boot;
  legacy local journal DTOs cannot authorize native incarnation replacement.
- execution_replace_incarnation(): legacy administrative replacement must refuse
  native records before writing a new Registered. Native replacement needs its
  own typed, qualified path and the common held-claim/verified-release fence.
- execution_result(): retains explicit legacy attempt decoding and can read the
  common identity for historical legacy result ownership; it cannot modify the
  native record or grant native execution. Late receipt handling remains scoped.
- AuthorityView compatibility/hot loaders extract current state.identity only;
  this stays the common node/boot fence. They never manufacture central sessions.
- intelligence inventory/resolver/feedback inspect identity under node locks;
  native capabilities/provisionability must come from exact accepted native
  metadata plus daemon-owned qualified-runtime policy, never arbitrary worker
  strings. Existing installed-only inventory behavior remains unchanged.

The only execution_nodes writers are the three registration/reconciliation/
replacement paths in execution.rs (register, replacement permit, replacement)
and the new native owner. Every native owner write locks node first and refuses
active foreign/mismatched incarnation claims. The optional metadata is strictly
bounded and immutable for a boot while any claim is held; changed operation
content conflicts, and exact replay never restores old incarnation authority.

## Native producer boundary

Qualified runtime configuration is owned by the daemon. Register can report only
actual public runtime/capability artifacts; acceptance requires the configured
exact qualification and supported adapter version. Poll does not grant work
merely because a capability was advertised. Placement still requires current
project/node/catalog/selection/source/material fences and the common task claim.
Native prepare and grant do not route through the fixture ExecutionSpec decoder.

An ungranted offer can expire before any guardian exists. Its release requires a
separate central NeverGranted provenance: immutable evidence that no grant was
ever issued, permanent closure against later prepare/grant, material revocation,
and actual source/effect quiescence. This does not claim physical guardian stop.
Migration013 is immutable; the distinct closure and genuine qualified host-start
producer/consumer seam require coordinated additive runtime storage/contracts.
Until those owners exist, neither unknown stop nor a journal revision authorizes
release, Running, terminal activity, or lifecycle-source registration.

The current writer candidate uses a separate append-only operation history in
migration019. Exact historical receipts return the original Register document
under current machine authentication and never reinstall the old metadata.
A new operation with identical semantic registration leaves current metadata
untouched; any metadata/incarnation change refuses while any common task claim
is held. The SQL guard also fences accidental direct writes and prohibits a
native-to-fixture conversion. Receipts are limited to1024 per node and32KiB
canonical request bytes (64KiB stored JSON ceiling); exhaustion refuses rather
than deleting replay history.

The complete configuration owner anchor is checked under SHARE lock, before
project/policy locks, so an older daemon cannot qualify a runtime removed by a
newer installed configuration. The installer uses configuration →project →policy
order and never takes node/attempt locks. Native callers take node first and
must fence configuration before taking project/policy locks. Historical receipt
readback does not require the old runtime to remain qualified.

The API candidate is POST `/v1/worker/profiled/register`, machine-listener only,
64KiB body ceiling, strict transport Register document; the echoed Register is
an acknowledgment and confers no grant or graph/source/terminal access. It is
not mounted until ordered migration018/019 registration is integrated. Tests
use explicit019 SQL while018 remains in the independent Started worktree.
The test runtime and qualification artifacts are synthetic; these tests do not
qualify an installed worker, host adapter, provider credential, or cold launch.
