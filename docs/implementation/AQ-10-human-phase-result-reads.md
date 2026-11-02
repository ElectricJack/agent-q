# Opt-in historical phase result metadata

Base: `cd64196`. This bounded API slice reads existing qualified B1 records;
it creates no report, proof, acceptance, completion, media grant or new SQL owner.
The focused, independently reviewed implementation is integrated in `7fb35a8`
after the normal actual-Codex-to-Graph path passed. Its SQL/router qualification
below remains separate evidence; the actual execution case does not itself
exercise this human read endpoint. Canonical service and dashboard promotion
remain separate.

The existing `GET /api/human/results` list and exact revision route currently
filter/refuse phase origins even though `results::load_locked` verifies actual
Legacy, Frozen and Refresh historical owners. Its public coverage literal is
`recorded_legacy_native_only`. The current dashboard and supplemental media owner
deliberately enforce that boundary. Broadening those pages under the old literal
would be false; changing the default literal would break the existing client.

## Additive profile

Root approved an opt-in query `profile=recorded_native_v2` on the existing list
and exact revision routes. An absent profile preserves current v1 behavior,
coverage, field set and phase refusal. Unknown profiles refuse rather than
silently falling back. The v2 page uses honest coverage `recorded_native_v2` and
the same page/metadata shape, limits, UUID keyset ordering, exact report digest,
Owner authorization, no-store response and bounded work budget.

Opt-in v2 retains the authenticated full `User` and the existing `ActorPin`
identity/revision observation before B1 I/O. It fences that pin after common
Node/attempt locks and before commit/response, holding the user SHARE lock until
commit. This rejects same-UUID identity rebinding and an ABA restoration while
waiting on a common owner. The legacy UUID-only `read` entry remains legacy-only;
new v2 calls require `read_as(full_user)`. Default v1 HTTP keeps its inherited
UUID/role authorization behavior; this slice does not claim a full-principal
race fix for v1.

Root approved this minimal additive v2-only metadata field:

```json
{"origin":"phase_refresh_execution_v1"}
```

`origin` is the closed enum `legacy_prepare_v1`, `phase_execution_v2`, or
`phase_refresh_execution_v1`, derived exclusively from the generic
historical `RecordedResult.origin` after full owner verification. The field is
omitted in default v1 responses. No new origin comes from caller input, launch
labels, current allowlists or a raw result-row string. The existing
`schema_version:1` record structures otherwise remain unchanged; the explicit
read profile/coverage selects the additive interpretation.

Without per-item origin, a mixed v2 list cannot safely drive downstream
origin-specific capabilities. The field enables future adapters to distinguish
those domains while keeping all phase media actions unavailable. The legacy-only
media owner and frozen media contracts are unchanged. A future v2 client must
obtain an actual legacy v1 binding before using existing legacy media APIs; a v2
coverage string is never rewritten as legacy coverage. No UI edits or capability
advertisement are part of this slice.

## Owner and source associations

Discovery selects bounded report IDs joined to their actual launch project.
Default v1 keeps its phase-attempt exclusion. Opt-in v2 discovers all reports;
every returned item goes through `results::load_locked` in the existing bounded
Owner transaction. That loader verifies report/receipt bytes, nominal issued
execution and Started/Stopped history, registration/qualification, exact spec and
launch binding, outcome artifact linkage and report head. A malformed origin or
foreign association fails the read, never becomes an empty successful page.

The public repository/base and source/effect IDs remain projections of the
verified exact spec/report. Supplied receipt IDs are references, not proof of
retention, source coverage or settlement. This slice does not broaden the source
retention owner, synthesize checks, or infer B2/C outcomes. Historical phase
records remain readable after execution expiry/runtime retirement; stale exact
report digests and foreign project scopes still refuse.

## Focused qualification

Reuse the actual disposable Frozen and Refresh issuance/terminal/result owners
from `phase_results` with explicitly synthetic proof DTOs. Add focused router
cases per family covering opt-in list/exact detail, honest origin/coverage,
repository/base/source references, corrected head and UUID pagination, default
v1 omission/refusal, foreign project/revision, unknown query/profile, and current
Owner revocation and a real held-common-lock identity-rebind ABA refusal. Reuse
existing corruption vectors through the new public read
to prove nominal owner verification is retained. Add opt-in v2 assertions to the
two existing legacy human result tests. Run only these focused cases plus the
affected strict checks under an assigned lease, not the full phase pipeline.

No canonical schema, service, database, dashboard bundle or project is changed.
Actual provider/physical execution and phase media remain separately qualified.

## Qualified isolated candidate

Final product/test source `451e51a29c42645d7ccda97dc2dfc5b11a61dc75` passed six
focused phase SQL/router cases in **24.62s** (18 unrelated phase cases filtered)
and two legacy human-result cases in **1.75s** (47 unrelated cases filtered).
Their affected test compilation took 46.71s and 5.09s respectively. Strict clippy
for those two targets passed in 39.09s. Earlier pre-identity-fix source `d1b6e26`
passed the four functional cases in 18.32s; that log is retained as an earlier
increment, not final identity qualification.

Independent source review accepted actual historical origin dispatch, default
wire/filter compatibility, source-reference meanings, unchanged legacy-only
media and the final full-principal fix. The two new actual Node-lock barrier
vectors rebind then restore the same Owner fields before allowing B1 reads to
continue: the stale request returns 403 and a fresh request 200, with authority
invariants unchanged. Malformed nominal qualification returns 409; relabeling a
stored origin to the other phase family returns 500 rather than a successful or
empty read. Legacy v2 metadata becomes byte-equivalent to default v1 after
removing only the opt-in origin and restoring its coverage discriminator.

[Machine qualification and source hashes](evidence/human-phase-results/qualification.json)
and [retained logs](evidence/human-phase-results/README.md) preserve the exact
commands, counts and limits. These are actual disposable SQL/router owners with
explicitly synthetic proof/source/runtime fixtures. No actual Codex, provider,
Graph completion, canonical service, dashboard bundle or phase media was run or
changed in this qualification. No full 18-case phase suite was rerun.

## Dashboard consumer follow-up

A read-only audit of dashboard `4457ab7` identified the bounded remaining join.
No dashboard code or bundle changed as part of this API integration.

- Advertise a dedicated versioned native-result read capability from the server
  when the supported owner/profile is available. Existing `results.read` alone
  cannot distinguish an older server. Missing or unsupported capability must
  produce an explicit unavailable state without a native-profile request or
  silent fallback.
- Keep the legacy adapter strict. Add a separate native adapter requiring
  `recorded_native_v2` coverage and a valid origin on every row. That profile
  includes all three supported origins, including legacy records.
- Carry the profile through list/detail requests, pagination and navigation.
  Include it in report and media-window cache identity alongside the existing
  principal/scope/report/revision keys.
- Preserve the media owner and wire contract. Only validated legacy-origin rows
  can construct its existing legacy media binding; phase rows mount no media or
  upload client and issue no media requests, even when global media capabilities
  are enabled. Do not relabel a native page's coverage as legacy.
- Preserve the Results mock hierarchy, density and responsive composition. B1
  metadata does not establish acceptance, and its source base is not a claimed
  result checkpoint. UUID keyset order is not a chronological latest-report
  guarantee. Verify these meanings, request/cache isolation and media exclusions
  with focused adapter/view tests and the dashboard's required screenshots.

The principal files are `src/api/adapters/results.ts` and
`src/features/results/Results.tsx` in aq-dashboard, plus its existing result/media
fixtures and tests. Reuse the current cards and the unchanged legacy media
adapter; this follow-up is not a Results layout redesign or a phase-media grant.
