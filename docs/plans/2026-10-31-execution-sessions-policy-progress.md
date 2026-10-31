# Execution, sessions and policy implementation progress

The design/implementation reconciliation is complete for the current three-area
scope. This checkpoint records continued implementation, not completion of the
three product areas. The [completion map](0002-execution-sessions-policy-completion.md)
and [task graph](0001-v2-milestones.md) retain the remaining acceptance gates.

Daemon source `c1d6597` (state24) passes778 workspace tests with all features enabled,
8 intentionally ignored, strict all-target/all-feature lint and formatting.
The build took93 seconds and test-process times sum to129.93 seconds on this shared
machine; this is an observed run, not a promised CI bound. Exact source hashes and
logs are in the [reviewed state24 integration manifest](../implementation/three-area-reviewed-state24-qualification.json).
This includes the historical result-owner read, credential issuance rotation-race
fix and pure review protocol. The prior [764-test state24 checkpoint](../implementation/three-area-state24-qualification.json)
and [717-test state21 checkpoint](../implementation/three-area-integration-qualification.json)
remain retained separately. Tests use the normal ordered migrations through024.

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

Additional components integrated after the full state24 checkpoint:

- [Admission0.1.6](../implementation/aq-admission-0.1.6-qualification.md) has43
  passing repository tests and43 passing independent extracted-package tests,
  with strict lint and formatting. It adds one unambiguous final startup payload
  descriptor and a Recorded-only result acknowledgment. Native consumption remains
  in progress; the database ledger is now integrated below.
- [Pre-offer Git I/O](../implementation/AQ-08-preoffer-io-qualification.json) has7
  passing focused tests, including an actual export/retry/fresh-retention test
  against retained aq-git/Git binaries. Fixed bounded chunk reads reopen members
  without following symlinks. Migration022 adds durable quota reservation, verified
  finalization and exact attempt linking. The current authenticated status/chunk endpoints and typed Source observation
  owner are now integrated through ordered migration025. Their
  [frozen qualification](../implementation/AQ-08-source-025.qualification.md) passes
  37 SQL/router/store tests and one metadata check, with one declared real-Git-peer
  ignore, strict lint and formatting. Full producer assembly remains open.
- Upstream keyed playbook admission is retained in aq-playbooks sourcea463e59:
  atomic run/receipt creation and protection against old writers. Its exact
  archive has158 all-feature test executions and strict lint/format checks.
  Central adoption now includes an explicitly mounted durable first bridge,
  actual message.send adapter, cancellation and protected outbox. A subsequent
  accepted-effect recovery fix is published as runner/Postgres/CLI0.1.2 at source
  `d3bd349`, with160 independent extracted-package test executions. It fixes a
  lost-reply recovery path that otherwise stranded an accepted human question in
  Blocked. AQ host adoption and broader adapters remain separate integration work;
  default startup is still open.
- Migration024 records immutable result reports and exact Stopped/outcome bindings,
  bounded correction heads and replay receipts. The ledger does not accept results,
  release holds, settle source/effects or complete graph tasks.

Continuing work:

- Native preparation/offer/grant assembly and actual worker consumer. The worker
  now verifies actual inline artifact/startup bytes and consumes native reporting,
  control and bounded source-download endpoints. The private material installer
  commits exact lease/operation/boot provenance and private bytes before returning
  a receipt. The latest worker client checkpoint `3bbf168` passes26 focused tests,
  strict lint and formatting. Transport refuses remote plaintext and ambient
  proxies and disables implicit retries. The exact0.1.7 control contract passes53
  independent extracted-package tests. These components do not yet form a durable
  execution driver: the [native journal/restart plan](../../../aq-worker/docs/implementation/WRK-08-native-journal-plan.md)
  specifies immutable operation writes, uncertain-launch holds and stop-and-preserve
  recovery. Source HTTP verification is not actual Git workspace hydration.
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
- Broader AQ playbook adapters and actual default host startup. The pure review
  protocol passes11 standalone tests. The optional human/source owner is now
  integrated as a candidate026 component:11 actual-PG tests plus2 current-retention
  checks, with strict lint/formatting. Its source peer is synthetic; actual native
  Git end-to-end qualification, ordered026, profile2 decision adapter and executed
  check ownership remain open. A human answer is not result eligibility/completion.
  PB04's 25ms latency target remains unmet and is not relaxed.

No live-provider qualification, deployment, external human notification or push
was performed. GATE-04/GATE-05 and broader v2 acceptance remain open.
