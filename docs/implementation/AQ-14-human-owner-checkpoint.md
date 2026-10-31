# AQ14 human owner candidate (unregistered026)

The candidate implements an actual durable central human decision owner over
validated B1 records, actual source-transfer owner rows and a fresh configured
public Git retention inspection. It does not implement B2 eligibility, a task
completion mutation, an executed check, or a playbook decision adapter. Migration
026 remains unregistered pending native025; component tests explicitly install
026 tables while preserving state24. No migration-gap or startup repair exists.

`reviews::Service::prepare` loads the recorded result/current report head and
actual retained import acknowledgement under common ownership locks, commits the
read transaction, and then performs one bounded public `retention.inspect` over
the configured local socket. `admit` reloads the exact owners in a new transaction
and rechecks observation identity/expiry after lock waits. PreparedReview and the
retention observation are non-deserializable process-local wrappers. Replaying
stored JSON cannot create either permit. No Git I/O runs under authority locks.

The historical `RecordedSourceConfirmation` proves an actual retained public-Git
import acknowledgement; it does not prove current physical availability. The
separate current observation requires the exact active transfer handle, repository
binding, one exact ref/OID, no release operation/reason and a positive generation.
It expires30 seconds after read start. No remote fetch or implicit retry occurs.
Current worker source transfer supports SHA1 only; null OIDs are refused here even
though the frozen review wire permits their shape. A later release is not
prevented by a distributed lock; B2 must recheck retention/policy/effects/holds.

The first human question pins the canonical subject, exact original task CAS and
content revision, result/spec/outcome/source owners, current eligibility project
revision, explicit current owner-user set, fixed review policy and deadline.
Only internal callers can prepare/admit; no public endpoint accepts a purported
verified subject. Answers derive actor identity from RequireOwner and recheck the
current users row after waits. HumanAnswer, Expired and Cancelled receipts are
separate. A superseded result cannot receive a new approval; exact historical
receipt replay remains readable without a new vote/current execution authority.

All mutations take the global retained budget lock before common node/attempt
locks, source/head rows, immutable subject and decision rows, then current user/
project rows. Derived lineage counters are updated only while holding that same
global guard; they are not an independent lock entry point. This deliberately
serializes initial owner mutations. Statement and lock waits are bounded to2s
and500ms. Creation, resolution, cancellation and expiry share that order;
preliminary immutable ID discovery takes no decision row lock. Immutable final
receipts can be read before role checks without taking new mutable authority.

Admission request/subject and final receipt/outbox payloads use configured
protected_storage keys with domain/identity AAD. One SQL transaction commits the
human finality, pending→final CAS and completion outbox. A failure of any one
rolls back all three. Stable authenticated operation IDs reject changed payloads.
Exact replay is checked before capacity; no replayable key is pruned. Limits are
16 questions/subject,256 retained operations/original task,10000 globally and
64MiB logical reserved payload. Two full final envelopes plus overhead are
reserved on admission. Conservative reservation may refuse well before the
operation-count ceiling; PG/WAL/disk capacity is separate. No GC is implemented.
Only the comment digest is retained in the receipt; raw comment text is not a
stored notification or a second authority record.

The optional `build_admin_router_with_policy_owners` injection exposes owner GET
inspection and POST answer/cancel under existing identity/CSRF/maintenance gates.
Normal builders pass None. No production configuration/startup enables it yet.
There is no GET mutation, posted passed:true check, posted owner identity, public
expiry flag, or provider/Discord call. Maintenance retains the existing policy;
this candidate introduces no new general maintenance mutation exemption.

Qualification is component evidence: eleven actual disposable-PG result tests
include one expanded synthetic known-native/source scenario covering a local
public-Git protocol peer, replay/conflicts, competing answers, role revocation and
expiry after observed DB lock waits, cancellation, outbox-write rollback, optional
admin auth/CSRF/unknown-field handling and concurrent correction/answer replay.
These are not actual selected native CLI/public-Git retention qualification.
Two pure current-retention tests cover closed exact active-handle/ref checks and
changed owner provenance. Strict lint/fmt and frozen source evidence are recorded
separately after the candidate freeze.

Remaining AQ14 work: profile2/new-generation decision.request adapter, durable
completion ACK/reconciliation integration, check.request with an actual registered
executor, actual native-result/current-Git end-to-end qualification, richer bounded
late-attempt audit, API fixtures, and ordered025/026 integration. AQ14 remains in
progress. Existing profile1 catalog and frozen public contract bytes are unchanged.
