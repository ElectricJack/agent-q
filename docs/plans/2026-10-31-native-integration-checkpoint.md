# Native execution integration checkpoint

The daemon, worker and dashboard have independently tested integration checkpoints.
They do not yet complete a native task through a real harness and policy outcome.
This is the current integration status; the earlier
[three-area checkpoint](2026-10-31-execution-sessions-policy-progress.md) retains
its historical test results and source pins.

| Area | Integrated behavior | Remaining product work |
| --- | --- | --- |
| Daemon schema and preparation | Ordered state25→32 migrations, immutable definitions, bounded graph-to-offer publication, Accept→Prepare→initial Grant, renewal, read-only reconciliation, automatic ungranted expiry | Phase host/report lifecycle integration and combined physical presence reporting, multi-project fairness, worker granted-before-Started proof production and full settlement |
| Worker durability | Private native journal, resource holds, exact Accept replay, durable input request identities, material installation pins, actual Git workspace ownership, durable Prepare join and received initial Grant history | Continuing renewal owner, launch/stop callbacks, result/release, phase input/execution process qualification |
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
this is not fleet fairness. Its outer publication window is at most30seconds, but the original selection
and budget observations cap actual preparation at2seconds from observation.
Measured Accept deliveries leave about1.3seconds. Large cold hydration remains
an explicit limitation; the outer bound is not a usable30second preparation lease.
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
The [contract evidence](../implementation/AQ-08-granted-closure.qualification.md)
does not enable a server closure owner or worker proof producer.
[Schema28 and the strict initial-grant terminal fence](../implementation/AQ-08-closure28-prerequisite-integration.md)
are integrated at `e930d75`:57 native and eight schema checks passed, with strict
affected lint and formatting. The newer server owner qualification and integration are recorded below. [Initial lineage budgets and retained installation receipts](../implementation/AQ-08-initial-budget-material-receipts-evidence.md)
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
The [independent worker-process/real-daemon probe](../implementation/AQ-08-worker-prepare-assembly.md)
now passes its tiny fixture at source `b411421`, crossing actual HTTP, SQL, private
material installation and separate central/worker Git processes through usable
initial Grant. The run takes3.10seconds including fixture setup. An earlier run
of the same pinned batch worker reached a committed server Grant, then rejected
the delivery after its conservative preparation deadline. That is an ever-issued
held grant, not NeverGranted. Both results remain retained; the passing profile
does not qualify cold preparation or load. No complete native host execution is
yet claimed. The [actual Git producer-to-grant checkpoint](../implementation/AQ-08-actual-git-grant-qualification.md)
passes14 initial-grant tests including two actual-Git cases. The [fresh pre-grant Graph fence](../implementation/AQ-08-initial-grant-current-graph.md)
now closes the observed task/context-change gap and passes43 selected checks,
including both actual-Git cases. It rereads bounded readiness and exact authorized
context and checks freshness immediately before commit without retiming original
inputs. The earlier preview was pinned to `960ffc7`; the newer preview update is recorded below.
Shared migration27 and public0.1.7 are frozen. No path may fabricate missing
original requests, convert timeout to release proof, or restore old-boot authority.

Granted-without-Started recovery still needs its qualified worker producer and
activation; the explicitly injected server owner is now integrated. It cannot
reuse NeverGranted or manufacture Started to release a claim. AQ-08, WRK-08,
WRK-14, AQ-12/13/14 and all related product gates stay in progress. No model call,
external notification, deployment or publication is claimed by this checkpoint.


[Atomic inline batching](../../../aq-worker/docs/implementation/WRK-08-native-artifact-batch-qualification.md)
is integrated at worker `6c339bc` (source `de8fa62`):149 package tests passed with
four declared ignores, plus a separately enabled actual-Git test and strict lint.
The36 original inputs now need two journal publications instead of72. All request
IDs remain stable, failed downloads create no partial completion receipts, and
eight fsync fault cuts preserve either the old or complete new journal state.
The independent smoke process uses this API at worker `1ed243c`. These component
counts overlap earlier worker suites and do not establish cold preparation.

