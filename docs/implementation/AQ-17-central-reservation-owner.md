# AQ-17 durable central mailbox reservation owner

This design freezes the first durable reservation increment at daemon `72281b8`.
Root reserves the next migration number, 37, after Media's 36 is accepted and
imported. No migration is applied to the canonical database by this work. The
[first pure authority core](AQ-17-central-authority-core.md) is already separate.
This increment adds no HTTP route, daemon driver, provider startup, credentials,
Run, session, execution generation, host/dispatch ID, profile pin or API-07 runtime
availability. Its useful effect is an actual durable, recoverable reservation of
existing human mail, with current authorization and cancellation before startup.

## Actual owners and limits

`agent_access/messaging/global.rs` checks the full current Owner row under lock,
then resolves the immutable schema-35 global conversation. `global/writes.rs`
admits immutable human mail and send receipts through the existing private
`messaging.rs::append`. `mailbox_messages` owns bodies/envelopes, `mailbox_heads`
owns cursor order and `mailbox_operations` owns original send idempotency. New
central tables must not copy bodies, increment that cursor, create another send
receipt, or modify schema-35 retention counters.

`migrations/0005_messaging.sql` requires an execution-attempt FK for both
`mailbox_deliveries` and `mailbox_receipts`. Central reservations never use those
tables or fabricate an attempt. `mailbox_messages.payload_digest` is the original
AQ-11 envelope digest, not a SHA-256 of just its body. Exact source references
retain that original digest. A separate body hash, if inspected, has a different
meaning and cannot replace it.

Only current Owners share this installation's global reservation scope. Project
selection and session ACLs confer no access. This is installation policy, not a
request to impersonate a worker or the original human sender. A registered actor
is a genuine new durable logical actor tied to the existing real conversation.
Its actual closed local policy/config bytes are validated and retained with their
canonical digest; no invented catalog, composition or credential reference is
needed. The unavailable profile/HostPlan bridge is represented explicitly.

The initial fixed reservation policy permits up to 16 complete messages, at most
65,536 original UTF-8 body bytes, and at most 128 examined mailbox rows per query.
Every single current body (at most 16,384 bytes) fits the raw-body budget. A query
can be partial; its scan cursor is a scheduling hint, not an acknowledgement or
delivery cursor. Unexamined or held mail is not reported empty or consumed.
Selection is ascending mailbox cursor within a pinned upper mailbox head. Stop
before a message that would exceed the selected count/byte limit; do not silently
skip it as consumed. A bounded next scan can continue past previously examined
held inputs; wrap/reobserve after the pinned head so a released old reservation
can become eligible again. There is no background scanner in this slice.

Reserve separate central-owner capacity: at most 4,096 retained operations and
64 MiB reserved logical bytes per actor. Count all successful and retained-refusal
operations; bound canonical request/receipt/policy bytes and charge finite row/
index overhead conservatively. Replays and reads work at capacity; new effects
refuse before insertion. No counter decreases, row deletion, reset, compaction or
physical disk-size guarantee is implied. The future execution owner additionally
needs a finite bound on bound-turn attempts per message (proposed maximum eight).
That is not eight automatic retries and is not yet an executable capability.

## Four storage responsibilities

1. `central_supervisor_actors` binds one immutable actor UUID to the immutable
   actual global conversation UUID. It retains the actual validated current local
   policy bytes/digest, policy revision and approving human identity provenance,
   monotonic actor revision, optional current reservation, and monotonic owner
   capacity counters. All current Owners can manage that one actor. Actor ID and
   conversation mapping cannot be changed or re-created under another identity.
2. `central_supervisor_reservations` owns a reservation UUID and original reserve
   operation, actor/conversation/policy admission binding, exact selected batch
   reference digest, pinned through-cursor, raw body-byte total and monotonic
   reservation revision. Its only lifecycle is `awaiting_prepared_plan` then
   `cancelled_before_plan`. The first state is not a Run or launch permission.
   Identity/batch/admission bytes are immutable. No execution generation or
   session exists here. At most one awaiting reservation exists per actor, with
   a unique partial index plus the actor's locked current-reservation pointer.
3. `central_supervisor_mail_members` records ordinal and exact existing mailbox
   message UUID/key/cursor/original payload digest for each reservation. It has
   an FK to the actual mailbox message and reservation, unique ordinal/message/
   cursor per reservation, and indexed actor+message and actor+mailbox+cursor
   lookups across all retained reservations. These are immutable associations,
   not bodies, acknowledgements or an independently mutable delivery state.
4. `central_supervisor_operations` owns a stable per-actor operation UUID and
   kind, initiating authenticated human binding, exact canonical command bytes/
   digest and immutable admission result. The namespace is shared across kinds;
   reusing an ID for another command, principal or future dispatch/host role
   conflicts. Reservation UUIDs and operation UUIDs are distinct. Store mutation
   and its immutable receipt in the same transaction. Lookup returns the original
   receipt separately from the current reservation view.

No empty future Run table or nullable fake composition columns are introduced.
Later execution admission gets its own reviewed schema and source owner. New
records reject identity mutation/deletion/truncation; revision/counter transitions
are guarded. Cross-field source references are validated against the actual
mailbox row in the admitting transaction, including original recipient/thread and
human author shape. Preserve existing AQ-11 bytes and behavior. If selective
append-only protection of global mailbox rows is necessary, add it in the new
migration without editing frozen 5/35 or weakening legacy agent semantics.

## Lock order and authority

