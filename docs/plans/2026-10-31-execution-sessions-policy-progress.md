# Execution, sessions and policy implementation progress

The design/implementation reconciliation is complete for the current three-area
scope. This checkpoint records continued implementation, not completion of the
three product areas. The [completion map](0002-execution-sessions-policy-completion.md)
and [task graph](0001-v2-milestones.md) retain the remaining acceptance gates.

Daemon source `405c8b2` (state21) passes717 workspace tests with all features enabled,
7 intentionally ignored, strict all-target/all-feature lint and formatting.
The build took92 seconds and test-process times sum to88.12 seconds on this shared
machine; this is an observed run, not a promised CI bound. Exact source hashes and
logs are in the [integration manifest](../implementation/three-area-integration-qualification.json).

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

Continuing work:

- Native preparation/offer/grant assembly and actual worker consumer. Pre-offer Git
  export must precede attempt/spec-bound transport wrapping. Final rendered startup
  bytes must be pinned and counted exactly, without duplicate context delivery.
- Result recording, independent eligibility evaluation and actual source/effect
  settlement before claim release or expected-version graph completion.
- Bounded terminal tail qualification, followed by renderer/full baseline and
  snapshot/live barriers. Negative candidate14354e7 confirmed that a sole SIGSTOPped
  tmux control observer can block the harness write despite pause-after. Input was
  Applied and the guardian remained Running. A guardian-owned reader lease with
  bounded drain heartbeats is now the correction design; no tail acceptance claim
  is made until independent cutoff/progress and regression tests pass.
- Keyed playbook admission and actual AQ adapters. Upstream adds atomic run/receipt
  creation and an old-writer fence; central durable bridge adoption is separate.
  PB04's25ms latency target remains unmet and is not relaxed.

No live-provider qualification, deployment, external human notification or push
was performed. GATE-04/GATE-05 and broader v2 acceptance remain open.
