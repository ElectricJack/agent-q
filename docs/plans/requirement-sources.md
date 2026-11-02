# Requirement sources and historical task successors

The [2026-11-02 program plan](../superpowers/plans/2026-11-02-v2-full-application-plan.md)
is the tracking authority. This index supplies the requirement namespaces and the
historical provenance used by the MS4 acceptance matrix. The
[2026-10-30 JSON](0001-v2-milestones.json) and its generated Markdown are frozen.

The historical checker still validates sibling documents. In an isolated slot,
it finds siblings beside the Git common checkout. If those clones lack historical
evidence, explicitly set `AQ_PLAN_PLATFORM_ROOT` to the supplied read-only
platform reference directory before running `python3 scripts/check-v2-plan.py`.
Local aq-server documents and evidence always resolve in the current checkout.

## Requirement namespaces

Each prefix below has one owning source family. Multiple rows may cite the same
spec because it owns several disjoint ID series. The two worker contract parts
share continuous W and T series; each series is registered once with both files,
not assigned twice. Existing E/X/D/W/U/P/H/R/S/N/C, SBX and DEP IDs stay unchanged.

For existing numbered tables, retain the original numeric suffix: for example,
aq-git R1 is `GIT-R1`, performance P1 is `GIT-P1`, playbooks P1 is `PB-P1`,
and worker T21 is `WRK-T21`. Architecture P1 and R1 retain their original names.
These aliases apply to program tracking; they do not rewrite component tables.
The external top-of-spec alias notes are tracked as follow-up docs tasks.

For a source without an ID table, the assigned prefix identifies the document,
and a section citation identifies its requirement text. Use
`PREFIX:<exact heading anchor>` with the heading title and pinned source revision
in evidence, for example `SRV-MEDIA:png-and-canonical-rendering-policy` or
`GRAPH-1:<heading-anchor>`. Do not invent a numbered requirement or infer acceptance
from a section citation. A changed section must be reconciled against its pinned
revision. Prefix ownership is stable even if headings change.