Every operation derives the human from a trusted caller context; no command field
can supply a principal. Requery and lock the current users row, matching ID, role,
email, issuer and subject. Viewers, changed identity and revoked Owner status
refuse. The registration/configuration command validates actual closed local
policy bytes against the real conversation. Reserve requires its current enabled
policy and exact expected actor/policy revisions. Disabled policy does not block
authorized pure lookup or cancellation of already retained reservations.

Use the common order: current user `FOR SHARE`, immutable global conversation
`FOR SHARE`, actor/policy `FOR UPDATE`, actor operation identity/replay, current
reservation `FOR UPDATE`, mailbox head `FOR SHARE`, exact source rows in cursor
order `FOR SHARE`, member/operation/receipt writes. An initial configuration
serializes creation on the already existing singleton conversation row; it does
not race two different actors into that conversation. All policy/revoke/cancel
paths use the same prefix. Existing human enqueue locks user, conversation,
human-sender operation, then mailbox head; central never acquires them in reverse
or borrows a human sender lock as a machine identity.

Recheck current identity, enabled policy and expected bindings after lock waits,
then perform CAS and receipt insertion in the same transaction. No external
profile, file, host or network operation occurs while locks are held. Use finite
pool/queue, statement and lock deadlines; overload refuses without an unbounded
waiter list. Lost commit response is unknown, not proof of rollback. Identical
command bytes and principal replay the retained result; changed bytes/kind or
principal conflict. Check current authorization before replay. A retained stale-
revision/disabled-policy refusal is `not_reserved`, proves no reservation for that
operation, and cannot become a new admission under a later configuration.

All Owners can inspect shared reservation history and original admission results.
Replay still binds the original initiating principal; a different Owner may
inspect recovery state but cannot silently execute under that user's identity.
Cancellation uses a new stable operation, expected current reservation revision
and original reservation UUID. It clears only that reservation's active hold;
retains its members, original receipt and operation identity; and atomically
updates the actor pointer/revisions. There was never a host/dispatch intent, so
`cancelled_before_plan` is truthful. It is not reusable cancellation semantics for
a started Run. Revoking policy prevents new reservations but does not erase an
old one or fabricate cancellation, stopped writers or a delivery disposition.

## History and the future execution boundary

The first private history query covers all retained reservation references by
message UUID OR same-mailbox cursor and all retained operation UUIDs across
command kinds. Changed aliases/digests or unsupported history fail closed. It
returns explicit `execution_history: unavailable`, `history_scope:
reservation_only`; it does not construct `MailHistoryFence` with generation zero
or claim dispatch reconciliation. Any unexpected future operation kind/source
must refuse admission rather than be treated as unseen. State-format startup
fencing prevents an older reservation-only writer from opening a later execution
schema. Missing source rows or query coverage are unavailable, not no-history.

The future real prepared-turn owner must atomically consume a still-current
reservation, resolve actual policy/profile/selection/HostPlan ownership and exact
startup bytes, create the next real Central identity/generation, reserve fresh
cross-role dispatch+host operation UUIDs, and bind an immutable CentralPlan before
committing any physical intent. Cancelled pre-plan reservations consume no Run
generation. Actual rendering must re-read the same immutable mail rows and check
original digests and final rendered-context budget; the raw body budget is not a
promise that every renderer/profile has room. Startup is one immutable bounded
turn, with no native resume, interactive injection or mailbox redelivery claim.

Then, and only then, its private authenticated all-history query must join every
retained real Run/dispatch association by candidate UUID OR cursor, preserve all
original refs and reconcile original NotHandedOff receipts. Any old pending,
unknown or handed-off association holds that input, including generation one
across a disjoint generation two. Both new operation UUIDs must be checked across
all actor roles and generations. Build every explicit core fence entry under the
same actor lock; snapshot digest binds actor/policy/conversation revisions and
source query coverage; successor validation and CAS occur in that transaction.
Caller-supplied source refs, dispositions, `stopped=true` or schema-valid Run bytes
are not authenticated facts. No GC may erase prior holds or operation namespace.

The mapper seam is the pure `aq-codex-mapper` package extracted at worker
`f0b6b76`: Settings borrows shared HarnessSettings/ModelSettings plus an explicit
terminal-input flag; Paths yields a plain Recipe. It imports no actor, offer,
permit, material or provider payload. Future central assembly supplies real
resolved settings and artifact/version/environment pins. Worker remote wrappers
retain their own exact joins. The daemon must not depend on the whole worker,
copy its profile/adapter logic, weaken LaunchManifest, or hand supervisor
credentials to a node. Actual central assembly, host/permit owner, verified
physical facts, stop uncertainty and replies remain later reviewed increments.

## Qualification and implementation boundary

Freeze this document and [internal API](../../protocol/central-reservation/v1/README.md)
first, obtain independent review, then import accepted Media 36 and author only
source migration 37 plus dormant owner and focused tests. Root owns coordinated
state-format registry/bootstrap integration and any later disposable application.
Never apply the migration to canonical state in this lane. No HTTP mounts,
capability advertisement, startup driver or API-07 schema/runtime field changes.

Use disposable actual PostgreSQL/mailbox rows. Cover full Owner/Viewer/identity
and policy changes during lock waits; real immutable global mapping; real ordered
Unicode/body-byte bounded selection and partial scans; exact metadata/no copied
bodies; concurrent reservation CAS; response-loss replay and changed bytes/kind/
principal; retained refusal; pure lookup; cancellation/disable/replay/restart;
all-reservation UUID+cursor aliases/cross-kind operation reuse; capacity refusal
with reads/replay retained; guard/upgrade/drift/old-reader refusal; and unchanged
existing AQ-11 agent digest/receipt behavior. Future execution history must remain
explicitly unavailable. These tests prove durable pre-plan ownership, not a live
supervisor, a stopped process, delivery, a reply or provider isolation.
