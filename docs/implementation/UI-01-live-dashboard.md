# Initial dashboard/framework integration

Status: implementation in progress, 2026-10-31.

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
behavior, asset integrity and API separation. A built-browser run against the
real assembled daemon and disposable PostgreSQL is still pending. Component
mocks alone do not establish that boundary. The final dashboard record will
retain its exact assembled source revisions, commands and outcomes; no product
execution/session gate is closed by the component checks.

Frontend plan: [live framework connection](../../../aq-dashboard/docs/plans/0004-live-framework-connection.md).

## Merged checkpoint and qualification

API-02 and UI-01 remain **in progress**. The following local changes are merged
on the daemon integration branch; no release, deployment or gate completion is
implied:

| Boundary | Frozen source | Integrated source / evidence |
| --- | --- | --- |
| Human dashboard API and generated `dashboard-read-1` schema/fixtures | `3632eada81389f2dd9d0ab3a1f148c78746c9a42` | `44dcab8` / `8bc7a63` |
| Startup-verified optional static bundle service | `14bd10840f4abf46148e905f9feb90fe2af7c7b4` | `26201d9` / `22d375e` |

The frontend committed checkpoint observed during this reconciliation is
`aq-dashboard` `4bd914af9c2dc59d06696b035792946fa40cc80e`. Subsequent live
compatibility work is not frozen by that pin; the pending D05 record must name
the exact frontend source and built bundle used by the assembled browser run.

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
successfully with locked/offline dependencies. The retained local build log is
`/tmp/aq-dashboard-live-server-build.log`; this is build evidence only. Frontend
live compatibility fixes and actual built-browser smoke are **pending**, owned
by the dashboard integration lane. Its intended final record is
[aq-dashboard/docs/evidence/d05-live-framework.md](../../../aq-dashboard/docs/evidence/d05-live-framework.md)
(to be created after the smoke). The existing [D04 record](../../../aq-dashboard/docs/evidence/d04.md)
covers earlier frontend/mock qualification and must not be read as daemon/browser
integration evidence. No broader API, execution/session, release or product gate
is closed by these checks.

Retained backend evidence: [summary](dashboard-read-1-evidence.md),
[hash inventory](dashboard-read-1-evidence.json),
[live schema/fixtures](../../protocol/dashboard-read/v1/README.md).
Retained static evidence: [qualification](../evidence/dashboard-assets/README.md)
and [source inventory](../evidence/dashboard-assets/source.json).
