# AQ-14: exact human review subjects and executed check ownership

This slice follows the first AQ13 bridge; it does not implement the B2 result
eligibility evaluator or graph/task settlement. Migration026 is reserved here;
025 belongs to the native offer/source-observation producer. The existing public
outcome0.1.5/result-record0.1.6 contracts and playbook catalog profile1 stay byte
fixed. New review/check contracts use a separate versioned namespace and source
publication. No model/provider, worker credential, Discord transport or deployment
is required to implement the first human decision owner.

## Immutable subject, not a result eligibility claim

A review subject pins these exact facts:

- project/workspace and original task ID, graph task CAS version and independently
  retained content_revision; original admitted project revision;
- full LaunchBinding (session/attempt/node/boot/generation and spec/manifest/access
  digests), plus the retained typed spec artifact;
- original ExpectedOutcomeDefinition artifact, task observation/context artifacts,
  required criterion/check identities and validation_policy artifact;
- report ID, exact report body digest and actual0.1.6 result_record Receipt artifact,
  including the exact stopped-owner receipt;
- actual retained resulting source: repository/binding generation, base and
  resulting object IDs, resulting source artifact and resolved source-owner receipt.

A report's source_receipt_ids or pending_effect_ids are claims, not verified
source. Build the authoritative subject only after resolving actual retained
report/receipt/outcome bodies through the B1 owner and resolving resulting source
through the actual source owner. Do not read private graph/Git databases. Initial
creation may be a trusted internal typed interface; public creation cannot post
verified:true, source_confirmed:true, actor identity, or an eligibility boolean.
If actual source resolution is unavailable, hold/refuse authoritative review
creation rather than inventing a confirmed object.

The subject's canonical ArtifactRef is content addressed and immutable. Shape/
digest validation in a pure public contract grants no authority: the backend
must resolve the exact owner row/bytes. A non-deserializable VerifiedSubject
wrapper is constructed only by the actual owner-resolution adapter. Its stored
representation is durable provenance, not a reusable fresh HTTP permit. B2 later
loads review/check receipts from these owners and compares exact current pins;
a successful report recording, human answer or check alone is not eligibility.

Required B1 seam: results::load_locked(tx, report_id) returns validated retained
report,0.1.6 receipt, typed spec, outcome bytes/artifact and current-head fact under
its ownership locks. It must not evaluate completion policy. The source resolver
returns actual confirmed source/object/receipt metadata under its own public
retention interface. An unresolved seam remains a named integration gate.

## Human decision ownership and revision rules

First decision kind is review_exact_source, with fixed approve/reject choices.
Creation binds an immutable subject, current project-policy revision snapshot,
explicit eligible owner-user IDs (1–16), deadline and stable operation ID. Eligible
users are derived/checked through current users/project ownership; caller labels
cannot assert an owner role. The initial profile requires current Role::Owner,
matching outcome HumanOwner. PolicyActor/model assessments are a separate future
receipt kind, never coerced into a human approval.

Public resolve accepts only decision ID, expected pending decision version,
operation ID, choice and bounded optional comment. Actor identity comes from
current authenticated RequireOwner, never JSON, node/session credentials or a
message sender field. Lock decision then eligible/current user/project-policy
rows consistently, recheck expiry and current eligible owner after waits, and
CAS pending version1 to a final version2. A policy-revision/eligibility change
requires a new deliberate decision admission; do not silently restamp an old
answer. If a current result-head check is available under the B1 owner's lock,
refuse a superseded subject at resolution. External graph changes still require
B2's eventual expected-version check; no distributed atomicity is claimed.

The answer receipt binds decision/subject versions, exact subject artifact,
authenticated owner ID, eligibility/project-policy revision, choice, comment
digest and service timestamp. It is immutable. Two owners resolving version1
race on one row: one final answer wins; the other gets a version conflict. Same
operation ID with changed decision/choice/comment conflicts. Exact replay returns
the original retained receipt, without a new vote or fresh authority.

Request acceptance, human answer, expiry, cancellation, stale/late attempt and
transport delivery are distinct. Final receipt variants are tagged HumanAnswer,
Expired and Cancelled; only HumanAnswer can carry a real authenticated owner.
Cancellation before answer CASes the same decision and wins or is too_late;
late answers are auditable refused/late dispositions and cannot revive approval.
No approve URL GET mutates state. Notifications carry only an authenticated AQ
link; a Discord delivery/acknowledgement is not a decision. Automated playbooks
can request a human review but have no decision.resolve host command or human
Cloudflare/session credentials.

## Registered executed checks

check.request selects an exact check ID + validator ArtifactRef already required
by the pinned outcome, an exact subject and a stable operation ID. No arbitrary
shell/URL/path/tool name and no passed:true result endpoint exist. A durable check
intent precedes execution; acceptance is not completion. The check owner owns
executor registration, original input pins, bounded attempt/stop state and final
immutable evidence. Unknown/lost responses reconcile the original operation;
never start a second possibly live writer or invent a fresh retry ID.

