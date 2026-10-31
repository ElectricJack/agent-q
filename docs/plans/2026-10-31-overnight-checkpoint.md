# Overnight implementation checkpoint — 2026-10-31

The distributed foundations have advanced, but the real-agent milestone is still
open. The next product milestone is one native agent receiving a real graph task,
preparing a retained workspace, running under the guarded tmux host and settling
its result with durable evidence. Component tests do not substitute for that
complete path. The [canonical task graph](0001-v2-milestones.md) retains the
unfinished work as in progress or planned.

## Integrated daemon checkpoint

State format 20 adds a bounded owner for retained catalog, context, composition,
selection and outcome bytes, canonical retention receipts, and assigned-worker
range transfer. The independent review found no blocker in that scope. The
[artifact qualification](../implementation/AQ-08-inline-artifacts-qualification.json)
records 678 passing workspace tests, six declared optional ignores, strict lint
and formatting at source `ce1214f`. Main's build and source files were compared
with that tested revision after merging; they match exactly.
A subsequent test-only checkpoint adds four Preparing/Granted HTTP tests. All
eight artifact-owner/phase tests passed, with strict target lint and formatting;
no production changes were required. This supplements the full workspace run
rather than claiming a new full-suite total. See the
[phase qualification](../implementation/AQ-08-artifact-phases-qualification.md).

The daemon also contains current native runtime registration, durable Started
reporting, private provider-material delivery and the acyclic profile/selection
correction. The correction separates the policy known before model selection
from the resolved graph profile that includes the selected model. It verifies
permissions against actual typed context records and project/repository bindings.
These endpoints and contracts do not yet constitute a complete native launch
producer. Actual offer/accept/prepare/grant production, qualified context/token
planning facts and the worker's complete native reporting path remain open.

Relevant evidence:

- [Acyclic profile and selection packages](../implementation/AQ-08-acyclic-graph-policy-qualification.md).
- [Native Started ownership](../implementation/AQ-08-started-owner.md).
- [Private material transfer](../implementation/AQ-08-material-delivery.md).
- [Retained artifacts and assigned-worker transfer](../implementation/AQ-08-inline-artifacts.md).
- [Terminal relay integration](../implementation/AQ-21-relay-integration-qualification.json).
- [Observation ownership against the state19 daemon](../implementation/AQ-22-owner-state19-qualification.json).

## Worker checkpoint and capacity correction

The combined worker integration at `f36534f` passed 288 workspace tests with
29 declared optional tests ignored. Strict workspace lint and formatting passed;
both explicit local tmux/HTTP relay tests passed against the integrated
dependencies. Evidence checkpoint `fcae85c` is now merged into the local worker
branch. The earlier 23-test host tmux run remains separate evidence, not a claim
that those tests were rerun in the final integration. See the
[worker integration report](../../../aq-worker/docs/implementation/WRK-14-integration.md).

Terminal stage 1 provides guarded input, resize and controller fencing for an
already verified tmux session. Durable uncertain outcomes never trigger automatic
input reinjection. Reconnect observes the existing session; it does not launch a
replacement. Full terminal output acquisition, renderer/snapshot ordering and
live viewing remain open. The production worker does not yet adopt the new
bridge or advertise full terminal capability.

Observation work adds bounded indexed normalization, durable transport ownership
and an authenticated outbound HTTP peer. The production worker does not yet
adopt the experimental store. The four selected harness versions still require
qualification; synthetic normalizer fixtures are not native harness evidence.

A default-limit headroom probe found a real defect in experimental store 0.0.4:
ordinary retained history could consume space needed for a gap or a new-boot
fence. The original failing store and replay evidence are retained. The format5 correction is now qualified and integrated at worker checkpoint
`d165837`. It reserves metadata, delivery and control capacity inside the existing
16 MiB logical / 64 MiB physical configuration. The exact extracted0.0.5 package
passed 117 regular tests and two explicit headroom tests, strict lint, formatting
and source-drift checks. At the unchanged default limits it filled ordinary
history, recorded one loss gap and 15 live-source boot gaps, preserved retained
history and completed 64 boot transitions without duplicate source events.
The old-format store refused with its data and intent bytes unchanged.

The final combined worker source `946e13f` passed 294 workspace tests with 29
declared optional ignores, strict lint and formatting. Main build/source files
match that tested revision. See the
[component correction](../../../aq-worker/docs/implementation/WRK-15-reserved-control.md)
and [combined qualification](../../../aq-worker/docs/implementation/WRK-15-reserve-integration.md).
Physical-cap/ENOSPC progress and in-place format migration remain unqualified;
production runtime adoption remains off.

## Test turnaround

The latest logs give these approximate suite costs on this host with cached
dependencies. They are Cargo's reported build time plus the sum of individual
test-process durations, not independently measured total wall time.

| Suite/source | Passing / ignored | Test execution | Compilation | Approximate combined |
| --- | --- | --- | --- | --- |
| Daemon `ce1214f` | 678 / 6 | 96.14 s | 82 s | 3 min |
| Final worker integration `946e13f` | 294 / 29 | 142.94 s | 20.11 s | 2 min 45 s |

Daemon strict lint took 22.43 seconds; the final worker lint took 7.20 seconds.
The focused eight-test artifact check took about three seconds of test execution.
Cold builds, extracted-package tests,
explicit tmux qualification, extended fault campaigns and capacity probes are
additional work. Normal workspace suites intentionally leave opt-in external or
host-specific tests ignored; selected local tests have separately retained runs.

## Next implementation sequence

1. Complete the native launch producer/consumer path with actual authorized graph
   context, retained inputs, real preparation receipts and current grant checks.
2. Qualify the first native harness and demonstrate the real-agent milestone,
   including cancellation, restart and evidence-based completion.
3. Extend terminal output/viewing and native observation delivery, then build
   stall handling, verified waits, suspension and continuation on those proven
   lifecycle boundaries.

Playbook persistence experiments reduced query work but did not meet the existing
25 ms p95 transition budget. That work remains open; no unsuccessful optimization
was adopted as a production performance improvement. Dashboard integration and
multi-harness product qualification also remain planned.

The overnight batch is complete: current changes and qualification evidence are
committed locally, and the task graph is updated. This checkpoint is not a
deployed or published release; the broader v2 milestones above remain open.
