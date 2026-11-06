# Agent comments v1

These are the agent-facing task comment routes served by `aq-server` (aq-dashboard spec 0004 §4.1). An agent leaves findings on its own task, the way v1's `aq task comment` works, and reads the task's thread, including comments left for earlier attempts. `routes.json` lists the routes, bodies and error codes; the unit test `agent_comment_routes_match_contract` checks the route table against it. The model is `crates/aq-task-comments`.

## Rules

- **Authentication:** the per-attempt machine credential of an active attempt. The project, task, session and attempt come from the authenticated binding, so an agent comments only on its own task and reads threads only in its own project. A revoked or expired credential gets 401.
- **Add** (`POST /v1/agent/comments`): records a comment with `author.kind = "agent"` and the attempt's `session_id` and `attempt_id`. The comment ID is the `operation_id`.
- **Replay:** the same `operation_id` with the same body returns the same comment. A different body, or an `operation_id` already used by another author, gets `id_conflict`.
- **List** (`POST /v1/agent/comments/list`): the task's thread newest first, `limit` 1-100 (default 50). Pass `next_cursor` back as `cursor` for the next older page. The cursor is a position only and grants nothing. Deleted comments appear as tombstones with `body: null`.
- **Other tasks** (spec 0006): `list` takes an optional `task_id`. A task other than the attempt's own needs `comment.read` at a scope that reaches it (`project:self` by default; `task:subtree` reaches the tasks this attempt created below its task). The thread is always in the attempt's project.
- **Edit and delete** (`POST /v1/agent/comments/edit`, `/delete`, spec 0006): only a comment this attempt wrote (`author.session_id` and `attempt_id` are the credential's), in its project, at its current `expected_revision`. Another author's comment is `scope_denied`, a missing one `not_found`, a stale revision `stale_revision` (409) and a deleted one `invalid_request`. The response carries the comment and the revision the operation made. Replay works as for add.
- **Owner side:** owners read the thread through `GET /api/human/projects/{p}/tasks/{t}/comments` (human-graph-1) and write through the `dashboard-actions-1` kinds `task.comment.add|edit|delete`.
- **Startup context:** the thread is not yet part of the attempt's startup context. That context is the digest-pinned `ContextSnapshot` artifact assembled by `profiled_execution::producer::build::assemble` through `profiled_execution::context_projection::project`; the agent-access/v1 `ContextSnapshot` schema is closed (`additionalProperties: false`) and pinned by aq-worker, so adding the thread there is a coordinated agent-access contract change. Until then a later attempt reads the thread with `comments/list`.

## Grants (spec 0006 phase 0)

Each route also requires the spec 0006 grant named in its `grant` field, sealed with the launch (`agent_launch_grants`). An attempt that does not hold it, or holds it at a scope that does not reach the target, is refused before any effect. The body is the closed agent-access-1 `ErrorResponse` (`unsupported_capability` for a missing grant, `scope_denied` for a scope or quota refusal) whose `message` names the command, the missing grant and the grants of the same family the attempt holds. A client that sends `X-AQ-Refusal-Version: 1` gets the structured refusal instead (`protocol/commands/v1/schema.json#/$defs/Refusal`). Every decision, allow or deny, is recorded in `command_audit`.
