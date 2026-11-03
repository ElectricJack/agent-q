-- Verifier-only immutable issuance. Original records remain available for
-- operation replay; revocation is monotonic and cannot be undone by replay.
CREATE TABLE delegated_permits (
 permit_id UUID PRIMARY KEY CHECK(permit_id<>'00000000-0000-0000-0000-000000000000'::UUID),
 node_id UUID NOT NULL REFERENCES nodes(id),
 attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
 operation_id UUID NOT NULL CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'::UUID),
 request_digest BYTEA NOT NULL CHECK(octet_length(request_digest)=32),
 record JSONB NOT NULL CHECK(octet_length(record::TEXT)<=8192),
 revoked BOOLEAN NOT NULL DEFAULT FALSE,
 UNIQUE(node_id,operation_id),
 CHECK(record->>'permit_id'=permit_id::TEXT),
 CHECK(record#>>'{binding,node,node_id}'=node_id::TEXT),
 CHECK(record#>>'{binding,attempt_id}'=attempt_id::TEXT),
 CHECK(NOT record ? 'credential'),
 CHECK(jsonb_typeof(record->'verifier')='array' AND jsonb_array_length(record->'verifier')=32)
);
CREATE INDEX delegated_permits_attempt ON delegated_permits(attempt_id);
CREATE FUNCTION delegated_permit_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP IN ('DELETE','TRUNCATE') THEN
  RAISE EXCEPTION 'delegated issuance is retained' USING ERRCODE='23514';
 ELSIF TG_OP='UPDATE' THEN
  IF (to_jsonb(NEW)-'revoked') IS DISTINCT FROM (to_jsonb(OLD)-'revoked') OR (OLD.revoked AND NOT NEW.revoked)
  THEN RAISE EXCEPTION 'delegated issuance is immutable' USING ERRCODE='23514'; END IF;
 ELSE
  PERFORM 1 FROM nodes WHERE id=NEW.node_id AND revoked_at IS NULL FOR UPDATE;
  IF NOT FOUND THEN RAISE EXCEPTION 'delegated node unavailable' USING ERRCODE='23514'; END IF;
  PERFORM 1 FROM execution_attempts WHERE attempt_id=NEW.attempt_id AND node_id=NEW.node_id FOR UPDATE;
  IF NOT FOUND OR EXISTS(SELECT 1 FROM profiled_cancellations WHERE attempt_id=NEW.attempt_id)
  THEN RAISE EXCEPTION 'delegated attempt unavailable' USING ERRCODE='23514'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER delegated_permit_guard BEFORE INSERT OR UPDATE OR DELETE ON delegated_permits
 FOR EACH ROW EXECUTE FUNCTION delegated_permit_guard();
CREATE TRIGGER delegated_permit_truncate_guard BEFORE TRUNCATE ON delegated_permits
 FOR EACH STATEMENT EXECUTE FUNCTION delegated_permit_guard();
CREATE FUNCTION delegated_node_revoke() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.revoked_at IS NOT NULL THEN
  UPDATE delegated_permits SET revoked=TRUE WHERE node_id=NEW.id AND NOT revoked;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER delegated_node_revoke AFTER UPDATE OF revoked_at ON nodes
 FOR EACH ROW EXECUTE FUNCTION delegated_node_revoke();
CREATE FUNCTION delegated_attempt_revoke() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 UPDATE delegated_permits SET revoked=TRUE WHERE attempt_id=NEW.attempt_id AND NOT revoked;
 RETURN NEW;
END $$;
CREATE TRIGGER delegated_attempt_revoke AFTER INSERT ON profiled_cancellations
 FOR EACH ROW EXECUTE FUNCTION delegated_attempt_revoke();
UPDATE meta SET value='45' WHERE key='state_format_version';
