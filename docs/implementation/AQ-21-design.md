# AQ-21 terminal relay: staged implementation and missing seams

Status: isolated runtime candidate on base `86d28d7`, branch
`task/aq21-terminal-relay`, worktree `/home/jkern/dev/agent-q-aq21`. The approved
pure architecture now has an encrypted PostgreSQL adapter, scoped human/node
HTTP routes and the aq-session CLI candidate. Reviewed migration012/shared ACL
is already on main;013 hot authority and014 observations are preserved. AQ21
remains in progress until final source/qualification review. No product capability,
worker bridge, central adapter, renderer or live provider is qualified by this
candidate. See `AQ-21-runtime.md` for current ownership, bounds and evidence.

## Published contracts and identity

Use `aq-session-protocol =0.1.0` and `aq-session-api =0.1.0` unchanged. Browser
requests carry no principal. Authentication supplies the principal and a current
scope decision; graph access profiles never authorize terminal viewing/control.
All operations bind the complete TerminalIdentity, including remote node, boot,
attempt, assignment generation and session, or independent central authority,
session and generation. A central session is not represented as a fake worker.
Registration requires a trusted authoritative session binding and execution
expiry. Watching or acquiring control never launches or renews execution.

## Pure core boundary

`aq-terminal-relay` is an independent pure crate, with no server/routes, process,
network, SQL or tmux dependencies. Durable state is a versioned bounded ledger;
volatile output is a separate bounded replay ring. Planners return a candidate
ledger plus typed effects. A storage adapter must atomically persist that ledger
under the expected revision before effects become dispatchable. Fake peers model
successful/failed commits and crash recovery; mutation of a pure Rust value is
never labelled durable persistence. Production adapters must acquire the exact
session row, re-read current authorization/execution authority and sample a live
clock after locks, then CAS all control/input metadata and dispatch outbox changes
in that transaction. Cached pre-lock authorization is insufficient.

Trust inputs are explicit: current principal, scope decisions, terminal identity,
session revision and execution deadline. They come from trusted server joins, not
request fields or worker claims. Each mutation and watch read checks its own
operation permission/current scope and expected session revision. Durable replay
is bounded (initially 1,024 mutation IDs); reaching the cap refuses new work rather
than silently forgetting identity conflicts. A later retention design needs an
explicit operation/replay horizon, not eviction followed by possible reinjection.

## Controllers, input and crash semantics

A new grant increments a monotonic generation, freezes its lease identity/principal
and expiry (at most 15 seconds, never past execution authority), and records its
operation digest. Transfer waits for acknowledgement of that exact higher fence
or conservative expiry of the last *possibly delivered* old permit. Central
revocation stops new input admissions immediately. Sending a response is not proof
the peer observed the fence. A pending grant alone does not allow input. An active
lease cannot be silently extended beyond its last issued permit; explicit renewal
requires its own durable operation and renewed permit delivery protocol.

Input admission serializes with control changes, freezes shared InputCommand bytes
and command digest, and commits Queued before dispatch. Delivery commits Received
metadata; injection is peer-owned and must durably enter Injecting before local
PTY access. The daemon does not claim injection from dispatch. Peer receipts must
match exact command digest/terminal and legal monotonic transitions. Queued input
expires within two seconds; received/injecting/dispatched input is conservatively
unknown after crash/lost definitive observation. No reconnect queue redelivery of
unknown input, and no reacquisition clears uncertainty. Later interactive input is
paused until a server-issued post-uncertainty capture token and exact snapshot
are explicitly resolved by the current controller. Quiet output sequence alone
cannot prove freshness. Raw input is sensitive durable delivery material, never
general audit text; production at-rest protection/retention is a review gate.
Resize is a separately ordered idempotent setting, with immutable operation IDs
and monotonic sequence, not an input-replay workaround.

## Output and subscriptions

Only the authenticated registered peer can supply output for the exact session.
Frames validate length/digest, epoch and monotonic sequence. Exact retained replay
is deduplicated, changed bytes conflict, and old out-of-retention sequence IDs are
rejected/resynchronized rather than newly accepted. Missing output and evictions
become explicit bounded gaps. Snapshot capture identity, epoch and watermark are
preserved; replay applies only frames strictly after the snapshot barrier.
Overlapping unresolved gaps remain visible. A watermark is peer assertion until
WRK14 qualifies actual tmux/renderer capture ordering; core tests do not qualify a
renderer, terminal escapes, Unicode/full-screen modes or real TUI reconstruction.

Up to four scoped independently expiring viewers per session, 32 per node; one
controller, 32 KiB pending input/session, 1 MiB replay/session, bounded frame count
and gap metadata. Active central aggregate replay must stay under 128 MiB, node
output below declared per-session/node rate and wire limits. Idle sessions have
no terminal polls; one bounded 20-second terminal long poll per active node.
Sessions restarting/disconnecting expose explicit stale/gapped observation state.
Slow viewers never backpressure execution; replay is lossy and durable result
artifacts remain separate. Disconnect/restart requires explicit resynchronization.

Terminal admission has its own bounded budgets and pool, never the existing
execution `Class::Control` semaphore or `Store::control_view()` pool. The pure
capacity model tests independent terminal/control reservations; actual HTTP pool
and executor starvation tests remain integration acceptance.

## Missing wire and human API seams (must not overload v1)

