-- Central selection facts are independent of graph tasks and provider ledgers.
CREATE TABLE intelligence_credential_definitions (
 identity TEXT PRIMARY KEY,
 definition_digest TEXT NOT NULL,
 latest_revision BIGINT NOT NULL CHECK(latest_revision>0),
 private_state_digest TEXT NOT NULL
);
CREATE TABLE intelligence_registry (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 revision BIGINT NOT NULL CHECK(revision>0),
 config_revision BIGINT NOT NULL CHECK(config_revision>0),
 config_digest TEXT NOT NULL,
 credentials JSONB NOT NULL,
 private_fingerprints JSONB NOT NULL
);
CREATE TABLE intelligence_observations (
 scope_source_digest TEXT PRIMARY KEY,
 sequence BIGINT NOT NULL CHECK(sequence>0),
 observation JSONB NOT NULL
);
CREATE TABLE intelligence_operations (
 actor_id UUID NOT NULL REFERENCES users(id),
 operation_id UUID NOT NULL,
 payload_digest TEXT NOT NULL,
 response JSONB NOT NULL,
 PRIMARY KEY(actor_id,operation_id)
);
CREATE TABLE intelligence_node_snapshots (
 node_id UUID PRIMARY KEY REFERENCES nodes(id),
 boot_id UUID NOT NULL,
 sequence BIGINT NOT NULL CHECK(sequence>0),
 revision BIGINT NOT NULL CHECK(revision>0),
 snapshot JSONB NOT NULL
);
CREATE TABLE intelligence_decisions (
 decision_id UUID PRIMARY KEY,
 actor_id UUID NOT NULL REFERENCES users(id),
 operation_id UUID NOT NULL,
 payload_digest TEXT NOT NULL,
 decision JSONB NOT NULL,
 UNIQUE(actor_id,operation_id)
);
-- Only actual future profiled admission may seal this exact decision to an
-- attempt. No administrative route can attach a selection to fixture attempts.
CREATE TABLE intelligence_attempt_selections (
 attempt_id UUID PRIMARY KEY REFERENCES execution_attempts(attempt_id),
 selection_digest TEXT NOT NULL,
 selection JSONB NOT NULL
);
UPDATE meta SET value='9' WHERE key='state_format_version';
