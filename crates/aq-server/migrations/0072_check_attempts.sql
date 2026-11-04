-- D6 follow-up: the server->node check-attempt execution kind backing the
-- Host7 CheckExecutor. One row per admitted check.request operation, bound to
-- the pinned profiled result and its retained source, and assigned to the
-- node that ran that attempt. `stop` is the node-reported stop, written once;
-- profiled_check_evidence is derived only from it, never from the host.
CREATE TABLE check_attempts (
 operation_id uuid PRIMARY KEY,
 workspace_id uuid NOT NULL,
 project_id uuid NOT NULL,
 node_id uuid NOT NULL REFERENCES nodes(id),
 attempt_id uuid NOT NULL REFERENCES execution_attempts(attempt_id),
 report_id uuid NOT NULL REFERENCES profiled_result_reports(report_id),
 source_operation_id uuid NOT NULL,
 subject_id text NOT NULL CHECK (octet_length(subject_id) BETWEEN 1 AND 4096),
 subject_revision bigint NOT NULL CHECK (subject_revision >= 0),
 check_id text NOT NULL CHECK (octet_length(check_id) BETWEEN 1 AND 256),
 validator jsonb NOT NULL CHECK (octet_length(validator::text) <= 4096),
 source_format text NOT NULL CHECK (source_format IN ('sha1','sha256')),
 source_hex text NOT NULL CHECK (source_hex ~ '^([0-9a-f]{40}|[0-9a-f]{64})$'),
 execution_profile text NOT NULL CHECK (execution_profile = 'aq.check.sandbox/v1'),
 request_digest text NOT NULL CHECK (request_digest ~ '^[0-9a-f]{64}$'),
 state text NOT NULL CHECK (state IN ('queued','running','stopped')),
 stop jsonb CHECK (stop IS NULL OR octet_length(stop::text) <= 16384),
 created_at_ms bigint NOT NULL CHECK (created_at_ms > 0),
 updated_at_ms bigint NOT NULL CHECK (updated_at_ms > 0),
 CHECK ((state = 'stopped') = (stop IS NOT NULL))
);
CREATE INDEX check_attempts_node_queue ON check_attempts(node_id,created_at_ms) WHERE state <> 'stopped';
-- The request identity never changes and a stopped attempt is final.
CREATE FUNCTION check_attempt_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP = 'DELETE' OR OLD.state = 'stopped'
  OR (NEW.operation_id,NEW.node_id,NEW.attempt_id,NEW.report_id,NEW.source_operation_id,NEW.check_id,NEW.validator,NEW.request_digest)
   IS DISTINCT FROM (OLD.operation_id,OLD.node_id,OLD.attempt_id,OLD.report_id,OLD.source_operation_id,OLD.check_id,OLD.validator,OLD.request_digest)
 THEN
  RAISE EXCEPTION 'check attempt identity and stop are immutable';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER check_attempt_immutable BEFORE UPDATE OR DELETE ON check_attempts
 FOR EACH ROW EXECUTE FUNCTION check_attempt_guard();
UPDATE meta SET value='72' WHERE key='state_format_version';
