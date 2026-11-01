# Phase-aware fresh execution confirmation and original authority

Status: approved bounded design; implementation starts from ordered30. Migration
032 is reserved for this lane but is not registered until its actual owner and
upgrade tests are qualified. Admission0.1.9 stays byte-frozen. No legacy plan,
initial delivery or receipt row is fabricated to satisfy existing consumers.

## First supported profile and activation boundary

Additive031 belongs to the inventory remove/restore barrier and is a prerequisite.
Frozen029/030 bytes are not changed by this owner.

The first owner supports one exact already-accepted worker choice, no previously
charged/issued execution in that task lineage, and no configured cost/hard-cost/
complete-accounting requirement. A current locked no-prior-authority predicate
can establish elapsed execution0; absent ledger/provider data never establishes
cost0 or accounting completeness. Prior charged/unknown lineages and configured
cost requirements refuse pending a real current accounting adapter. Valid central
NeverGranted history may remain excluded only through its existing owning proof.

This slice adds an actual internally callable Grant owner and actual PG tests,
with **no production execution route mounted**. A distinct protected execution
qualification must name the exact phase/runtime/owner adapters; DTO or configuration
presence alone never supplies their facts. Existing no-launch policy remains
insufficient to activate execution. Worker phase inputs, launcher, Started,
continuation and closure adoption remain separate gates. No provider is called.

## Nine concrete fact adapters

Every adapter is a fixed local owner call over the original sealed binding and
choice, not a generic fact endpoint or caller-provided boolean. Facts retain the
actual underlying receipt/digest/revision and source expiry in bounded internal
provenance. A new evaluation timestamp never extends the underlying evidence.

| Kind | Actual owner and required association |
| --- | --- |
| Project | `project_bindings::check_execution_eligibility`, then `fence_for_offer` over its genuinely fresh frozen result; exact original project revision/catalog/permissions/source binding. |
| Registry | `intelligence::fence_current_registry` plus exact configured credential profile/revision/provider/account/endpoint/node authorization and actual private fingerprint. The original registry artifact pin must still match; no reselection or unrequested registry-pin replacement. |
| Principal | Actual immutable decision principal plus current owner/project-policy authorization under existing principal locks; exact Graph access profile/context binding and current revocation state. |
| Inventory | Actual locked normalized snapshot plus `presence::current_inventory_locked` returning verified InventoryV2 request/receipt/epoch/qualification. Full harness/CLI/adapter/environment/model/reasoning/capabilities/limits must still support the exact choice. Global semantics remain ProvisionableOnly, not an installation claim; legacy false cannot acquire a v2 meaning. Match the sealed original snapshot through `inventory_epoch_locked` to the fresh current compatibility epoch, allowing compatible heartbeats but refusing a remove/restore or legacy barrier. |
| Availability | Existing scoped health owner over actual latest retained observations. Require ObservedAvailable with finite original expiry/age, no overriding negative/cooldown. Unknown refuses initial execution; no provider availability is inferred from an enabled credential. |
| Budget | Newly locked actual task-lineage attempt/NoIssue history and current policy. First profile requires only the current ungranted claim counted as charged and no previous issued/unknown execution. No copied/re-dated initial BudgetSnapshot or invented invoice estimate. |
| Context | Genuinely new bounded ready page and full context read for the exact original CAS/content revision. Compare the original full raw-context digest and authorized semantic context, removing only the documented observation timestamp from temporary comparisons. No read-new/overwrite-old or root-only continuation shortcut. |
| Git | Existing configured repository identity read plus `inspect_retained_for_execution` on the same linked export. Exact original repository/generation/object/base/policy/manifest/bundle; active actual retention, no new ref resolution or retimed SourceAdmission. |
| PrivateMaterialBacking | Existing protected private-file owner re-reads actual backing bytes outside SQL. Exact actual admitted MaterialLease and authenticated qualified-worker Prepare MaterialReceipt/private binding; retain that receipt in the Grant transaction. Its installation assertion is not a remote attestation or a pre-existing same-live-grant PresenceReport. |

Source timestamps remain historical in their own records. Fact expiry is the
minimum of original source evidence expiry/age, actual read budget and the phase
policy cap. Public FreshConfirmation is only the bounded serialization of facts
verified by these owners; public correlation validation authenticates nothing.
No new model/account/endpoint or resource request is accepted during this step.

## Transaction and clock boundary