[Native renewal](../implementation/AQ-08-native-renewal.qualification.md)
is integrated at daemon `660080c` (source `c5438d3`):27 SQL/HTTP/schema checks and
26 focused library checks pass, with two declared existing actual-Git ignores
and strict lint/formatting. It refreshes current owner facts for the same live
grant and immutable hard deadline, supports exact replay and rejects superseded
operations. Original retrieval and proposal expiry does not rewrite historical
installation or selection. Missing/changed installed receipts and fresh matching
negative facts refuse. This is not fleet-load qualification.

Two interface gaps are now explicit implementation requirements. First, the
[preparation phase design](../../../aq-worker/docs/implementation/WRK-08-preparation-phase-separation.md)
separates timely proposal acceptance, bounded no-launch preparation and a fresh
execution permit under a newly negotiated contract. Its contract and bounded owner now have the separate qualifications recorded
below; no existing0.1.7/0.1.8 clock was widened. Worker input adoption and fresh
execution confirmation remain open. Second, the inventory API cannot yet report
scoped continuing installed-material presence with exact lease/receipt evidence.
The passing initial renewal profile uses a genuinely fresh original inventory;
longer continuation needs that owned report seam, rather than invented global
credential availability or relaxed newer-negative checks. Both remain open.

The [schema28/renewal union](../implementation/AQ-08-schema28-renewal-integration.md)
at `8f1adcc` passes75 selected native/schema tests and strict server lint. Its
separate independent-worker probe refuses at hydration under a1,143ms accepted
window, so the older schema27 tiny-fixture success is not attributed to this
newer source. The actual local preview is now rebuilt at `8f1adcc` and upgraded
to schema28 with the existing fixture preserved; all four live browser checks
pass. Native optional owners remain off and no agents were launched.


[The granted-closure owner](../implementation/AQ-08-closure-owner-evidence.md)
is integrated at `8ab6d7a` (source `7eb9696`), with nine actual publication/SQL/HTTP
owner tests and three schema checks. Explicit immutable historical verifier
policy injection is required; default routes stay unmounted. The
[combined closure/renewal check](../implementation/AQ-08-closure-renewal-integration.md)
passes44 selected tests at `fbe90e9`, including a new counter-versus-terminal
ordering test, under the qualified two-thread profile. A higher-concurrency
initial-admission fixture expiry remains retained. No original clock changed.
Worker physical proof and separate settlement are still required.

The next parallel implementation lanes are the
[accepted preparation protocol](../implementation/AQ-08-accepted-preparation-v2-plan.md),
[scoped material presence](../implementation/AQ-08-scoped-material-presence-plan.md)
and worker phase input adoption plus continuing grant ownership before host hooks. Presence facts
are independent per attempt: refreshing one agent must not invalidate every
other agent's evidence or advertise its private material globally. These remain
in progress and do not activate runtime capabilities by documentation alone.


[Bounded two-read overlap](../../../aq-worker/docs/implementation/WRK-08-artifact-overlap-qualification.md)
is integrated in worker `f1e28d9` (source `a39e990`):135 default and152 all-feature
package tests pass, with four declared ignores, plus the separately enabled
actual-Git test and strict lint. A deterministic barrier proves two active reads
and independent control progress with unchanged atomic receipt completion.
The newer [actual process check](../implementation/AQ-08-overlap-worker-integration.md)
passes at daemon `6d4b178`/worker `a39e990` in3.17seconds. No original deadline
changed; earlier refusals are retained and cold preparation is still open.

The joint [0.1.9 contract package](../implementation/AQ-08-admission019.qualification.md)
is qualified at `1088d10`, with83 source and83 independently extracted tests,
strict lint and external schema checks. It adds explicit preparation phases and
scoped material-presence types;72 inherited0.1.8 files remain byte-for-byte intact.
It is a local package, not a registry publication or runtime activation.

