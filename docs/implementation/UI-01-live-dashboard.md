# Initial dashboard/framework integration

Status: implementation in progress, 2026-10-31.

The independent `aq-dashboard` application connects through the AQ human listener.
The initial `dashboard-read-1` profile covers authenticated bootstrap, authorized
project/session inspection and a partial overview. It reuses the existing status
and worker heartbeat APIs. Missing task, decision, search, supervisor, quota and
terminal capabilities remain explicitly unavailable. This read subset can be
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

Validation must include actual human-route ACL/scoping/cursor behavior, asset
integrity and API separation, plus a built-browser run against the real daemon
and disposable PostgreSQL. Component mocks alone do not establish integration.
Exact source revisions, commands and outcomes will be retained in the dashboard
evidence record after the assembled smoke. No product execution/session gate is
closed by this integration.

Frontend plan: [live framework connection](../../../aq-dashboard/docs/plans/0004-live-framework-connection.md).