| Prefix | Source | Citation / alias |
| --- | --- | --- |
| `E` | [0001-ecosystem-and-releases](../architecture/0001-ecosystem-and-releases.md) | Existing E acceptance IDs. |
| `X` | [0002-execution-and-recovery](../architecture/0002-execution-and-recovery.md) | Existing X failure-scenario IDs. |
| `D` | [0002-execution-and-agent-access](../specs/0002-execution-and-agent-access.md) | Existing D daemon requirement IDs. |
| `W` | [0001-worker-framework](../../../aq-worker/docs/specs/0001-worker-framework.md), [0002-execution-and-agent-access](../../../aq-worker/docs/specs/0002-execution-and-agent-access.md) | Existing continuous W requirement series across these two parts of the worker contract. |
| `U` | [0001-dashboard](../../../aq-dashboard/docs/specs/0001-dashboard.md) | Existing U acceptance IDs. |
| `P` | [0003-agent-profiles-harnesses-and-terminals](../architecture/0003-agent-profiles-harnesses-and-terminals.md) | Existing P IDs: profiles and intelligence. |
| `H` | [0003-agent-profiles-harnesses-and-terminals](../architecture/0003-agent-profiles-harnesses-and-terminals.md) | Existing H IDs: harness and host. |
| `R` | [0003-agent-profiles-harnesses-and-terminals](../architecture/0003-agent-profiles-harnesses-and-terminals.md) | Existing R IDs: remote terminal. |
| `S` | [0004-session-operations-waits-and-notifications](../architecture/0004-session-operations-waits-and-notifications.md) | Existing S IDs: session observability and recovery. |
| `N` | [0004-session-operations-waits-and-notifications](../architecture/0004-session-operations-waits-and-notifications.md) | Existing N IDs: human notifications. |
| `C` | [0004-session-operations-waits-and-notifications](../architecture/0004-session-operations-waits-and-notifications.md) | Existing C IDs: waits and continuations. |
| `SBX` | [0005-sandboxed-execution-and-deployment-topology](../architecture/0005-sandboxed-execution-and-deployment-topology.md) | Existing SBX IDs in section 10. |
| `DEP` | [0005-sandboxed-execution-and-deployment-topology](../architecture/0005-sandboxed-execution-and-deployment-topology.md) | Existing DEP IDs in section 10. |
| `GIT-R` | [0001-aq-git](../../../aq-git/docs/specs/0001-aq-git.md) | Alias `Rn` → `GIT-Rn`. |
| `GIT-V` | [0001-aq-git](../../../aq-git/docs/specs/0001-aq-git.md) | Alias `Vn` → `GIT-Vn`. |
| `GIT-P` | [0001-aq-git](../../../aq-git/docs/specs/0001-aq-git.md) | Alias `Pn` → `GIT-Pn`; performance P1–P7 in section 14.1. |
| `PB-P` | [0001-aq-playbooks](../../../aq-playbooks/docs/specs/0001-aq-playbooks.md) | Alias `Pn` → `PB-Pn`. |
| `PB-T` | [0001-aq-playbooks](../../../aq-playbooks/docs/specs/0001-aq-playbooks.md) | Alias `Tn` → `PB-Tn`. |
| `WRK-T` | [0001-worker-framework](../../../aq-worker/docs/specs/0001-worker-framework.md), [0002-execution-and-agent-access](../../../aq-worker/docs/specs/0002-execution-and-agent-access.md) | Alias `Tn` → `WRK-Tn`; continuous test series across the two contract parts. |
| `GRAPH-1` | [0001-aq-graph](../../../aq-graph/docs/specs/0001-aq-graph.md) | Section citation; no existing requirement-ID table. |
| `GRAPH-2` | [0002-v1-behavioral-compatibility](../../../aq-graph/docs/specs/0002-v1-behavioral-compatibility.md) | Section citation; no existing requirement-ID table. |
| `GRAPH-3` | [0003-unified-node-versioning](../../../aq-graph/docs/specs/0003-unified-node-versioning.md) | Section citation; no existing requirement-ID table. |
| `GRAPH-4` | [0004-shared-node-model](../../../aq-graph/docs/specs/0004-shared-node-model.md) | Section citation; no existing requirement-ID table. |
| `SRV-INFRA` | [0001-infrastructure-foundation](../specs/0001-infrastructure-foundation.md) | Section citation; no existing requirement-ID table. |
| `SRV-BIND` | [0003-project-execution-bindings](../specs/0003-project-execution-bindings.md) | Section citation; no existing requirement-ID table. |
| `SRV-INTEL` | [0004-intelligence-selection](../specs/0004-intelligence-selection.md) | Section citation; no existing requirement-ID table. |
| `SRV-MEDIA` | [result-media-png](../specs/result-media-png.md) | Section citation; no existing requirement-ID table. |
| `UI-PRODUCT` | [0002-product-design](../../../aq-dashboard/docs/specs/0002-product-design.md) | Section citation; no existing requirement-ID table. |
| `UI-API` | [0003-dashboard-api](../../../aq-dashboard/docs/specs/0003-dashboard-api.md) | Section citation; no existing requirement-ID table. |
| `WRK-PROFILE` | [0003-profiled-tmux-sessions](../../../aq-worker/docs/specs/0003-profiled-tmux-sessions.md) | Section citation; no existing requirement-ID table. |
| `WRK-OBS` | [0004-session-observation-and-suspension](../../../aq-worker/docs/specs/0004-session-observation-and-suspension.md) | Section citation; no existing requirement-ID table. |

## Open historical task → new task mapping