1. Validate exact phase Prepare and complete original inline plus linked source
   receipt set against actual retained030 proposal/Accept/accepted receipt. Verify
   current authenticated node/boot, runtime, cancellation/closure and no-launch
   ownership. Capture immutable pinned inputs in a bounded read transaction.
2. Release authority locks. Perform fresh Graph/project/Git/private-file reads
   through existing bounded configured adapters. At most two operations run, with
   owned blocking permits retained across waiter cancellation. No remote mutation
   or provider call occurs during confirmation.
3. Reacquire canonical node → attempt/hot → project/catalog → registry/inventory/
   principal → shared retained quota suffix locks. Reread exact pins and current
   heads, and sample expiry again after every wait. Changed or expired facts
   refuse; never renew observations just to fit a delayed commit.
4. Atomically retain the exact worker installation receipt, Prepared journal,
   phase execution request/receipt/provenance and hot grant/hard deadline. A
   transaction failure leaves no partial grant or fabricated installation.

The issuance instant must be inside the accepted phase. Execution lease duration
and hard deadline are independently bounded by the current exact policy and known
lineage remaining budget; neither is capped by expired proposal history or the
old preparation delivery. Facts need be fresh at issuance; their source lifetimes
are not an automatic execution renewal. Graph/Git observations are sampled facts,
not a distributed transaction that freezes external services forever.

## Immutable OriginalAuthorityHistory

Add one typed read-only owner with private constructors:

- Legacy: actual original `kind='prepare'` request digest, StoredInitialGrant/
  GrantedLaunch bytes, versions, issuance/hard deadline and actual retained
  installation. Existing legacy behavior and bytes stay unchanged.
- PhaseV2: actual030 proposal/request/accepted receipt and their digests, exact
  phase Prepare, ExecutionReceipt/FreshConfirmation, actual runtime-registration
  history, installed-material receipt and the retained underlying fact provenance.

A normalized view exposes exact binding/grant/owner IDs, issuance/hard deadline,
materials and original receipt kind/digest. It never manufactures PreparationPlan,
GrantedLaunch or InitialGrantDelivery for the phase variant. Missing, mixed,
ambiguous, changed or structurally inconsistent indexed/body history refuses.
Historical reads carry no new nonce, fresh permission or renewed clock.

032 stores immutable canonical request/receipt/provenance bytes with finite caps,
node/attempt/operation/grant identities, exact digests and issuance/expiry columns.
It references the actual030 acceptance and actual registration history; no fake
legacy receipt satisfies a foreign key. Its retained bytes and material-receipt
bytes join the existing global/node quota transaction. No uncertainty is pruned.

Exact replay reads the retained original execution receipt. Current active scope
and immutable grant identity/hard cap are rechecked; a fresh nonce can only deliver
the same still-live authority. No new grant ID, new facts in the old receipt,
renewed execution expiry or copied budget is created. Historical lookup remains
separate from live delivery; expired/revoked authority never revives. Lost Grant
responses remain ever-issued/unknown and cannot enter NeverGranted cleanup.

## Consumers explicitly not enabled by this slice

`started::record` currently trusts hot grant plus Prepared journal without loading
a legacy original receipt, so add an explicit phase-origin refusal until it is
adapted to the typed history. Renewal, closure and legacy reconciliation currently
require legacy original rows/plans and remain refusing for the phase origin;
negative tests make that deliberate. Physical stop/cancellation must not be
misrepresented as a successful task or new execution permission. Before mounting
an execution route, Started/renewal/closure/reconciliation and actual worker
journal/input/host consumers must qualify this origin end to end.

Unknown accepted-response recovery is also separate: no opaque received Accepted
after a lost reply means current worker transport conservatively holds after
proposal expiry. Nothing here erases that held disposition or invents its receipt.

## Qualification

Use actual disposable PostgreSQL and existing synthetic local owner peers, then
pinned actual Git where relevant. Prove all nine owners, unknown/negative/stale/
changed-choice refusals, exact set/private binding, delayed quota/authority expiry,
rollback after receipt insertion, CAS/context changes, actual retirement, immutable
replay/unknown response, corrupt/missing/ambiguous history, legacy compatibility and
explicit unsupported-consumer refusals. Cost/previous-issued profiles must refuse,
not pass via placeholders. No production activation, provider/host run, full cost
accounting, PB04 latency or broad cold-workload closure follows from these tests.
