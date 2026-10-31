-- Immutable authorized intent, independent of agent credential lifetime.
CREATE TABLE agent_graph_operations (
 operation_id UUID PRIMARY KEY,
 operation_seq BIGSERIAL NOT NULL UNIQUE,
 attempt_id UUID NOT NULL REFERENCES agent_launch_bindings(attempt_id),
 node_id UUID NOT NULL REFERENCES nodes(id),
 request JSONB NOT NULL,
 payload_digest TEXT NOT NULL,
 mapped JSONB NOT NULL,
 policy JSONB NOT NULL,
 status JSONB NOT NULL,
 phase TEXT NOT NULL CHECK(phase IN ('queued','dispatching','unknown','confirmed','conflict','rejected')),
 created_at_ms BIGINT NOT NULL,
 dispatch_count INTEGER NOT NULL DEFAULT 0 CHECK(dispatch_count BETWEEN 0 AND 8),
 first_dispatch_at_ms BIGINT,
 next_retry_at_ms BIGINT NOT NULL DEFAULT 0,
 lease_id UUID,
 lease_expires_at_ms BIGINT,
 retained_bytes BIGINT NOT NULL CHECK(retained_bytes>=0),
 reserved_creations INTEGER NOT NULL DEFAULT 0 CHECK(reserved_creations>=0)
);
CREATE INDEX agent_graph_operation_attempt ON agent_graph_operations(node_id,attempt_id,operation_seq);
CREATE INDEX agent_graph_retained_attempt ON agent_graph_operations(attempt_id);
CREATE TABLE agent_graph_outbox (
 event_id UUID PRIMARY KEY,
 operation_id UUID NOT NULL UNIQUE REFERENCES agent_graph_operations(operation_id),
 kind TEXT NOT NULL CHECK(kind='graph.operation_confirmed'),
 payload JSONB NOT NULL,
 delivered BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE TABLE agent_created_records (
 attempt_id UUID NOT NULL REFERENCES agent_launch_bindings(attempt_id),
 record_id UUID NOT NULL,
 PRIMARY KEY(attempt_id,record_id)
);
CREATE TABLE agent_operation_cursors (
 verifier TEXT PRIMARY KEY,
 node_id UUID NOT NULL REFERENCES nodes(id),
 attempt_id UUID NOT NULL REFERENCES agent_launch_bindings(attempt_id),
 query_digest TEXT NOT NULL,
 after_sequence BIGINT NOT NULL,
 upper_sequence BIGINT NOT NULL,
 expires_at_ms BIGINT NOT NULL
);
UPDATE meta SET value='7' WHERE key='state_format_version';
