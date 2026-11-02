# AQ-25 internal verified-wait owner38 candidate

The internal Rust owner now persists bounded verified-wait plans under actual
AQ08 authentication. This is a tested foundation candidate, pending independent
review and parent integration. AQ-25/AQ-26 scheduling, agent suspension and wake
are still incomplete. Existing admission paths do not honor its gate yet.

`waits::Owner::open(state, keys, resolved_policy)` authenticates retained protected
records in one repeatable-read startup snapshot. Keys are loaded before SQL work.
No configuration field, default runtime, HTTP route, CLI command, public capability
or background polling loop constructs this owner in this increment.

Its internal API is deliberately small:

- `register` freezes exact bounded request bytes, stable operation/wait identities,
  actual machine principal, original policy/deadline and original receipt.
- `observe` returns retained state and enforces actual cancellation, fixed deadline
  or changed source authority. It can complete those transitions while source IO
  is still physically held; it is not a proposed pure public inspection GET.
- `hint` marks a member dirty without accepting a verdict or cursor.
- `drive_clock` claims one bounded round, commits, observes the actual configured
  clock, then reacquires current authority and applies the pure plan.

The separately compiled `test-support` factory/port qualifies fake source owners.
Default construction rejects retained `test_fixture_v1` histories. Default
registration supports Timer predicates only; Graph/mailbox predicates refuse.
The existing task-mailbox identity and head lock are reused as an ordering seam,
not advertised as a qualified mailbox observation adapter.

The owner receives an opaque `AuthenticatedNode`, derives lifecycle identity from
actual AQ08 authority plus verified retained ProfiledOffer, and compares the
request Binding against that identity. `GraphActive` checks current registered
boot and grant; `IdentityOnly` inspects actual cancellation without inventing a
new grant. Current authentication and clock eligibility are checked after waits.
The original issued grant identity remains immutable under the existing AQ08
owner. No caller-selected Binding or cancellation boolean grants authority.

Admission locks installation/project budgets, project cycle guard and task gate
before node/attempt/hot authority. Progress omits budget locks. The existing
mailbox head precedes registry/member/work/slot rows. External source callbacks
run after claim commit, behind component-global nonwaiting capacity2 and per-wait
activity retained by the actual blocking job. Async cancellation/timeout cannot
free a still-running callback. Fresh lease/ticket/dirty/authority/deadline checks
refuse late results. No external source IO occurs under SQL locks.

The [allocation contract](AQ-25-owner38-envelope.md) reserves one1MiB envelope
and all eight slots before acceptance. Terminal and cancellation slots cannot be
spent on progress. Known-cycle history includes virtual wait-to-owning-task
bridges, with trigger-assigned activation/retirement project revisions. These are
internal locked registry revisions, not Graph committed-source cursors or wake
proofs. Non-Timer fixture predicates retain external-closure uncertainty.

The schema guard compares280 frozen catalog objects, including actual definitions
and internal FK trigger state. Configured open additionally authenticates AEAD,
original bytes/actor/policy/IDs, snapshot/index/work, cycle receipt, terminal,
cancellation and resume/completion projections. The catalog reference was generated
only from the reviewed migration on a new disposable PostgreSQL17 database.

[Durable qualification](evidence/aq25-owner38/qualification.json) records20 actual
owner cases in20.08s,18 pure cases in0.10s, three SQL upgrade/drift cases in2.22s,
and selected strict checks. Actual fixtures use synthetic launches/clocks/source
owners; they perform no provider, worker-host or live Graph activity. Earlier
compile, projection and fixture failures are retained alongside the fixes.

No canonical database, existing live Graph, service, backup or media fixture was
read or changed. No production binary was built. Frozen migrations1–37 remain
unchanged. A future format38 promotion needs its own reviewed backup/rehearsal;
an old37 binary refuses a38 database, so format rollback requires restoration.
