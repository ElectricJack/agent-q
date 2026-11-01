# Native execution integration checkpoint

The daemon, worker and dashboard have independently tested integration checkpoints.
They do not yet complete a native task through a real harness and policy outcome.
This is the current integration status; the earlier
[three-area checkpoint](2026-10-31-execution-sessions-policy-progress.md) retains
its historical test results and source pins.

| Area | Integrated behavior | Remaining product work |
| --- | --- | --- |
| Daemon schema and preparation | Ordered state25→27 migrations, immutable definitions, bounded graph-to-offer publication, Accept→Prepare→initial Grant, read-only reconciliation, automatic ungranted expiry | Renewal, multi-project fairness, granted-before-Started closure and full settlement |
| Worker durability | Private native journal, resource holds, exact Accept replay, durable input request identities, independent material installation pins and actual Git hydration/workspace ownership and durable Prepare input join | Grant/renewal and launch/stop callbacks, result/release |
| tmux host | Durable blank terminal baseline before sealed-image exec; original physical server/pane ownership checks | Native driver adoption, selected real harness qualification and operational load envelope |
| Playbooks | Catalog2 human decision adapter, lost-Accepted-reply recovery and explicit optional startup/drive/shutdown | Broader real adapters, workflow pilots and verified network database transport |
| Dashboard | Live daemon overview/projects/sessions/workers with capability gates and verified static assets | Live terminals, interaction and remaining task/workflow/domain views |

The daemon checkpoint is `eacdc05`, with frozen implementation source `50840a0`.
[Schema evidence](../implementation/AQ-schema26-native27.qualification.md)
records 206 passing selected tests, two existing ignores, strict lint/formatting,
four updater rollback checks, and an actual PostgreSQL archive/restore that
reproduced the original format25 snapshot. Schema installation alone enables no
optional runtime. The earlier native preparation evidence remains separately
attributed to its original source.

The previous combined worker checkpoint is `281f8a4`, with source `0ff68c2`.
[Worker evidence](../../../aq-worker/docs/implementation/WRK-08-worker-integration-qualification.md)
records 371 broad workspace tests with 43 explicit ignores at the preceding
combined source, then 122 worker tests after the original-runtime-pin followup.
Explicit actual-binary runs passed three Git cases, 36 tmux cases and three
terminal bridge cases. These counts overlap component coverage and must not be
summed as unique product tests. One earlier tmux case expired under concurrent
storage-test/build load; that negative remains retained, and identical deadlines
passed under lower controlled load. Startup under saturation is not qualified.

The [initial live dashboard checkpoint](../../../aq-dashboard/docs/evidence/d05-live-framework.md)
has 234 unit/component, 35 mocked browser/accessibility and four actual-daemon
browser checks. The [new assembled checkpoint](../implementation/schema27-activation-dashboard-qualification.md)
at daemon source `077a0ad` passed 45 selected server checks and all four live
browser checks after upgrading the isolated fixture database from25 to27. The
existing fixture session survived. No agents were launched. Optional playbook
startup has its own explicit-configuration qualification and remains off in the
preview; its first engine connection profile requires a trusted Unix socket.

The newer [assembled grant checkpoint](../implementation/AQ-08-grant-expiry-dashboard-integration.md)
at `a7f7665` passed45 selected daemon checks and four browser checks on its rebuilt
preview. [Native publication](../implementation/AQ-08-native-offers.qualification.md)
and [initial grants](../implementation/AQ-08-native-initial-grant.qualification.md)
retain their independent32- and153-test component qualifications. The publisher
currently selects one slot, the highest-priority enabled project and its first ready task;
this is not fleet fairness. Its original preparation window is at most30seconds
and can be shorter. Large cold hydration remains an explicit limitation.
[Automatic expiry](../implementation/AQ-08-offer-expiry-integration.md) closes only
proven never-granted attempts and preserves retained source/effect ownership.

Worker source `ec41ad7` now adds format2 journal input intents and independent
material installation pins. Inline verified payloads remain in memory and must
be downloaded and verified again after loss; receipt metadata is not availability
or live authority. It does not yet expose Prepare or claim actual Git hydration.
Its [frozen qualification](../../../aq-worker/docs/implementation/WRK-08-native-inputs-qualification.md)
at `238c2ce` records129 package tests passed, three existing actual-Git ignores,
22 focused tests as a subset, and strict lint/formatting. Earlier broad worker
test counts above are not attributed to this newer source.

[Read-only reconciliation](../implementation/AQ-08-reconciliation-qualification.md)
is integrated at `24877ac`: 51 selected checks passed with zero ignores, including
the actual Git grant cases, plus strict lint and formatting. It scopes history to
the authenticated node and original boot, validates actual terminal receipts,
and never issues delivery clocks or adopts a worker journal.

Worker hydration source `f14694e`, integrated with evidence at `a95918e`, passes
142 package tests (four declared ignores) and its separately enabled pinned
actual-Git case. It downloads and verifies source, imports it, owns a worktree,
and checks current retention/head/cleanliness. The default Git profile requires a
conservative logical reservation of21GiB+1MiB per attempt; a32GiB test reservation
is not physical free-space evidence. Driver Poll/reservation capacity must agree
with this bound before activation. Source geometry is bounded to512 ranges;
large cold preparation and physical disk pressure remain unqualified.
[Exact worker evidence](../../../aq-worker/docs/implementation/WRK-08-native-hydration-qualification.md)
retains these limitations. Journal format3 refuses older layouts unchanged.

The pure additive granted-closure contract0.1.8 is packaged and integrated;
58 source and58 extracted-package tests pass, with old0.1.7 compatibility checked.
This does not enable a server closure owner or worker proof producer. Parallel
implementation now covers renewal and ordered schema28 plus the server closure
owner. [Initial lineage budgets and retained installation receipts](../implementation/AQ-08-initial-budget-material-receipts-evidence.md)
are integrated at `876b60b`:57 native tests with zero ignores plus two policy
helpers passed. The grant's immutable deadline respects the original remaining
lineage budget. Exact verified installation receipts are atomically retained and
included in preparation storage quotas; missing history refuses replay/renewal.

The [worker Prepare join](../../../aq-worker/docs/implementation/WRK-08-native-prepare-qualification.md)
is integrated at `42d0ea8` (source `82d3c07`):146 package tests, four declared
ignores, a separately enabled actual-Git case, and strict lint/formatting passed.
It joins actual verified inputs, installed materials and freshly checked source
ownership into an immutable request before any send. Journal format4 refuses
older layouts unchanged. These counts overlap the previous hydration suite.
A separate worker-process/real-daemon assembly test is the next interface check;
no complete native host execution is yet claimed. The [actual Git producer-to-grant checkpoint](../implementation/AQ-08-actual-git-grant-qualification.md)
passes14 initial-grant tests including two actual-Git cases. The [fresh pre-grant Graph fence](../implementation/AQ-08-initial-grant-current-graph.md)
now closes the observed task/context-change gap and passes43 selected checks,
including both actual-Git cases. It rereads bounded readiness and exact authorized
context and checks freshness immediately before commit without retiming original
inputs. The preview remains the separately pinned `960ffc7` binary.
Shared migration27 and public0.1.7 are frozen. No path may fabricate missing
original requests, convert timeout to release proof, or restore old-boot authority.

Granted-without-Started recovery still needs its server and worker owners: it cannot
reuse NeverGranted or manufacture Started to release a claim. AQ-08, WRK-08,
WRK-14, AQ-12/13/14 and all related product gates stay in progress. No model call,
external notification, deployment or publication is claimed by this checkpoint.