A completed check receipt binds subject/outcome/policy, required check ID and
validator digest, actual executor/definition version, exact checked input refs,
actual result artifacts and explicit stop/exit evidence where a process ran.
Passed, Failed, Cancelled, TimedOut, Unknown and Stale are different dispositions.
Missing input or uncertain stop cannot produce Passed. An LLM assessment or
worker/model output naming a validator is not that executor's receipt.

The first independently implementable executor may be a named in-process
source-artifact-integrity validator over actual owner-resolved immutable bytes.
Its configured validator artifact fixes the algorithm and purpose; it actually
hashes those bytes and checks the required retained source/object binding. It
cannot satisfy a different unit-test/build validator ID or arbitrary acceptance
criterion. No subprocess means no invented process-stop proof. If actual source
bytes/owner proof are unavailable, it stays unavailable. Generic code checks and
judgment.request wait for a qualified configured executor/central harness and
remain explicit integration gates, not fake adapters.

## Storage026 and bounded retained history

| Owner table | Immutable identity and write boundary |
| --- | --- |
| review_subjects | canonical subject artifact + resolved owner references, protected subject bytes, original task lineage; immutable |
| human_decision_requests | stable requester/operation key, subject digest, fixed choices, eligible owner IDs, project-policy snapshot, deadline; versioned pending/final state |
| human_decision_answers | immutable authenticated answer/expiry/cancel receipts, exact operation/payload conflict protection |
| human_decision_late_attempts | bounded authenticated stale/late attempts; never usable as approval |
| review_check_intents | stable check operation + exact subject/validator/inputs, bounded execution/recovery state; no caller verdict |
| review_check_receipts | immutable actual executor evidence/result with subject/validator/stop bindings |
| review_owner_outbox | stable producer/epoch/event IDs, protected exact completion payload and engine ACK, committed with owner finality |

Reuse existing protected_storage and actual users/project/result/source owners.
Do not create a second result or graph ledger. Bound requests/receipts to64KiB,
comments to2048 UTF8 bytes, eligible users16, required checks32, evidence refs64.
Initial limits: <=16 decisions/subject, <=256 retained review/check operations
per original task lineage, <=8 explicit executions per exact subject/check,
<=10000 globally retained decisions/checks and64MiB protected owner payloads.
Derive lineage from actual admitted result/task records, not a caller-chosen UUID.
Serialize quota check+insert with retained budget rows. Exact retained replay is
checked before capacity; no uncertain effect/key is pruned to make admission pass.
Physical PG/WAL capacity and GC are separate operational limits.

## Catalog profile2 and durable bridge integration

Keep profile1 registry/schema/fixtures and message.send@1 bytes unchanged. Add an
explicit profile2 selection/new namespace generation or namespace ID, with exact
new catalog digest and capabilities. Never widen an old stored grant/catalog
silently. Preserve older namespace readers and deny unsupported profiles.

First registered addition is decision.request@1, requiring explicit
 decision.request capability and typed project/task scope. It is Reconcilable,
returns Accepted with stable decision-ID handle, and uses a fixed authenticated
completion producer with correlation decision=handle. Completion payload is the
engine-required {outcome,output}, with approved/rejected/expired business outcomes
and exact immutable owner receipt refs. Cancellation is its separate stable
key/state; an accepted request is not human approval. Reconciliation reads only
the retained decision and never creates/reopens one after grant expiry.

Commit decision admission/finality and outbox in the owner transaction; bridge
and engine ACKs remain separate. Exact event replay/fanout retains destination
identities and cannot manufacture another vote/run. Extend bridge command/type
checks only for the new finite profile. Current pending source/result policies,
stop/retention holds and B2 eligibility remain owned by their existing services.

## Staged implementation and qualification

1. Publish pure versioned Subject/Decision/Check DTOs, canonical digests, strict
   bounds, schemas/golden fixtures and negative source/authority/unknown tests.
2. Implement026 human decision backend/HTTP with trusted subject-resolution seam,
   real current owner auth, CAS/idempotency, deadlines, cancellation, quota and
   protected records; fake/synthetic subject fixtures are explicitly component
   evidence until actual result/source resolution is connected.
3. Connect actual B1/source owner lookups; verify current head/exact retained pins.
   Implement independent actual registered source-integrity check only when its
   actual byte/retention inputs are available. No fake native/provider calls.
4. Add explicit profile2 decision adapter/outbox and assembled protected-engine
   human-resolution tests. Keep judgment/generic check/harness and B2 evaluator
   gates open until their actual owning implementations are qualified.

Tests cover changed-ID bytes, stale task/content/source/policy pins, ineligible or
no-longer-owner users, expiry after DB lock wait, two-owner answer races, cancelled
and late answers, response loss/restart and exact receipt/event replay. Check
fixtures prove wrong validator/subject/input/stop evidence cannot pass, actual
validation runs, unknown never becomes passed, and a retained check does not
approve changed source. Root qualifies ordered025/026 integration separately.
