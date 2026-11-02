# Refresh issuer source qualification

This increment joins actual authenticated SQL preparation owners to the nominal
aq-admission 0.1.10 refresh issuer. It adds actual four-registry issue provenance and
monotonic, current-owner revalidation of original issued-grant replay. The public
refresh execution route remains closed until the actual refresh terminal owner
is installed. No provider, model, physical host or worker binary ran here.

The shared implementation retains the original issuer's initial authority,
private backing, Graph/project/Git association, inventory compatibility,
principal, policy, installation, budget, quota, atomic journal and clock checks.
The exact old and new contract selects a nominal parser and owner; there is no
DTO conversion. Initial requests share one four-request permit lane and durable
quotas across both profiles. Old issuance/replay facts remain unchanged.

New replay releases its SQL transaction before external observations. Its owned
candidate contains immutable issued history and an actual preflight floor, not a
new grant. A final actual locked read fences original issue/preflight rollback,
current health/cooldown/source age, inventory, credentials, material, node,
principal, policy and project. After the read-only commit, captured source age
and the unchanged grant/deadline/retention bounds are checked once more before
constructing the response clock. Original request, receipt and provenance bytes
are never replaced or extended.

## Meaningful negative history

The first shared-future build typechecked but overflowed the default test-thread
stack. Boxing only the nested replay future was insufficient. Boxing the public
owner/loaders and test fixture construction fixed initial issuance but revealed
the successful replay path still retained the initial owner's large active poll
frame. Static binary frame metadata showed roughly 800 KiB for that frame, plus
roughly 315 KiB replay and nested current-owner frames. The final implementation
returns a private `Issued | ReplayCandidate` outcome before invoking replay, so
that active initial-owner frame has returned. The same capacity permit covers
both stages. No `RUST_MIN_STACK`, thread-stack increase or relaxed policy/timer
is used.

An early post-fix batch passed 8/9 new cases in 31.76s. The remaining test refused
during initial issue setup with the existing fixed fresh-owner-observation
`UpdatesUnavailable` class; no incorrect replay was accepted. The exact
underlying unavailable sub-owner was not captured. Its isolated stale/expired
branches passed 4.86s with unchanged limits. This initial setup refusal remains
unresolved evidence rather than an invented expiry or transport explanation.
Expected sibling-observation cancellation may close the synthetic Git UDS before
its reply; that specific BrokenPipe/ConnectionReset now ends the test peer task
without a spurious detached-task panic. Other peer errors remain assertions.

## Checks

The pre-delivery-commit tightening source passed all 23 focused actual-owner cases
with default test stacks and actual disposable SQL stores: the 14 frozen issuer
regressions passed 34.18s, and 9 refresh cases passed 34.62s. The new cases cover:

- genuine newer positive assessment before issue and actual four registry refs;
- unrelated positive advance preserving exact original request/receipt/provenance;
- positive advance, newer negative, and a deliberate same-config registry
  rollback during a real unlocked external observation;
- newer negative health and active cooldown;
- actual protected config installation and credential revision rotation;
- changed actual private backing and retired actual project principal policy;
- stale health and an expired original grant;
- still-live original execution after preparation/private retrieval expiry,
  without extending or rewriting the expired preparation/material lease.

The final post-commit delivery-clock guard is qualified in the following source
checkpoint's check record. All original declared timers, freshness policies,
Git limits, wire families and SQL schemas are preserved. The new route's closure
is deliberate: source owner tests do not qualify terminal ACKs, runtime
eligibility or actual end-to-end output delivery.

## Reliable owner fixture boundary

The final new fixture performs all expensive component/configuration setup before
its genuine new Poll/resolver decision. Immediately after that actual offer
returns, it verifies the retained decision is still inside its original expiry
and captures one trusted owner-test clock. It does not change any decision,
offer, observation, source timestamp or two-second bound. The actual acceptance
and subsequent owner operations use that same clock. The logical Prepared
boundary explicitly advances it by 100 ms after the actual private fetch/receipt;
negative tests explicitly advance it to the original expiry/age boundary.

The refresh Graph adapter now has a trusted current-ready callback parallel to
its existing current-context callback. Both wrap genuinely new network requests
and body receipt. They sample AQ's own observation clock only; upstream body,
CAS, creation/update times, original retention/expiry fields and raw response
hashes remain unchanged. Project, Git-retention, private-read, health, inventory
and SQL observations therefore use the same owner clock in these tests. Real
Tokio I/O deadlines are unchanged. The frozen adapter/fixtures retain their old
real-clock behavior, and actual static process qualification must use real clocks.

This fixes the positive owner-test assembly boundary instead of allowing stale
selection or extending TTLs. Before this correction, the post-commit guard batch
passed 8/9 in 32.76s; the affected three-mode case refused during actual acceptance
with the exact class `selection expired during admission locks`. Its isolated
case passed 7.84s, but no automatic retries were installed. A later real-clock
frozen baseline batch passed 13/14 with one setup Poll 409 before proposal; that
Poll's precise refusal class was not captured and is not relabeled as the known
new fixture's acceptance expiry. The earlier frozen 14/14 batch remains recorded.

With the corrected consistent-clock fixture, the final refresh suite passed 9/9
in 33.90s (2.39s test rebuild), including all three genuine external-read race
modes and original-grant replay after private retrieval expiry. There were no
agent/host processes, provider calls, database migrations, schema restamps,
canonical state changes, or public refresh runtime activation.

Final checks also passed strict Clippy for the library plus both selected issuer
test targets (all features, warnings denied, 21.13s), the default library check
(15.62s), and formatting/diff checks. The single frozen Graph refusal regression
that encountered the earlier setup Poll 409 passed in isolation 6.73s with its
original real clock and limits. Its earlier negative result remains evidence;
it was not silently converted into a successful batch. Strict duplicate-module
lint exceptions are confined to the four intentional nominal owner modules:
aliasing one module would cross the distinct protocol-type boundary instead of
sharing the actual algorithm. No general warning suppression was added.
