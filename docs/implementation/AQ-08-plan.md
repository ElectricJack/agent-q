# AQ-08 staged delivery plan

Status: stage 1 implementation authorized; storage/lifecycle stages require root
review. This plan implements real graph candidate observation, explicit placement
and sealed profiled authority. Readiness is never an execution claim. No existing
session 0.1.0 publication, legacy execution bytes/digests or graph-access profile
schema is expanded in place.

## Stage 1: independent contracts and bounded graph observations

Add `aq-admission` 0.1.0, a pure package depending only on the pinned public
session contract and ordinary serialization/schema libraries. Publish packaged
schemas, fixtures and drift tests for exact graph task revision, source selection
policy, retained artifact descriptors, private provider-material leases/receipts
and bounded placement snapshots. The initial retained-artifact helper caps 128 total descriptors, including
definitions, and may refuse an otherwise public-envelope-valid maximum-context
manifest. This is a declared smaller bounded profile, not full maximum-envelope
support. The types convey no authorization or retention
proof; runtime adapters must authenticate and verify actual owner records.

Placement uses bounded candidate/node snapshots, deterministic priority ordering,
project round-robin fairness and warm/cold locality hints. Resource-fit filtering
precedes truncation; an oversized earlier task cannot hide a later fit. Cold
eligible nodes may prepare. Availability/cache hints cannot grant credentials,
source access, model eligibility or execution. Returned reservations are proposals,
not durable capacity claims; actual admission and worker local checks remain.

Add `agent_access/scheduling.rs`, using the existing server-owned configured graph
client and pinned API1 header. Expose narrow fixed `/ready` and root `/context`
readers with UUIDs, bounded limits/depth/bytes and opaque bounded cursors only; no
caller URL/token. Validate retained upstream shapes, exact workspace/root identity,
positive CAS/content revisions, item counts and truncation. Pages are not a
whole-workspace snapshot. Preserve partial/truncated/unknown coverage and refuse
it where required startup context or readiness proof needs completeness. There
are no new installed scheduler/lease/lifecycle routes in this stage.

## Required source, artifact and provider-material ownership

A source-admission policy is operator configured and revisioned per project. It
names the public repository ID/binding generation and either an immutable commit
or an explicit logical ref with expected policy revision. Public Git resolution
returns the exact commit and original observation time. An omitted/unresolvable
policy refuses; no default branch or local path is guessed. Actual immutable
source/outcome/context/composition/selection artifacts must be retained before
offer publication. A descriptor/digest alone is not retention. Public retained
Git operations supply actual source evidence; no full Git scan is required.

A minimal local provider-material owner reads a configured private regular file
with no-follow/nonblocking bounded checks outside database lock waits. It verifies
current credential revision, configured provider/account/endpoint and private
fingerprint, then issues an immutable expiring lease for the exact shared
LaunchBinding/remote node/boot/attempt/session. The lease also binds credential
profile/revision, account/endpoint/purpose, byte length and material digest.
Material bytes travel only through the authenticated authorized node's private
binary delivery path, never manifests/prompts/logs or database artifact bodies.
No external secret service, new tunnel or worker-local credential mode is assumed.
A presence report never establishes provider account identity. The current owner
config's account mapping and independently qualified adapter govern that binding.

The worker validates delivered bytes against the exact lease and installs an
owner-only attempt-private credential binding; its bounded receipt identifies the
binding artifact, not a physical path. Receipt transport acknowledgement is not
launch authority. Expiry/rotation/revocation prevents new grants; private material
cleanup and no cross-attempt reuse are worker adapter obligations to qualify.
Source/material preparation may happen before grant; execution may not.

## Stage 2: common authority storage bridge (review before edits)

Retain the `execution_attempts` common identity columns and existing unique
unsettled-task index, preserving one claim across legacy and profiled kinds. Add
an explicit `kind` discriminator with a legacy default and a constrained actual
profiled state representation. Do not encode a real agent as a fake fixture
ExecutionSpec merely to satisfy existing foreign keys. Existing rows and legacy
wire/digest algorithms retain their bytes.

All legacy state readers must select/validate legacy kind before decoding
`AttemptState`; enumerate poll, registration/recovery inventory, accept/renew,
result/stop, cancellation, source transport, agent authentication and inspection
queries in review. Actual profiled state has its own decoder and state machine.
The existing identity/FK rows remain common so graph access, source transfers and
`intelligence_attempt_selections` bind genuine profiled attempts without invented
legacy state. Shared unique claim and serializable retry predicates prevent
legacy/profiled double admission for the same task.

