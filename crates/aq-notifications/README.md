# aq-notifications

Pure bounded notification delivery transitions plus a fixed Discord v10 webhook
adapter. The crate owns no database, worker credential, human authorization or
background retry loop. `Delivery::begin` returns a proposed state; only the
central adapter's committed transaction grants one HTTP attempt.

The adapter uses `wait=true`, disables mentions/embeds, requires a matching
webhook receipt, and performs no redirects, ambient proxy use or automatic
HTTP retries. Service failures and missing responses leave delivery unknown.
A transport receipt means accepted by Discord, never read, acknowledged or
approved by a human. Rate limits retain the longest observed delay.

`aq-server::notifications` owns current routing, project/user authorization,
durable intent/attempt/receipt records, finite quotas and dispatch recovery.
The portable human request types are shared with `aq-notify`. Existing AQ agent
mailbox contracts remain unchanged. Tests use loopback peers; the `test-support`
feature permits loopback endpoints and is not enabled in normal builds.