The worker's [durable initial-grant owner](../../../aq-worker/docs/implementation/WRK-08-durable-initial-grant-qualification.md)
is integrated at `d919e4c` (source `99e793c`), with164 package tests, five explicit
ignores, two separately enabled actual-Git checks, and strict lint/formatting.
Format5 retains the actual received Grant before checking present usability;
expired or changed inputs retain history and holds without live permission.
Cancellation retains exclusive input owners in a bounded per-attempt map.
This does not supply a continuing renewal owner or host launch callbacks.

The [updated independent process probe](../implementation/AQ-08-durable-worker-integration.md)
now requires durable received-grant evidence. Its first run at daemon `82d386f`
/ worker `30e182f` refused during artifact download with720ms left at Accept;
no central Grant was issued and the new durable assertions were not reached.
This negative is retained alongside the earlier format4 success. The
[accepted-preparation owner plan](../implementation/AQ-08-accepted-preparation-owner-plan.md)
addresses this measured limitation. The schema029 prerequisite and ordered030
installation are now integrated; their qualifications are recorded below. The
newer dashboard preview is pinned to `94893a3` with state30.


The [accepted-preparation owner candidate](../implementation/AQ-08-accepted-preparation-owner.qualification.md)
is integrated at `100d301` (production `9121809`, test follow-up `940b061`).
Thirteen PostgreSQL/HTTP cases pass, including fresh Store/AppState/config reopen,
independent no-launch leases and original input retrieval after proposal expiry.
Seventy-two related checks and two actual-Git legacy regressions pass separately.
The newer [actual v2 retained-source check](../implementation/AQ-08-accepted-preparation-actual-git.qualification.md)
passes at `0c74bf4`, integrated at `d96166c`: real Git source reads and exact replay
succeed after actual proposal expiry, and retired retention refuses reads while
keeping the task held. Original source eligibility had not itself expired in that
case. No phase execution Grant or full cold-worker hydration is claimed.

[Schema29 and qualified inventory ownership](../implementation/AQ-08-presence-029.qualification.md)
are integrated at `a523888` (source `85160a7`):11 actual PostgreSQL checks plus strict
lint/formatting pass. Independent inventory snapshots and semantic compatibility
epochs prevent a legacy/incompatible publication from reviving old installed facts.
This prerequisite does not yet mount presence-report routes or change renewal to
consume them. Ordered030 installation is now separately qualified below.
A review-found proposal node/boot-scope gap is corrected at `a271649`; the
[independent two-node regression](../implementation/AQ-08-proposal-scope-main29.json)
passes on29. Optional phase runtime configuration remains absent from the preview.

Worker [phase HTTP transport](../../../aq-worker/docs/implementation/WRK-08-preparation-v2-http.md)
is integrated at `8fa989a` with the exact0.1.9 archive;170 package tests and two
actual-Git regressions pass. The newer
[format6 phase journal](../../../aq-worker/docs/implementation/WRK-08-phase-journal-qualification.md)
is integrated at `7823fa3` (source `3326b59`/`08eb15c`), with179 package tests,
five declared ignores, two separately enabled actual-Git regressions, strict lint
and formatting. It atomically selects the protocol and stores the exact Accept
intent before sending; received history survives expiry/replay and cannot restore
live authority after restart. Redelivered offer clocks are associated with the
unchanged original publication without extending deadlines. These counts overlap.
A lost reply with no opaque observation remains unknown/held; phase input adoption,
execution issuance and recovery are still required. The new real-process phase
probe now passes on ordered030, as recorded below.


