# G1b — existing dashboard actions

Task: `steady-cascade-45.2`. Implementation stays on the assigned task branch.
The server publishes `task.create`, `decision.resolve` and
`supervisor.message.send` through `POST /api/v1/actions` and original-ID lookup
through `GET /api/v1/operations/{id}`. Dashboard caller migration belongs to G3/G8.

The adapters call the existing fixed-key human Graph operation owner, protected
exact review owner and durable global mailbox owner. The dashboard journal stores
admission identity, disposition and bounded references; it does not retain another
copy of task/message/decision intent. Legacy HTTP handlers share that admission
path and keep native response envelopes. Native preflight rejects changed intent
before consuming an ID already held in a domain journal. Native task byte-sensitive
replay remains in force; union task payloads use sorted JSON native bytes.

Task IDs equal operation IDs. The existing journal persists the original mapped
Graph request before dispatch, validates actual Graph receipts, and limits recovery
to the original fixed key. Read-only union lookup never dispatches. Explicit native
recovery can complete an ambiguous operation, which subsequent lookup reconciles.
Decision requests bind the exact decision revision and subject artifact; the native
review transaction checks the subject under authority locks and fences the full
current authenticated actor. Supervisor confirmation means durable enqueue only;
message ID remains caller-controlled and distinct from operation ID. Native
receipts retain `host_delivery: not_attempted` and no agent receipt.

Owner bootstrap enables `dashboard.actions` and keeps specific mutation capability
checks. Its envelope remains `dashboard-read-1`; a separate contract-reference enum
admits both read and action versions. The bounded principal-filtered SQL query
returns up to 100 unresolved IDs ordered by creation time and UUID, excludes
terminal/retired/foreign rows, and reports partial coverage on truncation. Current
role and authorization are checked again before delivery. Lookup rechecks current
scope and returned references and gives neutral absence for revoked access.

The generated server schema includes exactly three closed payloads. Fixtures cover
one request per kind, all five dispositions, errors 401/403/404/409/410/413/422/426/
429/503, expired cursor restart, equivalent replay and changed-payload conflict,
invalid requests, and read/action envelope compatibility. The validator checks UTF-8
byte bounds in addition to JSON Schema character bounds and verifies schema digests.
No dashboard asset, architecture/spec document, unrelated action family, migration
or dependency pin is changed.

## Validation

Validation is in progress. Final commands and results are recorded below before
publication. Commands run through AQ's admitted pytest subprocess adapter because
the installed AQ runner supports pytest rather than a Cargo backend. Cargo uses
`--locked`, two build jobs and two Rust test threads against a disposable local
PostgreSQL database. CI owns unrelated full-suite coverage.

Initial red: mounted tests failed the empty registry, disabled capability and empty
pending list. `.aq/tmp/g1b/red-actions.log` records those failures. The first mounted
implementation run passed 7/8 tests, including bootstrap bounds and recovery after a
native commit whose dashboard completion update was refused. The eighth test had a
wrong discovery URL; corrected to the real global conversation route and added a
success assertion. Intermediate compile errors are recorded in the scratch logs.

The existing real Graph response-loss test is extended to check shared union/native
identity, unresolved bootstrap discovery, read-only lookup, explicit original-key
recovery, post-recovery reconciliation and revoked access. It uses a copied pinned
local Graph binary, SHA-256
`882f7003a264ad322134113a35df319e346034191d7cfaa0960c85cb219c5167`.
The protected review fixture exercises actual owner admission and native finality,
not fabricated success responses. No browser run is claimed; G1b changes the server
contract and G3/G8 own dashboard consumers.

`cargo fmt --all` cannot discover vendored crates' workspace membership in this
worktree. `cargo fmt` / `cargo fmt --check` are the applicable workspace checks;
vendored dependencies are unchanged.