Introduce a narrow common `AuthorityView` derived from a locked kind-specific
record: exact node/boot/task/attempt/session/generation; offered/granted/cancelling/
finished status; grant ID and expiry; current authority revision; exact execution,
manifest, graph-access-profile/context and source binding; conservative stopped,
retention and unresolved-effect disposition. It exposes no writable graph state
or inferred stop proof. AQ07 authentication/effect fences and source retention
consume this view; each kind's real state owner retains transitions. Legacy
validation remains unchanged and gets compatibility regressions.

Proposed migration 0010 adds kind/profiled projections, immutable manifests and
retained artifact references, admission operation receipts, provider-material
lease records/verifiers, source admission observations and bounded scheduler
cursor/reservation metadata. No task lifecycle projection becomes a second owner;
graph remains the task/context authority. Root must review exact schema, lock
order and every affected decoder before this migration is written/registered.

## Stage 3: actual profiled transport and atomic admission

Reuse existing `ProfiledOffer`, `ProfiledAcceptance`, `ProfiledGrant`,
`LaunchBinding` and `SessionIdentity` exactly. Publish any missing AQ-owned
poll/prepare/material/artifact/renew/result/stop/reconcile envelopes separately;
do not change frozen session 0.1.0 or fixture protocol bytes. A concrete execution
spec artifact bridges exact public source/outcome/context/access-profile and
selected model/harness/resource policy to LaunchBinding.execution_spec_digest;
its canonicalization is separately pinned, not the legacy fixture digest.

The flow is graph observation -> public source/context preparation -> trusted
AQ18 composition/AQ20 resolution -> prospective exact assignment/session IDs ->
retained artifacts and material lease -> SERIALIZABLE offer transaction. The
transaction rechecks task revision/readiness observation freshness, node/project/
catalog/selection, source policy/resolution/retention, material lease revision and
expiry, current shared task claim and retry/budget predicates. It persists the
sealed choice, composition/manifest/artifact bindings, material authority and
actual offer together. Lock order must extend existing node -> project -> catalog
-> intelligence registry order consistently. No external calls/file reads occur
under these locks. Final time checks run after waits and immediately before commit.

Worker acceptance independently checks actual compatible qualified CLI/host,
resource/source/context/material readiness and exact public digests. The daemon
refetches current graph task/readiness and refuses changed/expired inputs before
grant. A grant never silently changes candidate or configuration. The native host
still enforces independent ownership/expiry and local admission. Qualified failure
classification must be sent while the exact attempt is granted/unexpired, before
its final result report.

## Stage 4: invalidation, fairness and qualification

The public graph contract advances owner node CAS/content revisions for lifecycle
and owned relationship changes. Retain those revisions and required dependency/
context observations. Reobserve before grant and settlement; AQ07 graph effects
use current guarded CAS. Remote readiness reads are not a distributed reservation:
if graph APIs cannot atomically fence a dependency-sensitive final effect, refuse
that automatic settlement rather than claim an impossible cross-database lock.
No graph task transition is inferred from process exit or a satisfied wait.

Multiple projects share bounded per-tick work and capacity through durable
round-robin cursor/explicit priority, with warm source hints as a tie preference.
Offer reservations expire but do not establish stop or release writer resources.
First attempts only until real stop, source retention and unresolved-effect
reconciliation support replacement. Missing accounting remains unavailable;
complete/hard-cost policies refuse until the actual ledger/enforcer qualifies.
Central supervisors require a distinct authority path and never fake a worker ID.

Qualification progresses from pure fixtures to fake HTTP/SQL races, then pinned
actual Git/Graph ready/context/source components. Tests cover paging/truncation,
wrong workspace/root/revisions, graph changes between observe/admit/grant,
source-ref changes, material mutation/rotation/expiry and unauthorized fetch,
artifact mismatch, duplicate/conflicting receipts, stale boots, mixed-kind task
claim races, lock-wait expiry, oversized head-of-line work, multi-project fairness
and cold preparation. Actual native CLI/provider-material/host execution remains
an explicit WRK08/product integration gate; synthetic qualification artifacts are
never presented as real native compatibility evidence.