[Ordered030 qualification](../implementation/AQ-08-ordered30.qualification.md)
is integrated at `94893a3` (source `66e83be`, integrated source `4fa0dce`). It
records19 owner/scope/schema cases,84 related checks with four declared ignores,
one metadata check and one separately enabled actual-Git case. These total105
distinct passing cases at that frozen source; they are not added to older suites.
Rollback and missing-table refusal tests use actual ordered Store installation.
One functional route fixture uses a fixed post-publication clock; real-expiry and
actual-Git/process qualification retain actual clocks.

The [real worker phase probe](../implementation/AQ-08-phase-worker-integration.md)
passes in4.27seconds at daemon `4fa0dce` / worker `cf943ff`. The independent worker
process persists its exact Accept, waits beyond the original proposal expiry and
replays under the separately issued no-launch preparation lease. Durable history
and journal revision remain unchanged. This does not yet test phase worker input
hydration, execution Grant or host launch. Earlier legacy preparation refusals
remain separately recorded.

The worker [scoped presence producer](../../../aq-worker/docs/implementation/WRK-08-scoped-presence-evidence.md)
is integrated at `54c6468` (source `04e2a32`):196 package tests with five declared
ignores,17 final focused checks and two actual-Git checks pass, with strict lint
and formatting. Positive reports require an actual current opaque grant and a
bounded descriptor-based read of the installed material. Unsafe/changed files,
cancellation and lost acknowledgements retain their distinct outcomes. These
counts overlap; server report/renewal consumption remains separate active work.

The [schema30 live preview](../implementation/AQ-08-dashboard-schema30.md) now runs
daemon `94893a3`, after a private backup and actual28→30 fixture database upgrade.
All four actual browser checks pass in2.2seconds, with the existing session
preserved. Optional native execution and playbooks stay inactive. Current
parallel work is phase input adoption, scoped presence consumption and the fresh
phase execution-grant owner. Product gates remain in progress.


The [format7 phase input owner](../../../aq-worker/docs/implementation/WRK-08-phase-input-evidence.md)
is integrated at worker `25619c5` (source `a406e48`):205 package tests with six
declared ignores and three actual-Git checks pass. The126-inline plus2-source
profile passes after removing duplicate full-proposal validation on every input;
the earlier31.86second expiry remains retained. No production deadline changed.
Explicit protocol provenance, cancellation/fault cuts, immutable Prepare replay
and history-only reopen are covered.

The [independent phase input process](../implementation/AQ-08-phase-input-worker-integration.md)
passes at daemon `c78093f` / worker `cae23c7` in7.23seconds. Actual HTTP, PostgreSQL,
private installation, journal and separate Git processes produce38 artifact/source
receipts and one material receipt, with durable Prepare3.705seconds after the old
local cap. This is an actual interoperability check through preparation, without
an execution Grant or host launch.

The worker [execution HTTP observation client](../../../aq-worker/docs/implementation/WRK-08-phase-execution-http.md)
is integrated at `b094bc9` (source `21404a2`):11 phase loopback tests and strict
lint pass. Its opaque replies preserve immutable first observations and separate
execution clocks; they are not durable execution or host permission. The next
parallel lanes are durable phase execution ownership, server fresh execution
facts/issuance, and scoped presence/renewal consumption. The latter adds ordered031
for a normalized provisionable-binding removal/restoration barrier; execution
provenance reserves032. These migrations are not claimed integrated yet.


The [server scoped-presence owner](../implementation/AQ-08-presence-owner-evidence/qualification.md)
is now integrated at `3b364ab` (source `7a19e0a`, integrated source `c3f2df1`).
Fifty-four actual SQL/router/grant-renewal/schema checks pass, with five explicit
artifact-dependent ignores, plus strict all-target/all-feature lint and formatting.
Ordered031 adds a normalized provisionable-binding-set barrier; removing and
restoring a binding requires a new read. Ordinary same-set inventory refreshes
and per-attempt reports remain independent. The evidence uses synthetic qualified
worker read metadata; it does not claim the actual remote physical producer.

