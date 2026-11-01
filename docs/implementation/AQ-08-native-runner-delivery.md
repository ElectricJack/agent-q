# Native production runner: next bounded delivery

Baseline: agent-q `1b7e4f5` / state 33, paired with the current native worker
components. This is an implementation delta, not runtime/model qualification.
No provider request is part of this increment.

The daemon's phase pipeline is usable by the controlled worker process, but the
ordinary worker executable has no native run command. `RunExecution` uses the
legacy synthetic protocol. Native launch is compiled only for tests/test-support
and accepts `SyntheticHostProfile`, a closed small-ELF adapter with a 4096-byte
startup ceiling. Removing that guard or renaming the profile would not produce a
qualified real-agent path.

## First worker increment

Add a foreground native runner and an installed-adapter boundary, using existing
native journal, source/material preparation, exact Grant, Started/Stopped and
output owners. Validate the private installed configuration and supported adapter
before registration/polling. Missing executable pins, qualification/counting/model
facts or required scopes must refuse admission; do not advertise a harness and
then consume an offer that cannot run. Keep the synthetic adapter test-only.

Use a closed adapter selection from operator-owned configuration. A task cannot
supply executable paths, arbitrary argv, shell hooks, provider endpoints or
credential values. The connector keeps its current enrolled node/service-token
credentials. The agent receives only its exact scoped provider binding and,
where qualified, its separately issued graph session credential.

The first CLI adapter must implement the existing
[worker adapter plan](../../../aq-worker/docs/implementation/WRK-08-codex-adapter-plan.md):
exact pinned executable, one final payload over a bounded stdin/file handoff,
isolated configuration, supported manifest settings, and actual observed readiness.
A wrapper PID or pre-exec marker is not readiness. Source, credential and startup
verification repeat at the actual exec boundary; output draining remains independent
of network delivery. Test with a pinned local no-provider program before any real
provider target is considered.

`LocalCommand.environment` currently participates in serialized host launch
metadata. It is not a secret handoff. The adapter should reuse `MaterialStore`’s retained private installation owner,
with the existing record, exact lease/binding/fingerprint and stopped-writer
cleanup. The first slice adds no second credential store or secret files; its
closed exec handoff must preserve that owner’s private access boundary. Never copy a
provider secret into public artifacts, argv, sealed launch metadata, logs, or the
connector's ambient environment. The worker owner should settle that concrete
exec handoff before exposing an eligible production adapter.

## Existing lease capacity, without renewal

The current 10-second execution lease is a fixture setting. Frozen admission 0.1.9
already permits an operator policy `maximum_execution_lease_ms` from 1 through
120000. A new immutable policy/qualification version may request a longer initial
lease for a bounded task. The daemon clamps it to the original execution wall and
lineage deadline; the current regression profile has a 60-second hard bound.
This changes neither an existing receipt nor its delivery clock. Original offer,
preparation and material-retrieval clocks remain immutable and may expire before
the running execution ends.

Phase terminal negotiation is explicitly `OriginalLeaseNoRenewalV1`. The legacy
Renew owner is not a phase renewal adapter. Do not let the runner silently renew,
reselect, restart, extend the hard deadline or infer an installed credential from
an expired retrieval lease. A task exceeding its initial permit stops under the
existing guardian. Longer execution needs a separately versioned continuation
owner and actual current-fact/worker integration.

Restart remains stop/preserve and authenticated historical reconciliation. A new
logical boot cannot inherit an old live Grant. Result recording, reviewed graph
settlement and verified source/effect release remain separate owners; neither
process exit nor a terminal ACK completes the task.

## Ownership and qualification boundary

Worker: command/service/config, closed installed adapter, private exec handoff and
ordinary native lifecycle orchestration. Server: retain the existing phase owner;
add no endpoint or table merely to duplicate its policy. The accompanying owner
regression checks explicit 30-second policy and 120-second policy hard clamping,
immutable original clocks, and exact Grant replay. It does not widen existing
fixtures or qualify a provider, tokenizer, sandbox, hard-cost ledger or real CLI.

The [installed adapter input boundary](../../../aq-worker/docs/implementation/WRK-08-installed-adapter-handoff.md) is now implemented and independently reviewed. It validates an operator-pinned manifest and installed executables, and borrows exact retained private material without adding a store or journal. Four adapter tests, five material tests, strict lint and default library checks pass. It deliberately has no command constructor or launcher; ordinary native orchestration and the concrete harness handoff remain next.

## Eight-subagent execution allocation

The [current parallel delivery plan](../../../aq-dashboard/docs/plans/0003-live-dashboard-delivery.md) assigns two independent native owners alongside six dashboard owners and root integration. WRK-17 owns the concrete adapter, startup/private configuration, observed readiness and launch/host boundary. WRK-18 owns installed preflight, command/configuration and production orchestration. Root and both owners agree a narrow executor/preflight interface before implementations depend on it; isolated executor peers allow orchestration tests while the concrete adapter progresses.

Both tasks have completed foundation prerequisites and can begin independently. WRK-08 remains the final concrete join and real runtime qualification gate. An executor peer never establishes advertised runtime eligibility. Once native work becomes sequential, one owner can complete the join while the freed slot takes ready dashboard work. Shared target caches and heavy test windows are assigned separately from source ownership.