One row for each of the 66 old `in_progress` or `planned` tasks, in JSON order.
Successors are all YAML task keys whose `continues:` includes the old ID;
deferred tasks remain listed and are explicitly marked. This is provenance,
not a transfer of passed evidence or a change to task scopes or dependencies.
API-04 (review inbox discovery) and UI-07 (live decision inbox) have **no successor**
in the current YAML. That records a tracking gap, not an owner-approved deferral.

| Old task | Frozen status | New task keys |
| --- | --- | --- |
| AQ-08 | in_progress | `B2`, `B3`, `B4`, `C4`, `C5a1`, `C5a2` |
| WRK-08 | in_progress | `I0`, `B6a1`, `C6a`, `C6b`, `I4a`, `I4b` |
| AQ-10 | in_progress | `B3`, `D3` |
| GATE-04 | in_progress | `I0`, `I4a`, `MS0`, `MS1` |
| WRK-09 | planned | `F3` |
| AQ-17 | in_progress | `F1a`, `F1b` |
| AQ-12 | in_progress | `D4`, `D9` |
| AQ-13 | in_progress | `D5` |
| AQ-14 | in_progress | `D6` |
| AQ-15 | planned | `D1a`, `D1b`, `D7` |
| AQ-16 | planned | `D8` |
| WRK-15 | in_progress | `H1` |
| AQ-22 | in_progress | `H2` |
| AQ-23 | planned | `H3` |
| AQ-25 | in_progress | `H4` |
| WRK-16 | planned | `H5` |
| AQ-26 | planned | `H6` |
| AQ-27 | planned | `F4`, `F6` (deferred) |
| GATE-05 | planned | `D9`, `D11`, `MS3` |
| API-02 | in_progress | `F7`, `G1a`, `G1b` |
| UI-01 | in_progress | `E3` |
| UI-02 | in_progress | `G3`, `G12` |
| REL-01 | planned | `E4` (deferred) |
| REL-02 | planned | `E4` (deferred), `J6` (deferred) |
| REL-03 | planned | `E1a`, `E1b`, `E3` |
| PERF-01 | planned | `J2` (deferred), `J3` |
| OPS-01 | planned | `E1a`, `E5`, `J5` |
| AQ-21 | in_progress | `H7` |
| WRK-11 | planned | `I1` |
| WRK-12 | planned | `I2` |
| WRK-13 | planned | `I3` |
| WRK-14 | in_progress | `B6c`, `H7` |
| UI-03 | in_progress | `G10` |
| INT-02 | planned | `J6` (deferred), `MS4` |
| UI-04 | planned | `G10` |
| INT-03 | planned | `MS4` |
| GATE-06 | planned | `MS4` |
| PB-04 | in_progress | `D10` |
| API-03 | in_progress | `G2` |
| API-04 | in_progress | No successor in current continues lists. |
| API-05 | in_progress | `G9` |
| API-06 | in_progress | `G6` |
| API-07 | in_progress | `F1b` |
| API-08 | planned | `G4` |
| API-09 | planned | `G4`, `H2` |
| API-10 | in_progress | `F2`, `G1a`, `G2` |
| API-11 | in_progress | `G6` |
| API-12 | planned | `G6` |
| UI-05 | in_progress | `G11` |
| UI-06 | in_progress | `G3` |
| UI-07 | in_progress | No successor in current continues lists. |
| UI-08 | in_progress | `G9` |
| UI-09 | planned | `G7` |
| UI-10 | planned | `G8` |
| UI-11 | planned | `G5` |
| UI-12 | in_progress | `G10` |
| WRK-17 | in_progress | `B6a1`, `B6b`, `C6a` |
| WRK-18 | in_progress | `B5a`, `B6a1`, `B6a2` |
| UI-13 | in_progress | `G11` |
| UI-14 | in_progress | `G7` |
| API-13 | planned | `G4` |
| UI-15 | planned | `G5` |
| UI-16 | in_progress | `G8` |
| UI-17 | in_progress | `G10` |
| UI-18 | in_progress | `G7` |
| UI-19 | planned | `G7` |