The [schema31 union process check](../implementation/AQ-08-phase-presence31-union.json)
also passes in7.23seconds with the actual format7 worker preparing inputs after
proposal expiry. It validates coexistence on31, not a physical scoped-report or
execution-Grant flow. The local dashboard preview stays on its independently
qualified schema30 binary. Parallel followups are simultaneous-head/control
isolation, fresh execution issuance (032) and durable worker phase execution
(format8); those candidates remain unqualified and unactivated.


The [latest local dashboard check](../implementation/AQ-08-dashboard-schema31.json)
runs daemon `37afe9d` on ordered31, after a private30→31 fixture backup/upgrade.
All four actual browser checks pass in2.1seconds, including preserved session
detail and direct refresh. Native execution, playbook startup and unsupported
terminal views remain inactive. Earlier preview pins above are historical.


The [format8 durable phase execution owner](../../../aq-worker/docs/implementation/WRK-08-phase-execution-owner-evidence.md)
is integrated at worker `f4d6a19` (source `82496ef`, integrated source `76eee8d`).
Its qualification records219 package passes, six declared ignores, three actual-Git
passes and strict lint. An actual retained opaque execution observation can survive
lost acknowledgements without resetting its original caps. Disk history alone does
not restore live permission. This is component evidence, not a real daemon grant.

The [latest independent input process check](../implementation/AQ-08-phase8-origin31-union.json)
passes in7.47seconds at daemon `87d54d5` with that format8 worker:38 artifact/source
receipts, one material receipt and durable Prepare after the original proposal
expires. It does not send a phase execution request or launch a host.

The [origin fence](../implementation/AQ-08-phase-origin.qualification.md) is
integrated at `1b6214f`, with root regression tests at `aeb9ed0`. Actual accepted
phase history cannot enter the legacy Started or renewal paths. One new regression
failed before the guard and both pass afterward. Earlier aggregate admission
expiry failures remain recorded; isolated passing reruns do not relabel them.
The [presence isolation checks](../implementation/AQ-08-presence-isolation-evidence/qualification.md)
are integrated at `2e621c5`: four deterministic isolation cases and three schema
checks pass. Per-attempt evidence stays independent, and held presence bodies or
pool connections do not consume the reserved renewal lane. This is not fleet-load
qualification or multi-slot placement support.

Current parallel work is the daemon fresh phase issuer and ordered032 transaction
qualification, its current-owner refusal tests, and the worker's explicitly
synthetic [observed tmux launch owner](../../../aq-worker/docs/implementation/WRK-08-native-phase-host-plan.md).
The schema31 dashboard preview remains the latest qualified preview. Runtime,
terminal and complete task-lifecycle gates stay in progress.


The [fresh phase execution owner](../implementation/AQ-08-phase-execution.qualification.md)
is integrated through `5643966`, with evidence at `e176890` (source `3af7741`).
Ordered32 retains exact phase receipt/provenance and shared quota charges. Actual
current owners confirm the original choice; installation receipt, journal and hot
grant commit atomically. A final post-insert fence checks observation/fact/lease
expiry and backwards time before commit. Its controlled-clock lock-wait test
proves rollback after the final awaited insert; it is not a latency measurement.

The initial12-case owner suite passes. The final14-case suite has13 passes and one
original two-second Accept setup expiry; that unchanged case passes individually.
Necessary accepted/origin/Started/closure/schema regressions pass39 cases. The
broader legacy grant suite records39 passes, two timing-boundary failures and seven
explicit ignores; both failures pass individually with unchanged clocks. Strict
all-target/all-feature server lint passes. These separate results are retained,
not combined into an all-green aggregate. Root verified28 owned source hashes
against both the frozen source and integrated branch and all16 evidence logs.

No production phase execution route, existing-writer deployment fence, paired
phase Started/Stopped/closure consumer or continuing phase renewal is enabled.
The worker's format8 execution owner and format9 synthetic-host candidate remain
separately qualified components; the independent actual worker/issuer process
probe and updated dashboard preview have their own pending assembly checks.


