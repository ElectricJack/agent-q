# commands-1: the AQ command surface (spec 0006)

This is the contract for AQ's generic command surface: what an agent, the
supervisor or the owner may run, and how a refusal says why. The design is
[`docs/specs/0006-agent-channel.md`](../../../docs/specs/0006-agent-channel.md).

- `schema.json`: the grant vocabulary (`Operation`, `Scope`, `Grant`,
  `GrantSet`) and the structured refusal (`Refusal`, §6.8).
- `fixtures/`: refusals exactly as the server produces them. The
  `aq-agent-grants` unit tests build the same values and compare.
- `check.py`: validates the schema and fixtures (`python3 check.py`).

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

On the `/v1/agent/*` question, comment and wait routes the structured body is
returned when the request carries `X-AQ-Refusal-Version: 1`. Without it those
routes keep the closed agent-access-1 `ErrorResponse` (code
`unsupported_capability` or `scope_denied`), whose `message` carries the same
text, so pinned agent-access-1 clients keep validating it.

## Audit

Every authorization decision, allow or deny, is one `command_audit` row:
caller, command, grant, target (own attempt, or the caller's project and the
task's relation to the caller's task), decision, reason code, operation ID and
request ID.
