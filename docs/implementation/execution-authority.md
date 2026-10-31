# Durable execution authority — first distributed slice

This implementation adds artifact-free deterministic agent execution to the existing daemon. It does not introduce a production graph scheduler, real model harness, source publication or task-completion policy. The implementation is in `crates/aq-server/src/execution.rs`, its authenticated HTTP adapter, and migration `0002_execution.sql`.

## Authority and persistence

The daemon stores registered worker incarnations, immutable offers and resolved specification digests, grants with absolute deadlines, ordered observations, cancellation directives, immutable report bodies and replay receipts in PostgreSQL. A node row lock serializes its transitions; a partial unique task index prevents competing unresolved attempts, including assignments on different nodes. Transactions commit authority and its replay receipt together before returning a response. No HTTP call or process work runs inside a transaction.

An acceptance or renewal replay returns the original grant sequence with only its remaining lifetime. It never starts another lifetime. A stopped process cannot receive another grant. Expiry removes execution authority, but does not establish that a process stopped or release an unresolved task claim. Pending external operations continue to hold the claim even after a stopped report.

Reports bind the original node/boot, attempt/session/generation and resolved specification. Each report body and admission disposition is retained independently of the current attempt projection. A correction must name its predecessor and gets its own immutable ID; changed bytes under an existing ID are rejected. Accepted evidence, cancelled evidence, stale evidence and rejected stop claims are distinct dispositions. An accepted fixture report is not graph task completion. Artifact handoff and reports with source artifacts explicitly refuse unsupported capability until a retained-source adapter exists.

Cancellation and report admission share the same transaction ordering. A result already accepted cannot be retroactively cancelled. Repeated cancellation operations share one worker directive; an unaccepted offer is declined without queuing a directive for a process the worker does not know. Directive receipt is separate from confirmed stop. Unknown writers remain held.

## Incarnation recovery

A direct registration with a different boot ID is refused. An authenticated reconciliation can authorize registration of that exact new boot only when **all centrally known prior writers** are confirmed stopped or were never granted authority. The submitted inventory is not the source of that proof. Registration atomically rechecks the proof, consumes the permit and retires old unaccepted offers. A grant accepted between reconciliation and registration prevents replacement. An empty inventory and an expired lease cannot hide a writer. The explicit owner replacement endpoint follows the same stopped-writer rule.

Old-boot reports can still be durably retained using the original binding; they cannot acquire current execution authority. Reconciliation never renews a deadline. Live process reattachment, descendant discovery and uncertain launch resolution remain worker recovery responsibilities.

## Public surface and compatibility

Foundation `/v1/worker/*` retains its meaning. Independently versioned `/v2/worker/*` routes use the public `aq-worker-protocol` crate vendored from an immutable standalone package. Requests are bounded, authenticated with the existing node credential, and checked against the body node identity. Administrative inspection, cancellation and replacement use the existing user/owner authorization and CSRF boundary. Worker routes remain on the worker listener.

The fixture scheduling route requires an owner and development authentication; it checks offer expiry and the sealed input digest. Maintenance blocks new acceptance while leaving renewal and result delivery available. Persistent state format is now 2. Format 1 migrates forward; old format-1 servers refuse the newer state. Existing updater backup/restore compatibility applies; a binary rollback alone is insufficient after the migration.

## Validation

PostgreSQL integration tests in `tests/execution.rs` exercise grant and receipt replay across reconnect, competing claims, stale sequences, cancellation ordering, expiry with unknown writers, pending operations, observation monotonicity, safe boot replacement, omitted-writer/racing-acceptance recovery, immutable report corrections, cancellation coalescing, cancellation before polling, bearer identity binding, listener separation, sealed fixture inputs, owner permissions and production-mode refusal. Store tests derive the future-format case from the current version rather than assuming format 2 is unsupported.

The extracted daemon workspace passes 235 tests (including 19 PostgreSQL execution tests), formatting and strict Clippy with protocol 0.1.1. Final commands, exact source inventory and retained outputs are recorded in `execution-validation.json` and its logs. Worker image and real HTTP smoke evidence is recorded separately by the integration qualification. Component unit tests alone do not close a product gate.

## Remaining scope

This slice does not qualify fleet placement/fairness, graph context and completion, real agent credentials, Git workspaces and retained source, playbook host adapters, messaging, dashboards, composite upgrades, or the 500-agent capacity target. Polling currently bounds its candidate rows before resource-fit filtering; incompatible earlier offers can delay compatible later offers. Production placement requires an explicit fairness/capacity design. Journal/process tests and fixture-image smoke are not proof of arbitrary descendant ownership, host power loss or control latency under transfer load. WRK-04 and GATE-03 remain open until their full failure matrix passes.
