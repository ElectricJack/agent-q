-- Actual phase-v2 initial authority. No legacy prepare/grant row is synthesized.
CREATE TABLE profiled_phase_execution_grants (
 attempt_id UUID PRIMARY KEY REFERENCES profiled_v2_accepts(attempt_id),
 node_id UUID NOT NULL REFERENCES nodes(id),
 boot_id UUID NOT NULL CHECK(boot_id<>'00000000-0000-0000-0000-000000000000'),
 operation_id UUID NOT NULL CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 grant_id UUID NOT NULL UNIQUE CHECK(grant_id<>'00000000-0000-0000-0000-000000000000'),
 registration_id UUID NOT NULL REFERENCES profiled_native_registration_history(registration_id),
 accepted_receipt_digest TEXT NOT NULL CHECK(accepted_receipt_digest ~ '^[0-9a-f]{64}$'),
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 receipt_digest TEXT NOT NULL CHECK(receipt_digest ~ '^[0-9a-f]{64}$'),
 request_bytes BYTEA NOT NULL CHECK(octet_length(request_bytes) BETWEEN 1 AND 131072),
 receipt_bytes BYTEA NOT NULL CHECK(octet_length(receipt_bytes) BETWEEN 1 AND 131072),
 provenance_bytes BYTEA NOT NULL CHECK(octet_length(provenance_bytes) BETWEEN 1 AND 131072),
 request_bytes_digest TEXT GENERATED ALWAYS AS (encode(sha256(request_bytes),'hex')) STORED,
 receipt_bytes_digest TEXT GENERATED ALWAYS AS (encode(sha256(receipt_bytes),'hex')) STORED,
 provenance_digest TEXT GENERATED ALWAYS AS (encode(sha256(provenance_bytes),'hex')) STORED,
 issued_at_ms BIGINT NOT NULL CHECK(issued_at_ms>0),
 expires_at_ms BIGINT NOT NULL CHECK(expires_at_ms>issued_at_ms),
 execution_deadline_ms BIGINT NOT NULL CHECK(execution_deadline_ms>=expires_at_ms),
 UNIQUE(node_id,operation_id)
);
CREATE INDEX profiled_phase_execution_node ON profiled_phase_execution_grants(node_id);
CREATE FUNCTION profiled_phase_execution_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE q JSONB; r JSONB; a profiled_v2_accepts%ROWTYPE;
BEGIN
 IF TG_OP<>'INSERT' THEN RAISE EXCEPTION 'immutable phase execution authority'; END IF;
 q:=convert_from(NEW.request_bytes,'UTF8')::JSONB;
 r:=convert_from(NEW.receipt_bytes,'UTF8')::JSONB;
 SELECT * INTO a FROM profiled_v2_accepts WHERE attempt_id=NEW.attempt_id;
 IF NOT FOUND OR a.node_id IS DISTINCT FROM NEW.node_id
 OR a.receipt_digest IS DISTINCT FROM NEW.accepted_receipt_digest
 OR NEW.issued_at_ms<a.issued_at_ms OR NEW.issued_at_ms>=a.expires_at_ms
 OR (q->>'schema_version')::INTEGER IS DISTINCT FROM 2
 OR (r->>'schema_version')::INTEGER IS DISTINCT FROM 2
 OR (q#>>'{original,operation_id}')::UUID IS DISTINCT FROM NEW.operation_id
 OR (r->>'operation_id')::UUID IS DISTINCT FROM NEW.operation_id
 OR (q#>>'{original,identity,node_id}')::UUID IS DISTINCT FROM NEW.node_id
 OR (q#>>'{original,identity,boot_id}')::UUID IS DISTINCT FROM NEW.boot_id
 OR (r#>>'{grant,grant_id}')::UUID IS DISTINCT FROM NEW.grant_id
 OR (r#>>'{grant,binding,attempt_id}')::UUID IS DISTINCT FROM NEW.attempt_id
 OR (r#>>'{grant,binding,session,node_id}')::UUID IS DISTINCT FROM NEW.node_id
 OR (r#>>'{grant,binding,session,boot_id}')::UUID IS DISTINCT FROM NEW.boot_id
 OR r#>'{grant,binding}' IS DISTINCT FROM q#>'{original,binding}'
 OR r->>'accepted_receipt_digest' IS DISTINCT FROM NEW.accepted_receipt_digest
 OR q->>'accepted_receipt_digest' IS DISTINCT FROM NEW.accepted_receipt_digest
 OR r->>'request_digest' IS DISTINCT FROM NEW.request_digest
 OR (r->>'issued_at_ms')::BIGINT IS DISTINCT FROM NEW.issued_at_ms
 OR (r#>>'{grant,expires_at_ms}')::BIGINT IS DISTINCT FROM NEW.expires_at_ms
 OR (r->>'execution_deadline_ms')::BIGINT IS DISTINCT FROM NEW.execution_deadline_ms
 OR NOT EXISTS(SELECT 1 FROM profiled_launch_registrations x JOIN profiled_native_registration_history h USING(registration_id) WHERE x.attempt_id=NEW.attempt_id AND x.registration_id=NEW.registration_id AND h.node_id=NEW.node_id AND h.boot_id=NEW.boot_id)
 THEN RAISE EXCEPTION 'phase execution body/accepted owner scope'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_phase_execution_immutable BEFORE INSERT OR UPDATE OR DELETE ON profiled_phase_execution_grants FOR EACH ROW EXECUTE FUNCTION profiled_phase_execution_guard();
UPDATE meta SET value='32' WHERE key='state_format_version';
