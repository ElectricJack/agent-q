# Native execution integration checkpoint

The daemon, worker and dashboard have independently tested integration checkpoints.
They do not yet complete a native task through a real harness and policy outcome.
This is the current integration status; the earlier
[three-area checkpoint](2026-10-31-execution-sessions-policy-progress.md) retains
its historical test results and source pins.

| Area | Integrated behavior | Remaining product work |
| --- | --- | --- |
| Daemon schema and preparation | Ordered state25→27 migrations, immutable public definitions, native Accept→retained preparation/material plan | Native graph-to-offer assembly, Prepare→Grant, renewal and reconciliation |
| Worker durability | Separate private native journal, resource holds, exact Accept replay and observed preparation; reopening preserves historical ownership | Durable downloads/material/Prepare, actual Git hydration, grant/renewal and launch/stop callbacks, result/release |
| tmux host | Durable blank terminal baseline before sealed-image exec; original physical server/pane ownership checks | Native driver adoption, selected real harness qualification and operational load envelope |
| Playbooks | Catalog2 human decision adapter and lost-Accepted-reply recovery | Explicit optional startup/drive/shutdown integration, broader real adapters and workflow pilots |
| Dashboard | Live daemon overview/projects/sessions/workers with capability gates and verified static assets | Live terminals, interaction and remaining task/workflow/domain views |

The daemon checkpoint is `eacdc05`, with frozen implementation source `50840a0`.
[Schema evidence](../implementation/AQ-schema26-native27.qualification.md)
records 206 passing selected tests, two existing ignores, strict lint/formatting,
four updater rollback checks, and an actual PostgreSQL archive/restore that
reproduced the original format25 snapshot. Schema installation alone enables no
optional runtime. The earlier native preparation evidence remains separately
attributed to its original source.

The worker checkpoint is `281f8a4`, with final source `0ff68c2`.
[Worker evidence](../../../aq-worker/docs/implementation/WRK-08-worker-integration-qualification.md)
records 371 broad workspace tests with 43 explicit ignores at the preceding
combined source, then 122 worker tests after the original-runtime-pin followup.
Explicit actual-binary runs passed three Git cases, 36 tmux cases and three
terminal bridge cases. These counts overlap component coverage and must not be
summed as unique product tests. One earlier tmux case expired under concurrent
storage-test/build load; that negative remains retained, and identical deadlines
passed under lower controlled load. Startup under saturation is not qualified.

The [live dashboard checkpoint](../../../aq-dashboard/docs/evidence/d05-live-framework.md)
has 234 unit/component, 35 mocked browser/accessibility and four actual-daemon
browser checks. Those four used daemon source `8bc7a63`, before this schema
integration, and synthetic sessions with no launched agents. A fresh live-browser
run against the assembled final daemon remains part of the next integration
checkpoint; the older smoke result is not silently attributed to new source.

Parallel work is divided at ownership boundaries: graph/profile/source offer
assembly; central initial grants; optional playbook startup/lifecycle; and root
worker/integration qualification. Shared migration27 is frozen, and exact
original-offer loading is a small shared prerequisite. Each source change must
be reviewed and tested before integration. No path may fabricate missing original
requests, convert timeout to release proof, or restore old-boot live authority.

Granted-without-Started recovery is still a separate missing protocol: it cannot
reuse NeverGranted or manufacture Started to release a claim. AQ-08, WRK-08,
WRK-14, AQ-12/13/14 and all related product gates stay in progress. No model call,
external notification, deployment or publication is claimed by this checkpoint.
