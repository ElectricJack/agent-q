# Composed execution recovery qualification

This is the next qualification plan after the normal actual-Codex scenario
reaches confirmed Graph completion. It does not report a passing recovery run.
Keep the finite local provider and simulated fixture Owner decision explicitly
labelled. A real external provider, human approval and canonical deployment are
separate qualifications.

## Reuse the existing owners

Worker `InputPipeline::replay_result_report` in
`crates/aq-worker/src/native/result/report.rs` already resends retained report
history. It does not recreate execution authority. The ordinary native runner
refuses to launch into an existing journal; a second `run_once` is not a recovery
mechanism. Prefer a bounded test-support hook around the existing fixture
executor to replay once in the same retained pipeline after a lost result
acknowledgment. This would establish in-process transport-loss recovery only.
A process restart/reopen claim needs a separately qualified history-only path.

The server's `graph_mutations.rs` test
`actual_qualified_graph_delivery_recovers_lost_receipt_without_duplicate_history`
already forwards to real Graph and loses the response. Reuse that fault boundary
for settlement instead of replacing Graph with a synthetic response peer.

The `phase_source_retention/case.rs` real-Git case already removes the worker
root and verifies central exports. Its launch/stop evidence is synthetic.
Apply the same central-source independence check to the actual stopped worker.

## One additional actual attempt

1. Forward the first exact result submission through the actual AQ router. After
   B1 commits, lose only its outbound acknowledgment. Retain the original worker
   journal, report bytes, report/operation/source IDs and absent local receipt.
   Confirm physical stop/ECHILD, paired lifecycle acknowledgments, central source
   retention, one recorded report and an uncompleted Graph task. Replay the saved
   report once through the existing owner. Require the same bytes and IDs, the
   original receipt, one report/head and no second harness launch or source
   recapture. Do not renew the execution grant or extend its deadline.
2. Before initial project setup, configure a bounded transparent forwarding gate
   to the pinned real Graph service. Lose exactly the first successful completion
   response after Graph commits. Reconcile the original completion operation
   through the production owner. Assert identical mutation payload,
   idempotency key and expected version across delivery attempts, and independently
   inspect real Graph task/history for exactly one transition. Do not read a new
   version and issue a replacement mutation. Preserve a single completion,
   release and outbox association.
3. Once physical stop and central source custody are confirmed, drain the owned
   worker/Git processes and retire the disposable worker data. Verify current
   central retention and export the exact tree/blob bytes through central aq-git
   without consulting the worker's private-state handle. Settlement must continue
   to use the original retained source identity after that retirement.

Before coding, choose the exact sequencing of retirement and Graph delivery so
the fixture can still observe its owned cleanup and the public central source
proof. Keep fault counts finite and preserve every failed attempt. No new
general recovery framework, public command, dependency or service is implied.

## Remaining product refusals

The combined positive/loss case cannot establish cancellation races, stale task
revisions or uncertain physical writers. Preserve the existing component tests
and add the missing composed refusal cases separately: cancellation before and
after eligibility, changed real task revision before completion, and uncertain
physical state remaining held. Do not mutate an already completed positive
world and label it proof of those earlier boundaries.

GATE-04 and AQ-10 stay open until their acceptance evidence is complete. Existing
component tests, a normal happy path and the proposed in-process replay each
support only their stated scope.
