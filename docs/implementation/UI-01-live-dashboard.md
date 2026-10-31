# Initial dashboard/framework integration

Status: initial live read connection qualified, 2026-10-31; broader implementation in progress.

The independent `aq-dashboard` application connects through the AQ human listener.
The initial `dashboard-read-1` profile covers authenticated bootstrap, authorized
project/session inspection and a partial overview. It reuses the existing status
and worker heartbeat APIs. Missing task, decision, search, supervisor, quota and
terminal capabilities remain explicitly unavailable. This read subset is
implemented against existing owners while API-02's broader domain dependencies
and UI-02's mutation/recovery requirements remain open.

The static bundle is built and tested in `aq-dashboard`, then supplied as an
artifact. Optional `AQ_DASHBOARD_DIR` startup loading verifies an exact manifest,
asset digests and bounded size before serving immutable bytes on known UI paths.
It adds no browser-facing database or separate dashboard backend. The worker
listener never serves dashboard pages. The optional `compose.dashboard.yaml`
overlay mounts an independently supplied bundle; publishing it in the signed
product release remains separate release work.

The frontend negotiates capabilities individually and rejects unsupported API
versions. Authentication/authorization failures remove protected content and
cached queries. Partial lifecycle coverage remains visible and unknown totals
stay unknown. A grant, heartbeat or UI row is not proof of a live agent process.

Component qualification below covers actual human-route ACL/scoping/cursor
behavior, asset integrity and API separation. The production bundle also passes four browser checks against the real assembled
daemon and disposable PostgreSQL, including a nonempty fixture session and mobile
deep-link reload. The retained D05 record pins the exact assembly and separates
fixture visibility from real agent execution. No product execution/session gate
is closed by this read connection.

Frontend plan: [live framework connection](../../../aq-dashboard/docs/plans/0004-live-framework-connection.md).

## Merged checkpoint and qualification

API-02 and UI-01 remain **in progress**. The following local changes are merged
on the daemon integration branch; no release, deployment or gate completion is
implied:

| Boundary | Frozen source | Integrated source / evidence |
| --- | --- | --- |
| Human dashboard API and generated `dashboard-read-1` schema/fixtures | `3632eada81389f2dd9d0ab3a1f148c78746c9a42` | `44dcab8` / `8bc7a63` |
| Startup-verified optional static bundle service | `14bd10840f4abf46148e905f9feb90fe2af7c7b4` | `26201d9` / `22d375e` |

The qualified frontend source is `aq-dashboard`
`dbbba20936a9ff9c8767796832aa29b9a0523368`; retained evidence is committed as
`956d587`. The production manifest SHA-256 is
`6d19f55ac7103e3993ddc0ea6f10cd9b20250abdf940690f99afee7198228f68`, also
reported by the running bootstrap. The frontend passes 234 unit/component tests,
35 mock browser/accessibility checks and four actual-daemon browser checks, with
no live test skipped. Typecheck, mock-exclusion and manifest generation pass.
Resource authorization/compatibility errors immediately gate views and clear
protected caches; each read checks the negotiated version and requested scope.

Backend qualification: five actual disposable-PostgreSQL/authenticated-router
tests and two pure cursor tests pass, with strict targeted lint, formatting,
schema-generation drift and five retained actual-route fixture schema checks.
The fixtures use synthetic sealed authority records; they prove projection and
ACL behavior, not native launch or provider activity. Role/binding and ordered
ACL rows are checked under short share locks, expiry is checked after awaited
SQL, each session releases common authority locks before another item, and a
final authorization partition fence precedes delivery. Read results, cursors,
request concurrency and SQL deadlines are bounded; no constant-history database
scan-performance claim follows from those bounds.

Static-service qualification: 88 focused checks pass and one existing immutable
Git-source fixture remains explicitly ignored. This includes four loader cases,
three actual PostgreSQL/router/local-HTTP cases, existing JWT/authz and worker
API regressions, plus strict all-target/all-feature server lint and formatting.
It verifies integrity, safe asset/path bounds, current human access, API-only
mode and worker-listener separation. It is separate from browser integration.

The assembled daemon at `8bc7a63bac9cab843d307b584c87683c1b51c6b9` builds
successfully with locked/offline dependencies. The build log, bundle manifest, source pins, exact test results and negative
outcomes are retained in
[aq-dashboard D05](../../../aq-dashboard/docs/evidence/d05-live-framework.md).
The assembled browser run used loopback development auth and a session seeded
through the existing development fixture API. It issued no accept, grant, agent
launch or provider call. The optional local preview runs at port 4188; normal
installation instructions are in the dashboard README. Publishing a signed bundle,
terminal interaction and additional task/workflow/domain views remain separate.
No broader API, execution/session, release or product gate is closed by these checks.

Retained backend evidence: [summary](dashboard-read-1-evidence.md),
[hash inventory](dashboard-read-1-evidence.json),
[live schema/fixtures](../../protocol/dashboard-read/v1/README.md).
Retained static evidence: [qualification](../evidence/dashboard-assets/README.md)
and [source inventory](../evidence/dashboard-assets/source.json).
