-- AQ owns project execution policy, never graph task state or private Git metadata.
CREATE TABLE project_execution_bindings (
 project_id UUID PRIMARY KEY,
 revision BIGINT NOT NULL CHECK(revision > 0),
 binding JSONB NOT NULL,
 validated_at_ms BIGINT NOT NULL CHECK(validated_at_ms > 0),
 binding_observation JSONB NOT NULL
);
CREATE TABLE project_execution_binding_history (
 project_id UUID NOT NULL REFERENCES project_execution_bindings(project_id),
 revision BIGINT NOT NULL CHECK(revision > 0),
 record JSONB NOT NULL,
 actor_id UUID NOT NULL REFERENCES users(id),
 created_at_ms BIGINT NOT NULL,
 PRIMARY KEY(project_id,revision)
);
CREATE TABLE project_execution_operations (
 actor_id UUID NOT NULL REFERENCES users(id),
 operation_id UUID NOT NULL,
 payload_digest TEXT NOT NULL CHECK(payload_digest ~ '^[0-9a-f]{64}$'),
 response JSONB NOT NULL,
 PRIMARY KEY(actor_id,operation_id)
);
UPDATE meta SET value='8' WHERE key='state_format_version';
