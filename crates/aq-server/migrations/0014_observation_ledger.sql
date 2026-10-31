-- AQ22 draft: exercised explicitly until ordered migrations012/013 land.
-- This metadata ledger stores references, not transcript bodies or provider secrets.
CREATE TABLE observation_nodes (
 node_id UUID PRIMARY KEY REFERENCES nodes(id),
 through_sequence BIGINT NOT NULL DEFAULT 0 CHECK(through_sequence >= 0),
 retained_bytes BIGINT NOT NULL DEFAULT 0 CHECK(retained_bytes BETWEEN 0 AND 134217728),
 event_count BIGINT NOT NULL DEFAULT 0 CHECK(event_count BETWEEN 0 AND 65536),
 source_count INTEGER NOT NULL DEFAULT 0 CHECK(source_count BETWEEN 0 AND 256)
);
CREATE TABLE observation_sources (
 node_id UUID NOT NULL REFERENCES observation_nodes(node_id),
 source_id UUID NOT NULL,
 generation BIGINT NOT NULL CHECK(generation > 0),
 attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
 authority JSONB NOT NULL CHECK(octet_length(authority::text) <= 65536),
 authority_digest TEXT NOT NULL CHECK(length(authority_digest)=64),
 PRIMARY KEY(node_id,source_id,generation)
);
CREATE TABLE observation_packets (
 node_id UUID NOT NULL REFERENCES observation_nodes(node_id),
 message_id UUID NOT NULL,
 body_digest TEXT NOT NULL CHECK(length(body_digest)=64),
 receipt JSONB NOT NULL CHECK(octet_length(receipt::text) <= 2048),
 PRIMARY KEY(node_id,message_id)
);
CREATE TABLE observation_events (
 event_id UUID PRIMARY KEY,
 node_id UUID NOT NULL REFERENCES observation_nodes(node_id),
 sequence BIGINT NOT NULL CHECK(sequence > 0),
 lineage_id UUID NOT NULL,
 attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
 recorded_at_ms BIGINT NOT NULL CHECK(recorded_at_ms>0),
 record JSONB NOT NULL CHECK(octet_length(record::text) <= 98304),
 decision JSONB NOT NULL CHECK(octet_length(decision::text) <= 98304),
 UNIQUE(node_id,sequence)
);
CREATE INDEX observation_lineage_events ON observation_events(lineage_id,node_id,sequence);
CREATE INDEX observation_attempt_events ON observation_events(attempt_id,node_id,sequence);
CREATE TABLE observation_call_heads (
 call_key TEXT PRIMARY KEY CHECK(length(call_key)=64),
 fact JSONB CHECK(octet_length(fact::text) <= 98304)
);
CREATE TABLE observation_facts (
 event_id UUID PRIMARY KEY REFERENCES observation_events(event_id),
 call_key TEXT NOT NULL REFERENCES observation_call_heads(call_key),
 fact JSONB NOT NULL CHECK(octet_length(fact::text) <= 98304)
);
CREATE TABLE observation_projections (
 lineage_id UUID PRIMARY KEY,
 revision BIGINT NOT NULL DEFAULT 0 CHECK(revision>=0),
 last_recorded_at_ms BIGINT NOT NULL DEFAULT 0 CHECK(last_recorded_at_ms>=0),
 projection JSONB NOT NULL CHECK(octet_length(projection::text)<=4096)
);
CREATE FUNCTION refuse_observation_history_mutation() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'observation history is immutable'; END $$;
CREATE TRIGGER observation_source_immutable BEFORE UPDATE OR DELETE ON observation_sources
 FOR EACH ROW EXECUTE FUNCTION refuse_observation_history_mutation();
CREATE TRIGGER observation_packet_immutable BEFORE UPDATE OR DELETE ON observation_packets
 FOR EACH ROW EXECUTE FUNCTION refuse_observation_history_mutation();
CREATE TRIGGER observation_event_immutable BEFORE UPDATE OR DELETE ON observation_events
 FOR EACH ROW EXECUTE FUNCTION refuse_observation_history_mutation();
CREATE TRIGGER observation_fact_immutable BEFORE UPDATE OR DELETE ON observation_facts
 FOR EACH ROW EXECUTE FUNCTION refuse_observation_history_mutation();
-- No retention deletion/namespace reuse is authorized by this checkpoint.
-- Ordered registration will update state_format_version when012/013 are present.
