# Agent waits v1

These are the agent-facing durable wait routes served by `aq-server` (H4). Every request and response is the `aq-lifecycle-api` envelope. The schemas live in `crates/aq-lifecycle-api/artifacts/request.schema.json` and `response.schema.json`.

- `routes.json` maps each route to its command, capability and fixture.
- `fixtures.json` holds sealed requests that `decode_request` accepts. The unit test `wait_route_fixtures_decode` checks them.

## Rules

- **Authentication:** the per-attempt machine credential. The node and attempt come from the authenticated binding. A registration whose scope or binding differs from that binding is refused with `scope_denied`.
- **Capabilities:** the command's capability must match the route, otherwise the request gets `unsupported_capability`.
- **Sources:** `message`, `task`, `dependency` and `human_decision`. Any other source kind is refused with `unsupported_capability`.
- **Replay:** the same `operation_id` returns the original response, byte for byte, including `original_created_at_ms`. A changed body returns `changed_payload`.
- **Bounds:**
  - at most 32 members;
  - one active wait per attempt (a second one gets `invalid_request`);
  - a maximum age of 30 days;
  - self-waits get `self_wait`;
  - known cycles get `known_cycle`.
- **Outcomes:** `satisfied`, `source_failed`, `cancelled` and `deadline_reached`. When a source is temporarily unreachable, the wait stays pending. Inspect shows the member's freshness and the hold reason.
- **Ack:** an ack consumes the ready resume intent exactly once. A duplicate `admission_id` returns the original receipt. A stale continuation generation returns `stale_generation` and leaves the intent ready.
- **Owner routes:** `GET /api/waits[?project_id=]` and `GET /api/waits/{wait_id}`. They require owner auth and are intended for the G10 views.

## Grants (spec 0006 phase 0)

Each route also requires the spec 0006 grant named in its `grant` field, sealed with the launch (`agent_launch_grants`). An attempt that does not hold it, or holds it at a scope that does not reach the target, is refused before any effect. The body is the closed agent-access-1 `ErrorResponse` (`unsupported_capability` for a missing grant, `scope_denied` for a scope or quota refusal) whose `message` names the command, the missing grant and the grants of the same family the attempt holds. A client that sends `X-AQ-Refusal-Version: 1` gets the structured refusal instead (`protocol/commands/v1/schema.json#/$defs/Refusal`). Every decision, allow or deny, is recorded in `command_audit`.
