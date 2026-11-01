# State29 inventory and scoped-presence storage prerequisite

This checkpoint registers migration029 and provides the authenticated
InventoryV2 library owner, immutable operation retention and persistent semantic
compatibility barriers. It does not install the new machine HTTP routes, the
per-attempt presence report owner or renewal consumption; those remain the next
slice. The legacy newer-negative rule remains unchanged. Worker physical
presence qualification remains separate.

The owner requires the exact typed qualification Definition and all referenced
runtime/adapter/evidence bodies, current admitted runtime/boot, installed global
configuration and registry. Actual protected private-file reads occur outside
SQL on two dedicated workers (two queued/four owned jobs); cancellation does not
release a queued/running job's permit. Final authentication/registry/runtime and
observation deadlines are checked after lock/quota waits. It records actual
normalized snapshot digests and immutable original acknowledgements.

An empty provisionable_only list makes no worker credential assertion: the daemon
derives only actually backed, configured, node-authorized server-material
bindings. A nonempty list restricts exact profile/revisions; unauthorized or
unbacked requested entries refuse instead of widening. All produced bindings
remain unavailable for globally installed semantics, with actual provisionability
proofs. No private bytes or path enters the response. This clarifies the existing
empty-list structural allowance; frozen0.1.9 DTO/schema bytes are unchanged.

Compatible v2 updates retain a node semantic epoch. A new accepted legacy
inventory, boot/runtime or qualification incompatibility advances the epoch;
restoration advances again, preventing pre-barrier Present from reviving. Exact
v1/v2 operation replay neither reinstalls snapshots nor moves the barrier.
Per-attempt sequence/negative heads are distinct storage, never advanced by
ordinary compatible node-wide updates.

Operation rows have immutable canonical request/receipt bytes and SHA256 checks.
SQL derives head revisions/epochs and bounds incoming uncompressed bytes plus
declared1024-byte overhead:65536 operations/256MiB globally,4096/16MiB per node,
4096 scoped heads globally/128 per node. These are finite retention admission
limits, not resident-memory or continuous-fleet acceptance. No unknown history or
negative fence is pruned for new authority.

Focused qualification covers11 distinct disposable-PG tests: six inventory owner
cases, two actual state28→29/fresh-state migration cases, two existing schema28
regressions and one existing cold native inventory regression. Strict server
all-target/all-feature lint and format pass. The per-attempt table guards and
negative-consumption behavior require their later actual report-owner matrix;
this prerequisite makes no completed presence/renewal claim.
