# Internal verified-wait owner38: frozen allocation envelope

Candidate contract before SQL qualification. Root reserves migration0038 for this
INTERNAL owner only. No route, public WaitRegistered, enforced-hold receipt,
scheduler bypass, live Graph registration or canonical migration is introduced.

Each retained wait reserves **1,048,576 logical durable bytes** before acceptance.
The reservation includes all original/auth/policy bytes, mutable work/projection,
and otherwise-empty terminal/cancellation slots. Progress does not borrow another
wait's headroom or reacquire global quota locks. Count reservations never decrease
in this slice; no history deletion/GC/replay-identity recycling is implemented.

| Component | Maximum bytes | Maximum rows |
| --- | ---: | ---: |
| Original request + resolved policy/auth anchors + IDs | 131072 | 1 protected slot |
| Current Snapshot | 262144 | 1 protected slot |
| Member index metadata | 32 ×2048 =65536 | 32 |
| Current source-work envelope | 262144 | 1 protected slot |
| Original internal registration receipt | 16384 | 1 protected slot |
| Actual AQ08 cancellation identity/request + receipt | 16384 | 1 protected slot |
| Immutable terminal outcome | 131072 | 1 protected slot |
| Metadata-only resume intent | 8192 | 1 protected slot |
| Immutable completion receipt | 16384 | 1 protected slot |
| Task gate metadata, conservatively charged per wait | 8192 | 1 shared row |
| Project wait-graph gate metadata, charged per wait | 4096 | 1 shared row |
| Fixed row/protection metadata allocation below | 65536 | 44 rows accounted below |
| Nonconsumable allocation remainder | 61440 | 0 |
| **Total** | **1048576** | **at most78 referenced/charged rows** |

The eight protected slots are pre-created in the admission transaction. Slots
for cancellation/outcome/resume/completion are initially empty, not filled with
invented receipts. Their **172032-byte terminal/cancellation allowance** cannot
be spent on source progress. There is one current work row and no unbounded
per-poll receipt/event journal. A stale work generation is rejected; original
registration, cancellation and terminal identities retain their exact receipts.

Fixed metadata65536 is allocated as: registry row8192; work row8192; eight slot
metadata rows8192 total; two budget metadata rows8192; at most32 known-dependency
edge metadata rows32768. Each metadata bound covers its complete bounded JSON
column projection; each protected-slot metadata allowance also includes its16
AEAD tag bytes. Ciphertext length is exactly plaintext length+16; nonce length12,
key ID bound64, digest format and metadata lengths are checked. No secret or
message body is placed in the member/edge/gate index metadata.

Row bound: one registry +32 members +eight slots +one work +32 edges +one task
gate +one project gate +two budget rows =78. Shared gates/budgets are charged in
full conservatively per wait, and a single fixed installation-budget metadata
row has a separate4096-byte base allowance. Orphan per-project/per-task records
are refused by deferred consistency/startup checks. Logical payload/metadata
accounting is not a hard PostgreSQL page/index/WAL/MVCC physical-disk bound.

Nonterminal Snapshot payloads are limited to **131072**, leaving at least131072
inside the262144 Snapshot slot for terminal outcome and bounded transition
metadata. A proposed source result must satisfy both the nonterminal-basis bound
and its final bound before it can commit. Oversized/unqualified source evidence
enters bounded recovery using the prior valid projection; it cannot consume
terminal/cancellation headroom or produce a partially accepted outcome. The
outcome is separately bounded to131072, and all terminal materialization occurs
in the same transaction from already reserved slots.

Default lifetime reservations:256 waits per exact workspace/project,2048 per
installation. Reserved bytes are exactly envelope count×1048576. The same SQL
hard ceilings permit an operator to lower a disposable test quota without a
production API or a test bypass. Lowering below used capacity refuses. Terminal
and cancellation writes remain possible when admission capacity is full because
they consume existing slots. Count/byte capacity races serialize installation
then project budget before the outer wait gates; ordinary progress uses only its
own already allocated envelope.

## Actual authority and source boundary

The internal owner receives an opaque authenticated AQ08 machine-owner context,
not a caller-selected Binding as authority. Before reserving it loads current
node→attempt/hot state through the existing authority owner, verifies the sealed
original ProfiledOffer, and derives `transport::lifecycle_binding`. It repeats
authentication/current authority under project/task gates before the write.
The supplied CON03 request binding is compared against that derived identity.
This is an internal daemon/machine-owner seam, not a public SessionAgent lifecycle
capability or a node-recovery protocol upgrade.

Source completion uses the same actual owner after locks. A cancellation flag
comes only from the AQ08 cancellation owner state/identity, never a caller bool;
it can retire a resume intent without rewriting an existing outcome. This first
internal increment does not mint detached continuation authority after a live
grant expires. AQ26's later retained-scope/admission integration remains required.

Real Graph source installation is unsupported. Qualified fake source ports are
test-only. The mailbox seam keeps head-before-registry ordering and uses the one
existing mailbox owner/cursor; no second mail/event database is created. External
source IO occurs only after work-claim commit, behind finite nonwaiting physical
admission retained until actual callback completion. Late callbacks cannot apply
after lease/generation/authority changes.

## Schema/qualification gate

All frozen migrations remain byte-identical. The new DDL will declare immutable
identity and original slots, monotonic registry/gate/work transitions, exact child
counts/slot bounds, atomic terminal/outcome/resume/cancel consistency, quota and
original-auth/projection linkage. Startup schema checks pin complete object
definitions/keys/actions/nullability/trigger/procedure attributes; configured
owner startup also decrypts/authenticates retained records and verifies decoded
projection/input/receipt consistency. SQL structural checks alone cannot claim
to authenticate encrypted source evidence.

Before any compiled SQL qualification, Root receives the concrete0038 DDL and
its digest plus this allocation contract. Disposable tests must cover full-quota
terminal/cancel completion, quota races, gate-before-node and mailbox-before-wait
NOWAIT cases, held IO with no SQL locks, exact replay/stale work, current authority
loss/cancellation/deadline, startup definition drift and retained-record tampering.

The first source candidate enables only the daemon-owned clock adapter. The
existing task-mailbox identity/head lock is reused before registry rows, but an
actual mailbox committed-observation verifier is still unsupported; fake source
ports are available only with `test-support`. This increment does not advertise
Graph/mail dependency functionality. The installation's default paths do not
construct the Owner or honor its gate yet.

Protected terminal capacity is checked from each accepted nonterminal basis by
measuring both cancellation and deadline plans at the maximum safe timestamp.
This covers timestamp-size growth and outcome duplication rather than assuming
that two nominal payload bounds necessarily fit after serialization.

Cycle indexing includes a virtual `Wait(wait_id) → Task(owner)` bridge for each
nonterminal record. Without that bridge a task→wait→task cycle could be missed.
The registry's trigger-assigned creation/terminal graph revisions retain this
bridge history; source-edge rows retain their own activation/retirement revisions.
Both are read at the original locked project graph revision when authenticating
an old cycle receipt. These two bounded registry integers fit its existing8192
metadata allowance, and add no rows or bytes to the fixed reservation.
