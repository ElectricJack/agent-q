# Execution, sessions and policy implementation progress

The design/implementation reconciliation is complete for the current three-area
scope. This checkpoint records continued implementation, not completion of the
three product areas. The [completion map](0002-execution-sessions-policy-completion.md)
and [task graph](0001-v2-milestones.md) retain the remaining acceptance gates.

Daemon source `7c70651` (state24) passes764 workspace tests with all features enabled,
8 intentionally ignored, strict all-target/all-feature lint and formatting.
The build took98 seconds and test-process times sum to122.31 seconds on this shared
machine; this is an observed run, not a promised CI bound. Exact source hashes and
logs are in the [state24 integration manifest](../implementation/three-area-state24-qualification.json).
The prior [717-test state21 checkpoint](../implementation/three-area-integration-qualification.json)
remains retained separately. Tests use the normal ordered migrations through024.

Implemented components in that checkpoint:

- Actual post-Started stop admission: original physical guardian/pane binding,
  immutable receipt, journal and authority transition commit together. Expiry,
  cancellation and retired launch configuration do not strand a valid stop;
  task/source/effect holds remain. Observed SQL-lock races test current credentials
  and recording time. Earlier standalone source6da51c8 passed694/6ignored.
- Qualified context projection/counting: strips ungranted metadata, preserves
  original observation freshness and exact scope, and requires explicit model/
  renderer capacity/counting facts. Missing facts refuse; byte bounds are not
  presented as tokenizer or context-window measurements.
- Versioned expected-outcome definition: task/source/context revisions, acceptance
  criteria, required validators and explicit human/policy review are pinned before
  launch. [Exact0.1.5 archive qualification](../implementation/AQ-10-outcome-contract-qualification.md)
  passes34 pure tests independently. It does not accept a result or complete a task.
- [Playbook host foundation](../implementation/AQ-12.foundation.md): exact source
  dependency, protected namespace ownership and bounded dedicated thread/queues.
  A blocked backend does not block actual daemon renew/cancel HTTP. No production
  commands or startup mount are implied by this foundation.

Additional components now included in the state24 assembled source:

- [Admission0.1.6](../implementation/aq-admission-0.1.6-qualification.md) has43
  passing repository tests and43 passing independent extracted-package tests,
  with strict lint and formatting. It adds one unambiguous final startup payload
  descriptor and a Recorded-only result acknowledgment. Native consumption remains
  in progress; the database ledger is now integrated below.
- [Pre-offer Git I/O](../implementation/AQ-08-preoffer-io-qualification.json) has7
  passing focused tests, including an actual export/retry/fresh-retention test
  against retained aq-git/Git binaries. Fixed bounded chunk reads reopen members
  without following symlinks. Migration022 adds durable quota reservation, verified
  finalization and exact attempt linking. Native authenticated download and full
  producer assembly still require integration.
- Upstream keyed playbook admission is retained in aq-playbooks sourcea463e59:
  atomic run/receipt creation and protection against old writers. Its exact
  archive has158 all-feature test executions and strict lint/format checks.
  Central adoption now includes an explicitly mounted durable first bridge,
  actual message.send adapter, cancellation and protected outbox. Default startup
  configuration and broader command adapters remain in progress.
- Migration024 records immutable result reports and exact Stopped/outcome bindings,
  bounded correction heads and replay receipts. The ledger does not accept results,
  release holds, settle source/effects or complete graph tasks.

Continuing work:

- Native preparation/offer/grant assembly and actual worker consumer. The worker
  now verifies actual inline artifact/startup bytes and consumes the existing
  native reporting/download HTTP endpoints (12 focused tests, strict lint/fmt).
  Durable native journal, installation and runnable control integration remain.
  The pinned Codex CLI interface inspection is non-authorizing; standalone
  guardian-owned executor and provider/model compatibility still need qualification.
  Pre-offer Git export must precede attempt/spec-bound transport wrapping. Final rendered startup
  bytes must be pinned and counted exactly, without duplicate context delivery.
- Independent result eligibility evaluation and actual source/effect
  settlement before claim release or expected-version graph completion.
- Bounded terminal tail qualification, followed by renderer/full baseline and
  snapshot/live barriers. Negative candidate14354e7 confirmed that a sole SIGSTOPped
  tmux control observer can block the harness write despite pause-after. Input was
  Applied and the guardian remained Running. The guardian-owned reader lease
  candidate `cfe2c33` passes 303 normal tests
  (36 ignored) and 29 actual-host tests, including independent cutoff and continued
  harness progress. Its dense-packet tail regression remains negative at that
  checkpoint. The subsequent bounded incremental parser passes30 actual host and3
  actual bridge tests. Earlier unchanged crash-fixture marker timeouts remain
  recorded as unresolved negatives. The combined host source `3825480` passes
  307 normal tests, 39 ignored, 32 actual host
  and 3 actual bridge tests, with strict lint/formatting. Full rendering/baseline
  and native runtime adoption remain open.
- Broader AQ playbook adapters and actual default host startup. Exact human review
  subjects and executed check ownership are the next separate design/implementation
  slice; a human answer is not a result eligibility or completion receipt.
  PB04's 25ms latency target remains unmet and is not relaxed.

No live-provider qualification, deployment, external human notification or push
was performed. GATE-04/GATE-05 and broader v2 acceptance remain open.
