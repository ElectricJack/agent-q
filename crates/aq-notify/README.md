# aq-notify

Operator access to central human notifications. This executable uses human
Cloudflare Access authentication; node and agent credentials do not authorize
these routes. Build with `cargo build --locked -p aq-notify --release`.

```
aq-notify --origin https://aq.example --cf-access-token-file /private/access.jwt inbox
aq-notify --origin https://aq.example --cf-access-token-file /private/access.jwt inspect MESSAGE_UUID
aq-notify --origin https://aq.example --cf-access-token-file /private/access.jwt send --request send.json
aq-notify --origin https://aq.example --cf-access-token-file /private/access.jwt route --request route.json
aq-notify --origin https://aq.example --cf-access-token-file /private/access.jwt retry-unknown --request retry.json
```

Save typed request JSON before submitting. Preserve its operation ID and message
ID across retries; changing an ID can create a new intent. Requests are at most
16 KiB; output is at most 512 KiB. The CLI never retries HTTP automatically and
omits untrusted server error bodies. An uncertain response requires inspection
of the retained IDs. Request files must be regular files, not links or pipes.

`SendRequest` contains `operation_id`, `message_id`, `revision`,
`scope: {project_id, workspace_id}`, `recipient_user_id`, `summary`, and `ttl_ms`.
Only an owner can send or edit routes. A recipient can inspect their messages;
an owner can inspect any current scoped message. Viewer access to the graph is
not borrowed as authority to send externally. The summary is explicitly supplied
for external delivery; no transcript is exported automatically.

`RouteRequest` contains `operation_id`, `expected_revision`, and `route` with
`route_id`, `revision`, `scope`, `recipient_user_id`, `endpoint_key`,
`endpoint_revision`, `enabled`, and `retry`. Retry fields are `max_attempts`
(1–8), `max_elapsed_ms` (1 second–24 hours), `initial_delay_ms` (250 ms–1 minute),
`max_delay_ms` (at most 1 hour), and `allow_duplicate_risk`.

`RetryRequest` contains `operation_id`, `delivery_id`, `expected_revision`, and
`accept_duplicate_risk: true`. It applies only to unknown deliveries whose
original route permits duplicate risk. It preserves cumulative attempt and
elapsed budgets. Identical retry intent replay cannot create another attempt.

The daemon configuration uses protected endpoint files:

```toml
[notifications]
revision = 1
[[notifications.endpoints]]
key = "discord-operations"
revision = 1
secret_file = "/private/discord-operations.webhook"
```

The secret file contains one HTTPS Discord v10 webhook URL. It must be an
owner-only regular file belonging to the daemon user, with no hard links or
final symlink. File contents never enter SQL, CLI output or logs. Change both
configuration and endpoint revisions when rotating a destination; missing files
disable that destination without preventing dashboard/API inspection. Omit the
section entirely to operate with dashboard/API notifications only.

Installed capacity is explicit: 128 MiB of reserved notification state, 4096
logical messages, 4096 deliveries, 128 routes, 128 retained destination keys,
and 8192 operation receipts. Quota exhaustion refuses the entire transaction.
This version has no receipt garbage collection or implicit identity reuse.
At most eight routes fan out one logical message, four HTTP sends run globally,
and one attempt is in flight per logical destination. A fair bounded scan
prevents a backed-off page from permanently hiding later work.

Discord links currently open the authenticated inspection API. The dashboard
can render the same message and delivery IDs when its notification view lands.

Local fixtures may build `--features loopback-development` and use
`--dev-email EMAIL` with a loopback HTTP origin. This explicit build feature is
not a production authentication fallback. Real provider testing is separately
opt-in; see `docs/implementation/AQ-24-plan.md` and INT03.
