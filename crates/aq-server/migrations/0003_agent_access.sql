-- Frozen trusted launch authority. Credentials contain no durable bearer secret.
CREATE TABLE agent_launch_bindings (
 attempt_id UUID PRIMARY KEY REFERENCES execution_attempts(attempt_id),
 node_id UUID NOT NULL REFERENCES nodes(id),
 binding JSONB NOT NULL,
 profile JSONB NOT NULL,
 context JSONB NOT NULL,
 revoked BOOLEAN NOT NULL DEFAULT FALSE,
 credential_version BIGINT NOT NULL DEFAULT 0,
 credential_verifier TEXT UNIQUE,
 credential_expires_at_ms BIGINT,
 authority_revision BIGINT NOT NULL DEFAULT 1
);
CREATE TABLE agent_read_cursors (
 verifier TEXT PRIMARY KEY,
 attempt_id UUID NOT NULL REFERENCES agent_launch_bindings(attempt_id),
 credential_version BIGINT NOT NULL,
 authority_revision BIGINT NOT NULL,
 query_digest TEXT NOT NULL,
 upstream_cursor TEXT NOT NULL,
 expires_at_ms BIGINT NOT NULL
);
CREATE INDEX agent_read_cursor_expiry ON agent_read_cursors(expires_at_ms);
UPDATE meta SET value = '3' WHERE key = 'state_format_version';
