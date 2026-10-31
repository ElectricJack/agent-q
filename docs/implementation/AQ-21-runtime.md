# AQ21 terminal relay candidate

Worktree: `/home/jkern/dev/agent-q-aq21`, base `86d28d7` (AQ08 hot bridge013 and mounted AQ22 observation014; the
redundant private profiled-fixture cherry-pick was skipped against main). The
reviewed schema/shared-inspection012 is main`7769508`; its SQL/helper remain
unchanged here. Final source/evidence hashes will be frozen after qualification.
Terminal runtime source remains uncommitted, unpublished, undeployed and not product-pinned.
The shared terminal-policy bridge is cherry-picked as`fab1c45` from main`eff6fa9`.

## Authority and durability

Startup explicitly commits `meta.terminal_relay_owner=<runtime UUID>`. Ordinary
requests cannot install/reclaim ownership. Every authority-bearing transaction
locks/checks that row FOR SHARE before common node/attempt, current users/ACL,
terminal ledger and deterministic child rows. A new install waits for old
transactions, then fences all old handlers and maintenance. Installation modifies
only meta; recovery runs in separate transactions. Terminal and notification
configuration/owner fences remain independent.

Registration/control/dispatch use the common `TerminalActive` bridge and exact
sealed TerminalPolicy, including on cold and verified-cache paths. The protected
ledger persists that policy; every load compares it to the bridge, never protocol
maxima as a substitute. Smaller input/pending/frame/replay limits remain binding;
output-only terminals cannot acquire control or enable input epochs. All bind exact full
session/node/boot/attempt/generation, immutable terminal epoch and source seals.
Historical watch/receipt reads use `TerminalHistorical` plus fresh current-role
and exact-session inspection ACL checks. Owner grants/revokes inspection and
control independently, with separate revisions. Inspection never implies control,
graph access or execution permission. Replacing a host requires a new execution
attempt; registration only reconciles the same surviving exact host to a newer
current authority revision. Central runtime registration is unsupported.

The encrypted canonical ledger, projections, metadata receipt and encrypted outbox
commit in one SQL transaction before any effect is exposed. Input outbox rows are
already claimed one-shot attempts, excluded on every later poll regardless of
transport cursor. Lost responses can cause unknown delivery, never blind replay.
Every dispatch repeats current role/ACL, live authority, lease, generation,
deadline and canonical FIFO checks. A poll scans at most8 distinct-terminal FIFO
hints; active in-flight terminals cannot block others. A principal whose current
scope/authority is absent has only provably undispatched commands durably Rejected;
IDs/digests and every dispatched/unknown record remain. This is refusal, not
retention pruning, execution cancellation or evidence of stopped writers. Input candidate rows are scheduling hints;
the canonical protected record validates identity and principal before dispatch.
Control replacement/release/revocation rejects unclaimed input and records claimed
input unknown. Fresh capture tokens and snapshot identities are bound to current
controller generation and post-uncertainty capture; old snapshots cannot resolve.

Poll/upload correlation IDs are transport correlation, not authority/idempotency
keys. Enclosed mutation/receipt/fence/capture identities carry immutable replay
semantics; partial upload progress is safe to retry by those inner identities.
No cursor authorizes an input retry or acknowledges a fence. Poll holds require
actual current subscription/controller work, one held poll per node, and release
all SQL connections/common locks before waiting. Opaque common worker_api
AuthenticatedNode contexts retain only a private zeroizing verifier; fresh locked
credential_hash/revocation and boot checks occur after waits, including poll wake. A held poll never renews execution.

## Protection, retention and bounds

Local owner-only regular single-link32-byte key files, max8 active/retained keys;
no symlink/FIFO reads, logging of secret bytes or plaintext fallback. Random-nonce
AEAD format/key-ID binding is shared in `protected_storage`; typed terminal AAD
includes complete terminal identity, ledger revision and ledger/outbox purpose
(operation and ordinal). Observation contexts/keyrings remain independently typed
and configured. Missing keys/corrupt or swapped ciphertext fail closed. Retaining
keys needed by existing records is an explicit operator obligation.

Hard candidate limits:4096 registrations;1MiB encoded canonical ledger/session;
1024 finite generic operation IDs and4MiB metadata replies/session;32KiB pending
literal input/session;64 epoch receipts with retired monotonic prefix;128 ACL rows
per scope;128 outbox rows and256KiB encrypted effect payload/session;4 watchers/
session,32 subscriptions and32 active/unacknowledged controller fences/node.
Generic mutation/ACL/capture/resize IDs are not pruned into replayable absence.
Long-lived generic receipt retention/normalization remains a release limitation;
input epochs alone do not qualify unlimited attach.

