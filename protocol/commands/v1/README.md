# commands-1: the AQ command surface (spec 0006)

This is the contract for AQ's generic command surface: what an agent, the
supervisor or the owner may run, and how a refusal says why. The design is
[`docs/specs/0006-agent-channel.md`](../../../docs/specs/0006-agent-channel.md).

- `commands.json`: every registered command, generated from the server
  registry (`crates/aq-command-registry` plus every `ActionKind`). The
  aq-server test `it_server commands::commands_json_is_generated_from_the_registry`
  fails when it drifts; regenerate with
  `AQ_REGENERATE_COMMANDS=1 cargo nextest run -p aq-server --test it_server -E 'test(/^commands::/)'`.
- `routes.json`: the reviewed routes outside the registry (deny by default,
  §6.5). `commands::every_mounted_route_is_a_command_or_reviewed` fails on
  any mounted route that is neither a command route nor listed here.
- `schema.json`: the grant vocabulary (`Operation`, `Scope`, `Grant`,
  `GrantSet`), the structured refusal (`Refusal`, §6.8), `CommandRequest`,
  `Whoami` and `Discovery`.
- `fixtures/`: refusals exactly as the server produces them. The
  `aq-agent-grants` unit tests build the same values and compare.
- `check.py`: validates the schema, fixtures, `commands.json` and
  `routes.json` (`python3 check.py`).

## Invoking a command

`POST /api/v1/commands/{name}` with a `CommandRequest`:

```json
{"operation_id": "…", "subject": {"type": "task", "id": "…"}, "expected_versions": {}, "reason": "…", "params": {…}}
```

Only `params` is needed for most commands. `operation_id`, `subject`,
`expected_versions` and `reason` are the action-union fields, used by a
mutation whose route is `/api/v1/actions` (its `body` is `action_request`).

The command surface is on both listeners:

| Listener | Caller | Credential |
| --- | --- | --- |
| admin | owner (or viewer) | Cloudflare Access |
| machine | agent | the attempt's `aqa_` bearer (inside a sandbox, the channel proxy adds it) |
| machine | supervisor | the F1b `aqsv1_` bearer (scope `enqueue`) |

Each command lists, per principal, the route that serves it, the `body`
mapping and the `params_schema` (with its `params_digest`). The server builds
that route's request from `params` and serves it in process, so the
response is that route's response: the same receipts, idempotency and
recovery as the REST form. Responses carry `X-AQ-Command: <name>`.

Body mappings:

| `body` | The route receives |
| --- | --- |
| `params` | `params` |
| `params_with_api_version` | `params` plus `"api_version": 1` |
| `query` | a GET; path segments from the named params, every other param as a query string value |
| `graph_read` | an agent-access-1 `ReadRequest` with the attempt's binding; `knowledge.search` defaults `kind_filter` null, `limit` 20, `cursor` null |
| `graph_command` | an agent-access-1 `CommandRequest`: `params` plus the binding (and `operation_id`/`reason` from the request when absent) |
| `action_request` | an `ActionRequest` with `kind` = the command name and `payload` = `params` |
| `local` | answered by the command surface (`whoami`) |

For an agent, an omitted `project_id` is the attempt's project; a
`project_id` that differs from it is refused (`scope_denied` or
`grant_missing`) unless an owner override grants that project. Agent routes
take the project from the binding, so the param is not forwarded.

## Discovery and identity

- `GET /api/v1/commands` lists what the caller may run (a `Discovery`), each
  with the scope of the grant it holds. It is filtered by the same check that
  `invoke` applies. `?all=true` also lists the others with the reason:
  `grant_missing`, `owner_only` or `not_available` (no route for this
  principal).
- `whoami` (`POST /api/v1/commands/whoami`, empty params) returns the
  principal (`Whoami`): for an agent, the attempt, task, project, session,
  profile, grants, credential expiry and authority revision.

## Errors

Every refusal and error on the command surface is a `Refusal` body. The
grant codes are below; the surface's own errors are `unknown_command` (404),
`not_available` (403), `invalid_request` (400), `unauthorized` (401) and
`internal` (500). Errors from the route that serves a command are that
route's errors; the command surface asks the `/v1/agent/*` routes for the
structured refusal (`X-AQ-Refusal-Version: 1`).

## Grants

A grant is an operation plus a scope. Scopes: `attempt:self`, `task:self`
(the attempt's task), `task:subtree` (the task and its descendants),
`project:self` and `project:<uuid>` (owner override only). Owner-only
operations (answering questions, resolving decisions, knowledge verify and
retire, evidence waivers, playbook control, administration and grant
changes) are not in the vocabulary and can never be held by an agent.

An attempt's grants are sealed with its launch in `agent_launch_grants`.

## Refusals

A refusal names the command, the operation it needed and the grants of the
same family the caller holds. It never names or confirms any other record.

| Code | Meaning | HTTP |
| --- | --- | --- |
| `grant_missing` | No grant for this command | 403 |
| `scope_denied` | The grant is held, but the target is outside its scope | 403 |
| `owner_only` | The command can never be granted to an agent | 403 |
| `quota_exceeded` | A grant limit such as `max_created_tasks` was reached | 403 |
| `session_revoked` / `session_expired` | The attempt's credential is no longer valid | 401 |
| `channel_refused` | The sandbox channel proxy refused the path (aq-worker) | 403 |
| `unknown_command` / `not_available` / `invalid_request` / `unauthorized` / `internal` | Command-surface errors (above) | 404 / 403 / 400 / 401 / 500 |

On the `/v1/agent/*` question, comment and wait routes the structured body is
returned when the request carries `X-AQ-Refusal-Version: 1`. Without it those
routes keep the closed agent-access-1 `ErrorResponse` (code
`unsupported_capability` or `scope_denied`), whose `message` carries the same
text, so pinned agent-access-1 clients keep validating it.

## Audit

Every authorization decision, allow or deny, is one `command_audit` row (a
command forwarded to a `/v1/agent/*` route is audited once, by the command
surface):
caller, command, grant, target (own attempt, or the caller's project and the
task's relation to the caller's task), decision, reason code, operation ID and
request ID.
