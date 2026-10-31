-- Launch transition journal heads are actual owner records, not request hashes.
CREATE TABLE profiled_owner_journals (
 attempt_id UUID PRIMARY KEY REFERENCES profiled_authority_hot(attempt_id),
 binding JSONB NOT NULL CHECK(octet_length(binding::text)<=4096),
 guardian_id UUID NOT NULL,
 launch_id UUID NOT NULL,
 reservation_id UUID NOT NULL,
 phase TEXT NOT NULL CHECK(phase IN ('reserved','prepared','started')),
 grant_id UUID,
 revision BIGINT NOT NULL CHECK(revision>0),
 digest TEXT NOT NULL CHECK(digest ~ '^[0-9a-f]{64}$'),
 CHECK((phase='reserved' AND grant_id IS NULL) OR (phase<>'reserved' AND grant_id IS NOT NULL))
);
CREATE FUNCTION profiled_owner_journal_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE h profiled_authority_hot%ROWTYPE; b JSONB; held BOOLEAN;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'retained owner journal head'; END IF;
 SELECT hot.state,l.launch_binding,a.holds_task INTO h.state,b,held FROM profiled_authority_hot hot
 JOIN profiled_launches l USING(attempt_id) JOIN execution_attempts a USING(attempt_id)
 WHERE hot.attempt_id=NEW.attempt_id;
 IF h.state IS NULL OR NEW.binding IS DISTINCT FROM b OR NOT held
 OR NEW.guardian_id::TEXT IS DISTINCT FROM h.state#>>'{owner,guardian_id}'
 OR NEW.launch_id::TEXT IS DISTINCT FROM h.state#>>'{owner,launch_id}'
 OR NEW.reservation_id::TEXT IS DISTINCT FROM h.state#>>'{owner,reservation_id}'
 OR h.state#>>'{stop,kind}' IS DISTINCT FROM 'unknown'
 OR h.state->'revoked' IS DISTINCT FROM 'false'::JSONB OR h.state->'cancellation_id' IS DISTINCT FROM 'null'::JSONB
 THEN RAISE EXCEPTION 'owner journal authority mismatch'; END IF;
 IF (NEW.phase='reserved' AND (h.state->>'status'<>'preparing' OR h.state->'grant'<>'null'::JSONB))
 OR (NEW.phase<>'reserved' AND (h.state->>'status'<>'granted' OR NEW.grant_id::TEXT IS DISTINCT FROM h.state#>>'{grant,grant_id}'))
 THEN RAISE EXCEPTION 'owner journal grant/phase mismatch'; END IF;
 IF TG_OP='INSERT' THEN
   IF NEW.phase<>'reserved' THEN RAISE EXCEPTION 'journal requires reserved origin'; END IF;
 ELSE
   IF ROW(NEW.attempt_id,NEW.binding,NEW.guardian_id,NEW.launch_id,NEW.reservation_id)
      IS DISTINCT FROM ROW(OLD.attempt_id,OLD.binding,OLD.guardian_id,OLD.launch_id,OLD.reservation_id)
      OR NEW.revision<=OLD.revision
      OR NOT ((OLD.phase='reserved' AND NEW.phase='prepared') OR(OLD.phase='prepared' AND NEW.phase='started'))
      OR (OLD.grant_id IS NOT NULL AND NEW.grant_id IS DISTINCT FROM OLD.grant_id)
   THEN RAISE EXCEPTION 'owner journal cannot rewrite scope, regress, or skip a phase'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_owner_journal_guard BEFORE INSERT OR UPDATE OR DELETE ON profiled_owner_journals
 FOR EACH ROW EXECUTE FUNCTION profiled_owner_journal_guard();

CREATE TABLE profiled_started_receipts (
 attempt_id UUID PRIMARY KEY REFERENCES profiled_owner_journals(attempt_id),
 node_id UUID NOT NULL REFERENCES nodes(id),
 operation_id UUID NOT NULL,
 request_bytes BYTEA NOT NULL CHECK(octet_length(request_bytes) BETWEEN 1 AND 16384),
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$' AND request_digest=encode(sha256(request_bytes),'hex')),
 receipt_bytes BYTEA NOT NULL CHECK(octet_length(receipt_bytes) BETWEEN 1 AND 16384),
 before_record_revision BIGINT NOT NULL CHECK(before_record_revision>0),
 UNIQUE(node_id,operation_id)
);
CREATE TRIGGER profiled_started_immutable BEFORE UPDATE OR DELETE ON profiled_started_receipts
 FOR EACH ROW EXECUTE FUNCTION profiled_sealed_row_guard();
CREATE FUNCTION profiled_started_insert_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE j profiled_owner_journals%ROWTYPE; h profiled_authority_hot%ROWTYPE;
 q JSONB; r JSONB; deadline BIGINT;
BEGIN
 SELECT * INTO STRICT j FROM profiled_owner_journals WHERE attempt_id=NEW.attempt_id;
 SELECT * INTO STRICT h FROM profiled_authority_hot WHERE attempt_id=NEW.attempt_id;
 q:=convert_from(NEW.request_bytes,'UTF8')::JSONB; r:=convert_from(NEW.receipt_bytes,'UTF8')::JSONB;
 deadline:=(h.state#>>'{grant,expires_at_ms}')::BIGINT;
 IF j.phase<>'prepared' OR NEW.before_record_revision<>h.record_revision
 OR h.state->>'status'<>'granted' OR h.state#>>'{stop,kind}'<>'unknown'
 OR h.state->'revoked'<>'false'::JSONB OR h.state->'cancellation_id'<>'null'::JSONB
 OR q->>'schema_version' IS DISTINCT FROM '1' OR r->>'schema_version' IS DISTINCT FROM '1'
 OR q->'binding' IS DISTINCT FROM j.binding OR r->'binding' IS DISTINCT FROM j.binding
 OR q#>>'{binding,session,node_id}' IS DISTINCT FROM NEW.node_id::TEXT
 OR q->>'operation_id' IS DISTINCT FROM NEW.operation_id::TEXT OR r->>'operation_id' IS DISTINCT FROM NEW.operation_id::TEXT
 OR q->>'guardian_id' IS DISTINCT FROM j.guardian_id::TEXT OR q->>'launch_id' IS DISTINCT FROM j.launch_id::TEXT
 OR q->>'reservation_id' IS DISTINCT FROM j.reservation_id::TEXT
 OR q->>'grant_id' IS DISTINCT FROM j.grant_id::TEXT OR r->>'grant_id' IS DISTINCT FROM j.grant_id::TEXT
 OR q->>'grant_id' IS DISTINCT FROM h.state#>>'{grant,grant_id}'
 OR COALESCE((q->>'journal_revision')::BIGINT,0)<=j.revision
 OR r->>'request_digest' IS DISTINCT FROM NEW.request_digest
 OR COALESCE((r->>'recorded_at_ms')::BIGINT,0)<=0
 OR (r->>'recorded_at_ms')::BIGINT>=deadline
 OR (r->>'recorded_at_ms')::BIGINT>=(h.state->>'execution_deadline_ms')::BIGINT
 OR (r->>'authority_revision_at_record')::BIGINT IS DISTINCT FROM h.authority_revision+1
 THEN RAISE EXCEPTION 'started record requires exact live prepared owner'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_started_insert_guard BEFORE INSERT ON profiled_started_receipts
 FOR EACH ROW EXECUTE FUNCTION profiled_started_insert_guard();
CREATE FUNCTION profiled_started_atomic_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE h profiled_authority_hot%ROWTYPE; j profiled_owner_journals%ROWTYPE; q JSONB; r JSONB;
BEGIN
 SELECT * INTO STRICT h FROM profiled_authority_hot WHERE attempt_id=NEW.attempt_id;
 SELECT * INTO STRICT j FROM profiled_owner_journals WHERE attempt_id=NEW.attempt_id;
 q:=convert_from(NEW.request_bytes,'UTF8')::JSONB; r:=convert_from(NEW.receipt_bytes,'UTF8')::JSONB;
 IF h.record_revision<>NEW.before_record_revision+1 OR h.state#>>'{stop,kind}'<>'running'
 OR h.authority_revision<>(r->>'authority_revision_at_record')::BIGINT
 OR j.phase<>'started' OR j.revision<>(q->>'journal_revision')::BIGINT OR j.digest IS DISTINCT FROM q->>'journal_digest'
 THEN RAISE EXCEPTION 'started receipt, journal and hot transition must commit together'; END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER profiled_started_atomic_guard AFTER INSERT ON profiled_started_receipts
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION profiled_started_atomic_guard();
CREATE FUNCTION profiled_started_projection_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.state#>>'{stop,kind}'='running' AND OLD.state#>>'{stop,kind}'<>'running'
 AND (EXISTS(SELECT 1 FROM profiled_owner_journals WHERE attempt_id=NEW.attempt_id)
      OR EXISTS(SELECT 1 FROM execution_attempts a JOIN execution_nodes n ON a.node_id=n.node_id WHERE a.attempt_id=NEW.attempt_id AND n.state ? 'native'))
 AND NOT EXISTS(SELECT 1 FROM profiled_started_receipts r JOIN profiled_owner_journals j USING(attempt_id)
   WHERE r.attempt_id=NEW.attempt_id AND r.before_record_revision=OLD.record_revision AND j.phase='started')
 THEN RAISE EXCEPTION 'native running projection requires original started receipt'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_started_projection_guard BEFORE UPDATE ON profiled_authority_hot
 FOR EACH ROW EXECUTE FUNCTION profiled_started_projection_guard();
-- Ordered runtime registration follows qualification of the native producer seam.
