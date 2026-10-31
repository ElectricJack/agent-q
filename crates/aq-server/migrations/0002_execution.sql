CREATE TABLE execution_nodes (
    node_id UUID PRIMARY KEY REFERENCES nodes(id),
    state JSONB NOT NULL
);

CREATE TABLE execution_attempts (
    attempt_id UUID PRIMARY KEY,
    task_id UUID NOT NULL,
    node_id UUID NOT NULL REFERENCES nodes(id),
    offer_id UUID NOT NULL UNIQUE,
    session_id UUID NOT NULL UNIQUE,
    holds_task BOOLEAN NOT NULL DEFAULT TRUE,
    state JSONB NOT NULL
);
CREATE UNIQUE INDEX execution_one_unsettled_task ON execution_attempts(task_id) WHERE holds_task;
CREATE INDEX execution_node_attempts ON execution_attempts(node_id, attempt_id);

CREATE TABLE execution_receipts (
    node_id UUID NOT NULL REFERENCES nodes(id),
    operation_id UUID NOT NULL,
    kind TEXT NOT NULL,
    payload_digest TEXT NOT NULL,
    response JSONB NOT NULL,
    PRIMARY KEY(node_id, operation_id)
);

-- Immutable report bodies survive corrections; execution_attempts is only the
-- current projection. Inserted atomically with its idempotency receipt.
CREATE TABLE execution_reports (
    node_id UUID NOT NULL REFERENCES nodes(id),
    report_id UUID NOT NULL,
    attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
    request JSONB NOT NULL,
    response JSONB NOT NULL,
    PRIMARY KEY(node_id, report_id)
);
CREATE INDEX execution_attempt_reports ON execution_reports(attempt_id);

CREATE TABLE execution_directives (
    directive_id UUID PRIMARY KEY,
    node_id UUID NOT NULL REFERENCES nodes(id),
    attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
    acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    state JSONB NOT NULL
);
CREATE INDEX execution_pending_directives ON execution_directives(node_id, directive_id) WHERE NOT acknowledged;

UPDATE meta SET value = '2' WHERE key = 'state_format_version';
