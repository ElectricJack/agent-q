# AQ-24 staged notification delivery

Status: implementation in progress in isolated `feat/aq24-notifications`.
Migration 0012 is reserved after AQ08 authority 0010 and AQ21 terminal 0011.
No real destination is configured or contacted during development.

## Ownership and scope

The existing mailbox implements task/session recipients, not human recipients.
Keep those published routes and API 1 bytes unchanged. Add a central logical
human-message owner keyed by immutable message UUID/revision and explicit
project/workspace/user recipient. This owns notification content and dashboard
inspection; per-destination rows own transport attempts only. A bounded trusted
service interface accepts messages from later playbook/escalation adapters.
Agents do not provide webhook URLs or choose external destinations. Initially
only authenticated owners can create human messages/routes; missing scoped agent
notification authority refuses rather than borrowing task-mail permissions.

An independent Rust `aq-notifications` crate provides bounded pure transitions
and a Discord HTTP adapter. It consumes existing lifecycle inspection types;
actual database authority remains in the daemon. Pure transitions alone are not
persistence. The daemon atomically stores immutable logical content, route
revision, payload digest and stable delivery ID before dispatch. Dispatch rechecks
current route, user/project visibility, revision, expiry and protected endpoint
configuration. It then commits a uniquely fenced attempt before external HTTP.
Network and protected-file reads occur outside database locks. A crash after
Dispatching becomes unknown, never an automatic fresh send.

## Transport semantics

Use Discord API v10 webhook execution with `wait=true`, a bounded plain summary
and authenticated AQ link, disabled mentions, suppressed embeds, no attachments
and no username/avatar/thread overrides. A validated message ID confirms transport
acceptance only. Loss before recording the response remains unknown because a
lookup requires the lost provider message ID; no idempotency support is assumed.
An explicit recorded duplicate-risk retry policy is required to retry unknown
sends. All such retries retain logical/delivery identity and increment attempts.
HTTP 429 waits at least the provider delay; global throttles apply to the provider
scope. Bounded attempts/time, per-destination backoff and finite concurrency prevent
retry floods. Revoked/deleted destinations suppress future sends but cannot recall
previous content. No transport receipt acknowledges a human or answers a decision.

These API details follow the [Discord webhook reference](https://docs.discord.com/developers/resources/webhook)
and [rate-limit reference](https://docs.discord.com/developers/topics/rate-limits),
read 2026-10-31. Provider behavior still needs opt-in real-destination qualification.

## Concrete persistence and integration

Migration 0012 will add logical human messages, immutable delivery identities,
operator-configured versioned routes, attempt receipts and bounded replay metadata.
Each delivery binds exact scope/message/route/payload. Stable IDs conflict on changed
content. Unknown attempts never disappear through ordinary expiry. Pending/active
rows remain pinned. Initial explicit row/byte/retention ceilings refuse new work
rather than drop receipts; a later qualified tombstone policy can reclaim history.

Protected configuration references owner-only regular files holding webhook URLs;
URLs/tokens never enter SQL, response errors, audit text, CLI arguments or workers.
A missing external config leaves startup/dashboard usable and records
`no_external_route`. A configured but unsafe file fails closed. Endpoint parsing
allows only the selected Discord origin and fixed v10 webhook path, with no redirects;
loopback fake transport is a separately enabled test capability.

A separate bounded dispatcher pool never uses execution-control connections or
semaphores. One due delivery per destination at a time, finite response/body limits,
route fairness and persisted provider deadlines isolate failures. Metadata-only
inspection/retry APIs and a finite CLI expose unknown, no-route and dead-letter
states. Authentication is re-read inside the mutation transaction. No event or
notification can recursively enqueue its own failure report.

Qualification includes state-machine fake clocks, actual fake HTTP, PostgreSQL
crash prefixes/restarts/CAS races, scope/revocation and isolated control capacity.
An opt-in ignored real Discord test may exist, but is never run automatically and
requires explicit destination authorization. This task does not implement inbound
Discord, human decisions, digest policy, or claim exactly-once remote delivery.

## Implemented checkpoint and remaining integration

The isolated implementation includes a pure delivery crate, bounded Discord
adapter, PostgreSQL intent/outbox owner, human API and `aq-notify` executable.
At this checkpoint 13 actual PostgreSQL/API/fake-HTTP cases, two real CLI process
cases and strict all-target lint pass. Tests do not send messages to humans.

Notification migration takes **011** by agreement with AQ21; terminal ledger
migration is now **012**. Neither migration had been applied outside disposable
test databases when this ordering changed. Runtime integration and clean full
qualification remain required before AQ24 is marked complete.

`aq-notify` defaults to Cloudflare human credentials from an owner-only token
file. Its explicit `loopback-development` build feature permits local test
identity. Save a typed request JSON with stable operation/message IDs, then use
`send --request FILE`, `route --request FILE`, or
`retry-unknown --request FILE`. The CLI never invents replacement IDs or retries
mutations automatically; `inbox` and `inspect MESSAGE_ID` reconcile retained
results. `retry-unknown` also requires the frozen route's duplicate-risk policy,
a current record revision, and `accept_duplicate_risk: true` in the intent.

Provider qualification is an ignored INT03 test, `notification_discord_live`.
It requires separately authorized operator execution, an owner-only
`AQ_NOTIFICATION_REAL_WEBHOOK_FILE`, and
`AQ_NOTIFICATION_REAL_SEND_CONFIRMED=1`. It sends exactly one fixed qualification
message, uses no ambient proxy/HTTP retry, and does not claim full outbox or human
acknowledgment qualification. It has **not** been run during implementation.
