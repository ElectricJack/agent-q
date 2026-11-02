# First explicit refresh static output attempt

Source `1139ced2720b7d869fad258e3f2f5fe1f95a5e5e` was independently source
accepted after offline compile and selected strict checks. The worker was
`06650543ad29db72573ac7775320008ccaed3c2e`, version 0.4.0/FORMAT11, using the
qualified admission 0.1.10 and terminal 0.1.1 refresh contracts. Exact binary pins,
original policy bounds and prelaunch free space are in [preflight.json](preflight.json).
This was one actual static-only run under the standing conditional integration
authorization; no Codex, external provider, browser or canonical state was used.

The case **failed** at constant stage `phase_output_attach` after 18.624s (exit 101).
It is not accepted-output or lifecycle-ACK evidence. Both acceptance calls and
Prepare returned HTTP 200. The real post-Prepared operator assessment recorded new
sequence 3 after revalidating the actual journal, worker identity, worktree,
synthetic private bytes and static program/helper/tmux pins; sequence 2 was kept.
The original preparation/grant/health/source-age and default Git bounds were
unchanged. A real grant was issued and actual baseline/Running/Stopped history
was retained with `kernel_wait_echild=true`. Both Started ACK and Stopped ACK were
absent. No human Watch, output upload or authorized tail was reached.

The source cause is a remaining nominal consumer join: worker
`native/launch/output_relay.rs` retains only frozen `StartedRequest` and reads
`history.terminal` in both attachment and registration. The refresh physical
owner correctly writes `history.refresh_terminal`. Attachment therefore refuses
at its existing missing-outbox guard. This is not an admission, health, deadline
or output-transport failure. The bounded proposed fix is nominal old/new retained
Started/ACK dispatch inside the same relay, preserving its actual live host,
tails, sequence ownership, Register wire body, quotas and network lane.

[run.log](run.log) records fixed diagnostic classes, numeric clocks/flags and
HTTP statuses only; no raw error chain, auth or private material body is dumped.
[result.json](result.json) records elapsed/exit time. A post-run scan found no
process executing the pinned test, worker or static peer. The durable physical
ECHILD observation is the stop evidence; absence of a process alone grants no
release authority. No second attempt has run. Earlier historical/frozen-profile
negative evidence remains unchanged in
[the original record](../accepted-output-watch-tail/README.md).
