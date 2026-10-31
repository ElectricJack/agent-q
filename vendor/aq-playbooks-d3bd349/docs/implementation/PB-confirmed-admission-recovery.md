# Confirmed asynchronous admission recovery: source publication0.1.2

A Host may durably admit an operation and lose its response before the engine
records the admission handle. The engine then has Unknown/Blocked. An actual
owner lookup can confirm Accepted with the original handle before external/human
completion exists. Runner0.1.1 recorded the handle but only resumed a Blocked run
for Completed, skipping accepted→notify/join until a final completion arrived.
If accepted was responsible for notifying the human, that could stall forever.

The narrow0.1.2 fix is in `record_result`: clear the obsolete Unknown intent reason
when admission is confirmed; change Blocked to Waiting only when that run's
pending Effect names the exact Accepted intent (or the existing Completed case).
The next normal transition evaluates the original accepted branch. It does not
invent completion, change Authority/input/handle, dispatch a second operation,
resume an unrelated pending operation or revive Cancelling/terminal work.
Existing host authorization and execution revocations still guard recovery.

Versions: runner, Postgres adapter and CLI become0.1.2, with exact workspace
dependency pins. Model/compiler stay0.1.0 unchanged. Postgres implementation and
SQL migrations are unchanged; its bump reflects the new runner dependency. Old
0.1.0/0.1.1 source archives remain immutable. Consumers must publish a new exact
source closure and bump their dependent package/version pins rather than mutate
same-version bytes. No registry publication occurs in this work.

The new regression restarts after Unknown, confirms the same owner Accepted
handle, requires Waiting *before* a completion event, executes accepted→join,
checks no second dispatch/root or changed Authority, then completes through the
exact trusted event and deduplicates replay. A second case proves revocation
refuses reconciliation and Accepted cannot uncancel work. The first test fails
under the unchanged old1.1 implementation (before-fix commitb52ba85) and passes
with this fix. Frozen AQ SQL/protected-engine evidence separately reproduces the
same defect; consumer qualification remains required after adopting these pins.

This does not automatically repair already-serialized1.1 states stranded as
Blocked+Accepted: Accepted is no longer in that old recovery queue. No legacy
state repair, online mixed-version adoption or migration is qualified. AQ's
qualified old catalog1 used synchronous message.send; new async catalog2 must
start in a deliberately new namespace under the qualified1.2 producer. Existing
namespace origins are not restamped. Normal ownership/protected-storage fences
and immutable keyed admission receipts remain unchanged.

This is a recovery correctness fix, not PB04 performance closure. Whole-namespace
snapshot costs and the25ms budget remain open. There are no provider, remote Git,
AQ daemon or Discord calls in the standalone engine tests; PostgreSQL tests use
only the explicit scripts/test-db disposable container. Exact source/archive
hashes and extracted-only qualification are retained in the adjacent evidence
JSON after source freeze.
