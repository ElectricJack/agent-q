# Genuine host-start reports and launch-owner journals

This checkpoint adds the missing transition between an execution grant and the
daemon's last-known Running phase. It does not infer a start from a grant, renew
counter, parser event or transport registration. AQ-08 remains in progress until
the actual worker launch owner produces these reports as part of native work.

## Evidence and identity

The additive `aq-admission 0.1.3` started module preserves existing preparation,
transport and execution-spec artifacts. A bounded request contains the exact
LaunchBinding and grant, logical guardian/launch/reservation owner, admitted
runtime and qualification, original started operation, journal revision/digest,
and a typed Running observation with an independently bound canonical digest.

The observation matches the public JSON shape in
`aq-worker/crates/aq-session-host/src/lib.rs`: guardian launch intent, manifest
and local configuration digests, physical nonce, machine boot/PID namespace,
process identity/start ticks, tmux server/pane identity and local permit clock.
The physical nonce is allocated after launch permission, so it is deliberately
distinct from the logical reservation owner established before a grant.

The actual host saves Running state in `supervisor.rs`, checks guardian liveness
through `SessionHandle::observe`, and invokes `LaunchAuthority::record_started`.
That callback must persist the original observation and its binding to the
logical owner before transmitting. The daemon verifies identity, scope and
current authority; it relies on the explicitly qualified worker owner for the
remote process observation. This is delegated evidence, not independent kernel
attestation or proof that a process remains alive indefinitely.

## Atomic owner transitions

Migration 018 adds one bounded launch journal head and one original start receipt
per attempt. `journal::record_reserved` runs after Hot becomes Preparing with its
actual logical owner. `record_prepared` runs after the actual grant is installed,
within the same transaction. Both callers hold node→attempt→hot locks. Their
revision/digest comes from the durable worker journal, never a request hash.
The head advances Reserved→Prepared→Started with strictly increasing revisions;
same-phase exact replay is a no-op and changed content conflicts. This head covers
launch transitions, not an invented ledger of subsequent renewals or stops.

The Started service carries opaque authenticated node context into the transaction
and checks the current credential under the node lock. It requires actual native
registration and the exact installed global configuration anchor and runtime
qualification. Config allowlists alone cannot invent registration. It checks the
sealed binding, current grant, logical owner and actual Prepared journal head,
then samples the server clock after the final owner lock.

The original receipt, Started journal head and Hot Running projection commit
together. Deferred SQL checks prevent a receipt being committed without that
transition; the journal cannot become Started without its original receipt.
For native metadata or an owner-journal-backed attempt, raw Running projection
updates require the same original record revision. Explicit historical
compatibility fixtures remain separate from native worker evidence.

Starting changes only the phase and authority/record revision. Grant identity,
grant revision, expiry, hard deadline, task claim and source/effect state remain
unchanged. Replaying an exact original report returns only its historical receipt
and never updates Hot, even after a stop or expiry. Changed operation/body reuse
conflicts. A new first report after cancellation, expiry or stop is refused.

## Runtime surface and limits

Migration 018 and native-registration migration 019 are registered in order;
server/container state format is 19. The production worker listener mounts
`POST /v1/worker/profiled/started` with a 16 KiB bound and
`POST /v1/worker/profiled/register` with a 64 KiB bound. Both use reserved control
admission and SQL connections and the common current-node credential check.
The human listener exposes neither mutation. Startup installs the explicit policy
configuration before serving and refuses a stale global configuration revision.

The original Started request and receipt are each bounded to 16 KiB, with one
row per existing attempt; the journal binding is bounded to 4 KiB. This is a
logical row bound, not a physical disk or process memory guarantee. No transcript,
credential bytes or caller-selected executable enters this surface.

SQL/router tests use explicitly synthetic host/runtime fixtures. They cover
atomic rollback, exact and changed replay, stop/expiry non-resurrection, current
credential rotation, missing Prepared/native authority, direct projection and
journal bypass refusal, and a real observed SQL lock wait crossing grant expiry.
They do not qualify a native worker producer, selected harness or remote kernel.

Worker ordering remains: actual durable Started→central acknowledgment→fresh
source/terminal admission while execution authority is live. First source
registration after a partition and completed stop remains unsupported until an
explicit historical owner path is designed and qualified. No source or terminal
permission follows merely from possessing a Started receipt.