The [schema32 dashboard preview](../implementation/AQ-08-dashboard-schema32.json)
now runs the final-fence daemon implementation `5643966`, built at `ea1ca6e`.
The actual fixture upgraded31→32 after a private backup and stopping the old
server; the existing session remains visible. All four live browser checks pass
in1.9seconds. Current read-only views are connected; native execution, optional
playbook startup and terminal capabilities remain inactive in this preview.


The [independent phase execution process](../implementation/AQ-08-phase-execution-process-evidence/qualification.md)
is integrated at `e0d0eda`: actual daemon/HTTP/PostgreSQL plus pinned independent
worker and Git processes reach durable execution in6.23seconds, with issuance
2.079seconds after the original proposal expires. Real child registration occurs
before qualified InventoryV2 publication. The execution probe uses prompt input
preparation; separate expiry probes retain their deliberate pause. The earlier
registration, stale-availability and test-audit failures are preserved. No source
age, proposal, preparation or execution policy was widened.

The [format9 native host component](../../../aq-worker/docs/implementation/WRK-08-native-phase-host-evidence.md)
is integrated at worker `3954b07` (source `40d6e36`, integrated `8c813d6`). It fsyncs
the outer launch intent before host effects, verifies a blank tmux baseline before
sealed-image exec, and retains actual local Started/ECHILD history. Independent
stop/expiry survive a blocked writer. Unadmitted claim cleanup cannot erase a stop
fence; admitted work is never blindly relaunched. Twelve top-level actual tmux
cases pass (including one nested short-exec child), plus221 normal package tests
with18 explicit ignores and three actual-Git checks. Strict lint/default build and
formatting pass. This is a closed synthetic4KiB harness profile, not provider or
remote Started qualification. Root verified886 frozen source hashes,26 artifacts
and all14 changed source files against integration.

The [current format9 process union](../implementation/AQ-08-phase9-process-evidence/qualification.md)
is integrated through `71eb244` / test source `2297616`: the same real worker input
and fresh issuer chain passes in5.78seconds with journal9/revision27. Its separate
input-only/late-replay case passes in7.15seconds with revision26. These runs reach
durable permission, not physical host launch. The physical host evidence above
remains a separate qualification. Product gates stay in progress.

Next parallel implementation is the [paired phase terminal consumer plan](../implementation/AQ-08-phase-terminal-consumers-plan.md):
a bounded standalone origin protocol, typed daemon Started/Stopped/no-Started
closure owners, and additive033 compatibility fencing tested with already-open
old-style SQL writers. The schema32 preview stays current while these candidates
are isolated. No route, capability, renewal, task settlement or provider execution
is activated by that design.

The [standalone phase terminal contract](../implementation/AQ-08-phase-terminal-contract.md)
is integrated at `3a90ef3` with exact-archive evidence at `1766040`. Its12 source
and12 extracted tests pass, including strict negotiation, phase origin, bounded
Started/Stopped/closure envelopes and lost-first-Grant history lookup. History and
ACKs carry no live execution authority. Root independently verified18 source and26
evidence files. The format9 process evidence above also has independently verified
source, binary and log hashes. Worker producer, paired daemon owner and033 migration
qualification continue separately; this pure contract activates none of them.

The [actual phase host process union](../implementation/AQ-08-phase-host-process-evidence/qualification.md)
passes in6.99seconds with server test source `75a460e` and worker `2a818f4`
(original source `b799930`). Actual daemon HTTP/PostgreSQL, independent worker,
aq-git/Git, production host helper, tmux and the kernel now participate in one
probe. Execution is durable at revision27; local Running and ECHILD shutdown are
retained through revision34. Existing execution-only and input/expiry regressions
pass5.64s and6.97s with the same binary. Targeted strict lint and the binary's two
unit tests pass. Remote phase terminal ACKs, providers, renewal and task settlement
remain unqualified; the closed synthetic harness does not activate those gates.