Published TerminalCommand has only Controller/Input/Resize/Watch; NodeOutputBatch
has only input receipts, frames, snapshots and gaps. It cannot express:

- A fence acknowledgement binding exact lease, generation and terminal; explicit
  revoke fence acknowledgement without granting another input principal.
- A server-minted fresh-capture request with the uncertainty/controller boundary.
- A resize disposition and safe ordered setting acknowledgement.
- Human lease renew/release/control-status, capture request and subscription
  renewal/unsubscribe responses.

The core may use internal typed effects/observations for these seams, but they are
NOT a compatible extension of published v1 wire documents. No unknown JSON fields,
Watch overload or synthetic empty lease grants. Versioned contract publication and
consumer refusal/negotiation must precede routes/transport. Existing shared types
are reused for all identities, leases, actual input/resize, frames and snapshots.

## Proposed production storage, not an applied migration

After review, a dedicated migration (number coordinated with AQ08) would add:
terminal_sessions (authoritative registration/source/project/current identity,
revision/execution fence/controller generation/expiry/pending fence/uncertainty);
terminal_subscriptions (scoped principal/session expiry, unique operation digest);
terminal_commands (immutable request+command digests, sensitive encrypted command,
expiry, disposition, dispatch evidence, peer receipt); terminal_capture_requests
(unique token, post-uncertainty ledger revision, controller identity, exact
registered snapshot result); transactional terminal outbox and metadata-only audit.
Constraints/locks enforce one controller, exact session bindings, pending-byte and
operation count caps. Input state/outbox commits atomically; network outside locks.
Output bytes are disposable memory buffers with explicit restart gaps, not a
second durable session authority. Existing sessions/execution tables must be
examined for a canonical source join before choosing new registration columns.

Every lookup checks current human scope or registered peer identity. Browser
responses contain no node credentials or private tmux socket path. Central bridge
uses a typed local adapter, same authorization and ledgers, not worker tokens.
Old binaries refuse a new state format; version_info/readable format/updater tests
and rollback evidence change with any actual migration. No terminal migration is
installed by the pure-core increment.

## Later implementation sequence

1. Review pure ledgers/fake-peer cases and missing versioned contract seams.
2. Publish reviewed transport/human lifecycle additions; add SQL integration and
   authentication/pool tests in a separate reviewed increment.
3. Add same-origin human watch/control APIs and bounded worker-initiated exchanges;
   central local bridge shares core. No inbound node route, SSH, socket exposure,
   mandatory node WebSocket or arbitrary tmux command endpoint.
4. `aq session attach` consumes those exact authenticated APIs: read-only by default,
   explicit control acquisition; literal input/resize only while authorized, with
   visible uncertainty/gaps/age/truncation and explicit disconnect/release. No
   implicit launch and no domain approval through terminal text. AQ's CLI placement
   remains an integration seam: this workspace currently has no aq CLI crate.
5. Qualify WRK14 actual terminal snapshots/barriers, latency/rates/watch concurrency
   under stated RTT/output load and reserved execution-control stress. Until then
   label snapshots read-only development output and keep terminal capability off.

## Reviewed implementation checkpoint and current integration base

The isolated branch now bases on common-authority commit `84f00a8`; AQ08 migration
010 is real. AQ24 was allowed to take migration011 so it can complete independently;
AQ21's next terminal migration is **012**, coordinated with root. No terminal SQL
file was written under the prior reservation. Runtime integration remains underway.

Portable worker-facing `aq-terminal-relay-protocol` depends only on the exact
session protocol and serde/schemars/UUID/hash machinery; human wrappers and the
pure bounded ledger live in `aq-terminal-relay`. Nineteen core/publication tests
pass, including frozen independent UUID vectors and a deterministic 24-JSON-file
schema/fixture publication. These are candidate contracts, not runtime acceptance.
`AsyncJournal` supports a real SQL COMMIT before effects become observable; no
fake synchronous staging acknowledgement substitutes for durability. Input outbox
effects are already-claimed one-shot attempts and cannot be automatically replayed
by a generic outbox dispatcher after lost acknowledgement.

Epoch input is separately negotiated and selected immutably at registration.
UUIDv8 derivation binds full terminal identity, nonce, lease, controller generation
and monotonic input sequence. A64-receipt window and protected retirement highwater
bound hot history; unknown commands remain pinned across epochs/restarts. The
finite endpoint refuses epoch-mode sessions. Old epochs/retired sequences always
refuse, even exact retries; retained retries expose disposition without reinjection.
All non-input mutation IDs remain finite and unpruned; this is not yet a general
long-lived-control capacity claim. SQL will retain normalized metadata/receipts
and enforce explicit byte/count ceilings rather than quietly dropping identities.

The shared `aq-human-http` helper is frozen and reviewed for both terminal and
notification CLIs: private single-link JWT file, HTTPS production origin, redirects,
proxies and retries disabled; exact Origin/CSRF mutation headers; bounded request,
streamed response and timeouts. Explicit development mode is feature-gated and
loopback-only. Its five fakeHTTP/private-file tests and strict default/all-feature
Clippy pass. Local ChaCha20Poly1305 protection has three passing focused unit cases
for context/ciphertext swap, rotation/missing keys and unsafe private key files.
Real PG/ciphertext/restart/pool and scopedHTTP tests are still required.
