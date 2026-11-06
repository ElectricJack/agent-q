-- Spec 0006 phase 0: agent grants sealed with the launch, and one audit row
-- per authorization decision on the agent-facing surface.
--
-- agent_launch_grants holds the attempt's (operation, scope) set
-- (aq-agent-grants), sealed with its launch binding. It is a table beside
-- agent_launch_bindings, not a column of it, because settlement39 pins that
-- table's exact shape. An attempt with no row holds no grant, so the
-- question, comment and wait routes refuse it with grant_missing.
CREATE TABLE agent_launch_grants (
 attempt_id UUID PRIMARY KEY REFERENCES agent_launch_bindings(attempt_id),
 grants JSONB NOT NULL CHECK (jsonb_typeof(grants)='array'),
 grants_digest TEXT NOT NULL CHECK (grants_digest ~ '^sha256:[0-9a-f]{64}$')
);

CREATE TABLE command_audit (
 audit_id UUID PRIMARY KEY,
 recorded_at_ms BIGINT NOT NULL CHECK (recorded_at_ms>=0),
 caller_kind TEXT NOT NULL CHECK (caller_kind IN ('owner','viewer','supervisor','agent')),
 caller_id UUID NOT NULL,
 attempt_id UUID,
 task_id UUID,
 project_id UUID,
 command TEXT NOT NULL CHECK (length(command) BETWEEN 1 AND 128),
 grant_operation TEXT CHECK (grant_operation IS NULL OR length(grant_operation) BETWEEN 1 AND 128),
 target JSONB NOT NULL DEFAULT '{}'::jsonb,
 decision TEXT NOT NULL CHECK (decision IN ('allow','deny')),
 reason_code TEXT CHECK (reason_code IS NULL OR length(reason_code) BETWEEN 1 AND 64),
 operation_id UUID,
 request_id UUID NOT NULL,
 CHECK ((decision='deny')=(reason_code IS NOT NULL))
);
CREATE INDEX command_audit_attempt ON command_audit(attempt_id, recorded_at_ms) WHERE attempt_id IS NOT NULL;
CREATE INDEX command_audit_task_denials ON command_audit(task_id, recorded_at_ms) WHERE decision='deny';
UPDATE meta SET value='97' WHERE key='state_format_version';