Disposable output:128 session rings globally,32/node; each retains at most1MiB
payload,1024 frames,64 merged gaps and1024 snapshot-ID digests. Metadata/allocator
and cloned response overhead are additional bounded costs. Session output token
bucket256KiB/s, node2MiB/s; duplicate/barrier-retired data is checked before charging.
Snapshots share those quotas. Dropped frames produce explicit gap acknowledgements;
output never consumes execution-control request slots. No snapshot is fabricated
on restart; reads return Pending/resynchronization until a real snapshot exists.
Ready pages carry at most32KiB frame bytes/128 frames and at most1MiB JSON total.

Terminal admission:32 poll,4 data upload,4 human read and8 metadata/control slots.
Dedicated relay SQL pools total6 lazy connections:4 control and2 read/data,
with1s acquire,750ms statement,100ms lock and2s idle-in-transaction timeouts.
The metadata-only control-upload endpoint rejects any frames/snapshots/gaps and
has a256KiB body bound; raw-output uploads cannot select its reserved capacity.
The worker adapter must explicitly prioritize control-upload for ACKs/receipts.
Poll dispatch considers at most8 distinct terminal FIFO heads. A terminal with an
in-flight command cannot block another terminal's head. A provably unauthorized
never-dispatched input becomes durably Rejected while preserving its operation
ID/digest; dispatched and Unknown receipts remain untouched. Every candidate is
revalidated under current owner, authority and scope locks, without external I/O.
Execution renewal/cancellation, observations and notifications retain separate
pools/classes. Caller-owned shared lock contention still needs operating evidence.
HTTP poll/upload/interactive deadlines remain bounded. Shared authority locks can
still contend: separate pools do not prove a500-agent latency envelope. Full
ledger validation/encryption/projection remains bounded history work; no throughput
or disk-capacity claim is inferred from finite correctness tests. Database index,
WAL and allocator overhead require separate operating-envelope qualification.

Maintenance processes at most16 candidate rows per250ms tick, uses an advisory
in-memory scheduling cursor only, reloads/authenticates authoritative bytes under
owner/common locks, expires deadlines and recovers restart uncertainty. It never
dispatches input, launches, renews controller/execution or extends permissions.
Failures log fixed messages/terminal IDs, never input/output/ciphertext details.

## Qualification scope

Retained candidate logs are under `target/aq21-runtime-evidence` and `/tmp/aq21-*`.
Disposable PostgreSQL17.11 uses the exact same pinned image/durability settings as
migration012,2 CPUs/1GiB; no existing database is touched. Tests use the actual
synthetic native profiled factory with an explicit synthetic Running grant, not a
legacy state promoted into terminal authority. They qualify SQL/relay fences,
not runnable-model/credential/host qualification. Initial fixture evidence is
retained where foundation correctly refused live incarnation replacement.

The independent aq-session CLI uses frozen aq-human-http and exact copied
observation API DTOs. Watch-default JSON attach, explicit control/literal input,
resize/lease fencing, receipt backpressure and unknown-stop behavior use human
routes only. Its bounded JSON output is intentionally not a qualified renderer.
Actual provider/tmux ownership, WRK14 peer bridge, central adapter, product CLI
alias and final cross-runtime integration remain separate prerequisites.

The pre-fairness frozen113-file snapshot completed610 workspace tests with4 existing
opt-in ignores plus strict workspace lint/format. It remains separately retained
under `target/aq21-full-pre-fairness`. Final source adds bounded fairness and two
actual same-node multi-terminal cases, using the shared synthetic factory with an
explicit exact-project sibling insertion path. No migration or published artifact
bytes changed. Final qualification follows that correction.

The first final114-file full run failed in the unchanged updater executable-script
fixture with Linux ETXTBSY. Its log is retained under
`target/aq21-full-updater-etxtbsy`; the unchanged focused updater target passed all28
tests. A standalone deterministic fork reproduction demonstrates that a child
inheriting an O_CLOEXEC script-writing descriptor can prevent direct execution
until that descriptor closes. It identifies a plausible fixture mechanism, not
the actual inheriting process from the failed suite. No updater code, automatic
retry, ignore or serialization mask was introduced. The unchanged candidate is
qualified in a separate full rerun; the failed run remains part of the evidence.

Final qualification of the exact114-file candidate completed613 workspace tests
with4 existing opt-in ignores, strict all-target/all-feature workspace Clippy and
format verification. All114 frozen SHA-256 source hashes matched after the run.
This includes21 actual terminal PostgreSQL/API cases,24 pure fake-peer cases,
deterministic schema/fixture checks and the actual fake-HTTP CLI tests. The
unchanged updater fixture passed in this final run. Immutable copies of the final
run, failed updater run and earlier pre-fairness evidence are retained in
`/home/jkern/dev/.aq-qualification/aq21-relay-2026-10-31` with a hash manifest.
These are bounded correctness and packaging-contract results, not a general
long-lived attach or throughput qualification.
