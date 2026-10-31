# Historical result-owner read seam

`profiled_execution::results::load_locked(state, tx, report_id)` reads actual
retained B1 owner records. It validates stored report bytes/digest,0.1.6 receipt,
Started/Stopped/proof bytes and relationships, actual frozen offer/spec/manifest,
retained outcome body/receipt/linkage, and the current report head under common
node/attempt authority locks. Recording behavior is unchanged.

The returned RecordedResult is a transient Rust owner view, not a serializable
fresh authorization permit. Caller authenticates/project-scopes independently
and keeps the same transaction when relying on the current-head snapshot. It
works after grant expiry, node revocation or launch allowlist retirement without
requiring a current node credential. is_current_report_head is a snapshot fact,
not result eligibility, source retention, release permission or task completion.
Report source/effect IDs remain claims until their actual owners resolve them.

All review creation/replay/resolution writers will use one order: review budget
rows, common node/attempt authority, result head/source-owner rows, immutable
subject, decision row, current user/project-policy rows, final receipt/outbox.
Read immutable decision/subject identity first without a row lock only to discover
the correct report/attempt, then revalidate under these locks. Never lock decision
then acquire attempt authority while another path takes the inverse order.
B1 correction only takes common authority/head locks and never waits for review
budgets/decisions. Tests must keep correction/resolve/replay races explicit.

Ten actual disposable PG result tests pass, including the original seven B1
cases and three new owner-read cases: historical/revoked/retired-policy reads and
superseded head, report mutation with recomputed stored digest, and receipt or
stored spec corruption. Corruption fixtures explicitly disable immutable UPDATE
triggers to exercise deeper read verification. No native provider/physical
source/check/human approval qualification is inferred from these SQL fixtures.
