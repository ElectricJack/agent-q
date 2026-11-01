# Opt-in local playbook/review owner activation design

This optional implementation activates one explicitly configured catalog2 namespace. Absence of
`[playbooks]` preserves API-only defaults; invalid enabled configuration refuses
startup. It adds no provider, notification or executed-check adapter.

The first transport is the existing PostgreSQL service through a trusted local
Unix socket only. A private bounded DSN must explicitly select one absolute Unix
socket directory, one port/user/database and sslmode=disable; TCP, hostaddr,
multiple endpoints and server options are refused. This is local IPC, not a
plaintext network fallback. The held no-follow directory must be non-writable
by group/others, owned by root or the explicitly configured server UID. The
socket must be a real socket owned by that UID. Its fd-relative path is used for
connection; directory identity is rechecked before connecting. Parent placement
and the configured server UID are operator trust inputs. Ordinary Docker TCP-only
deployments cannot enable this profile; a verified-TLS TCP connector is later work.

Private engine and bridge key files are loaded at startup, never emitted in
Debug/HTTP/logs or passed to agents. Up to four trusted compiled artifact files
and explicit immutable bootstrap grants/bindings are installed. Restart reads
existing binding/authority first, refuses changed pins and never increments an
activation revision, renews a grant or restores revoked permissions by replaying
configuration. Missing bindings require current live authority. Artifacts and
bootstrap declarations are bounded; no generic install/command HTTP endpoint is
added.

Startup checks actual ordered review/bridge schema and current owner/project,
initializes the exact source-pinned namespace, then acquires the real protected
engine on its dedicated thread. There is no schema repair or replacement owner
on timeout. Recovery precedes serving. Drive uses bounded existing quanta at a
bounded interval, skips overlap, and holds after uncertainty instead of blindly
retrying. Control priority is between synchronous calls, never interruption.
Shutdown stops scheduling, fences central runtime ownership and asks the actual
engine to stop/release; a timeout is unknown, not physical release proof.

Actual startup/restart/shutdown tests use disposable PostgreSQL and a synthetic
local Unix transport peer. Production startup wiring does not qualify real native
review input, B2 settlement, notifications, executed checks or PB04's 25 ms budget.

Socket/server/database selection is trusted operator input. Matching the logical
AQ database name is a configuration guard, not proof of physical PostgreSQL
cluster identity. A closed engine queue/reply channel is not release proof:
shutdown stays unknown unless this exact Driver retains a prior successful Stop
acknowledgment. No such acknowledgment is inferred from process liveness or an
empty queue.

## Configuration and retained boundaries

The optional section requires `enabled=true`, `profile="human_review_v2"`, an
exact `namespace` (namespace UUID/generation/name, existing project/workspace and
real owner-user UUID), `engine_connection_file`, `socket_owner_uid`,
`engine_keyring_file`, existing `protection` key-file configuration,
`drive_interval_ms` (250..60000), and 1..4 `deployments`. Each deployment names a
private `artifact_file`, bounded `binding`, stable `grant_id`, exact typed `scope`
and absolute `expires_at_ms`. Scope grants only the explicitly named finite
capabilities/recipients; the existing owner enforces 24 h for new grants. Restart
never extends the absolute deadline or clears revocation. Existing activation
pins must match; changes require an explicit new namespace instead of a silent
binding rewrite. A decision policy also requires configured current source
retention; an unavailable source owner cannot be replaced by a worker claim.

DSN example contents (in an owner-only regular file, not inline TOML):

```text
host=/run/aq-postgresql port=5432 user=agentq dbname=agentq sslmode=disable password='operator-supplied-private-credential'
```

The engine keyring is a separate owner-only JSON file with `active_key_id` and a
`keys` map of retained key IDs to 32-byte hexadecimal secrets. Bridge protection
uses the existing raw 32-byte key files. Keep independently generated keys and
retired decryption keys; no key is generated, rotated or discarded automatically.
Connection/key files are limited to 4 KiB, artifact files to 240 KiB and all declared
artifacts to 1 MiB. Files must be owned by the daemon UID, regular, single-link and
not accessible by group/others; reads use no-follow/nonblocking held FDs.
Parent directory placement remains an operator responsibility. Never place these
files in an agent-writable root.

Engine owner bounds are 8 work/4 control requests, 8 transitions per tick,
128 runs, 2048 events, 256 queued destinations, 4096 receipts and 8 MiB logical engine
state. These are logical refusal bounds, not physical PostgreSQL/WAL/latency
budgets. The owner still uses synchronous bounded-session SQL on its dedicated
thread. Returned-call timeouts and the 30 s overall startup deadline do not prove a
stuck thread stopped. No replacement owner is spawned automatically.

Bootstrap spans separate central and engine transactions. Failure can retain
keyed grants or installed artifacts; it is not an all-or-nothing distributed
transaction and performs no blanket cleanup. No automatic drive starts before
all bootstrap declarations and recovery finish. Exact restart checks those
retained declarations without silently changing authority. Auto-drive holds on
uncertainty/refusal, and pending state remains inspectable for explicit recovery.
At most two accepted questions belonging to this namespace are considered for
deadline finality per quantum; expiry is separate from a human approval. The
current request/receipt/outbox owner and its locks remain authoritative.

`GET /api/playbooks/bindings/{id}` supplies the actual bounded activation revision
for a subsequent explicit admission CAS. It shares namespace-owner auth and the
existing work admission gate; no install, activation, arbitrary executor or
passed-check endpoint is added. Status reports the actual catalog and automatic
driver state. Existing CSRF, maintenance exceptions and separate worker-control
pool are unchanged. Only existing authenticated human routes receive the owner;
the worker listener gets no playbook/review routes.

The overall startup timeout can occur before acquisition, during a synchronous
engine call, or after central runtime adoption while bootstrap is incomplete.
It reports startup failure with an **uncertain retained prefix**, never
`not_applied` or released ownership. Dropping the last Driver before the managed
runtime exists marks its queue closing; it does not join or release the engine
from the async thread. The dedicated thread retains its actual storage connection
and ownership while a pending synchronous call runs, then observes closing and
releases when it can. A central incarnation may remain recorded. Restart must
reacquire actual engine ownership and exact-replay retained declarations; neither
a timeout nor a missing response is permission to spawn a replacement owner.
The frozen host's stalled-drop test and the new actual competing-owner startup
case qualify these narrow lifecycle boundaries, not arbitrary crash cuts or
an interruption deadline for a stuck OS/storage call.
