-- Original semantic registration is immutable history, not mutable inventory.
CREATE TABLE profiled_native_registration_history (
 registration_id UUID PRIMARY KEY,
 node_id UUID NOT NULL REFERENCES nodes(id),
 boot_id UUID NOT NULL,
 identity_digest TEXT NOT NULL CHECK(identity_digest ~ '^[0-9a-f]{64}$'),
 semantic_digest TEXT NOT NULL CHECK(semantic_digest ~ '^[0-9a-f]{64}$'),
 metadata_bytes BYTEA NOT NULL CHECK(octet_length(metadata_bytes) BETWEEN 1 AND 65536),
 metadata_digest TEXT GENERATED ALWAYS AS (encode(sha256(metadata_bytes),'hex')) STORED,
 UNIQUE(node_id,boot_id,identity_digest)
);
CREATE TABLE profiled_launch_registrations (
 attempt_id UUID PRIMARY KEY REFERENCES execution_attempts(attempt_id),
 registration_id UUID NOT NULL REFERENCES profiled_native_registration_history(registration_id),
 node_id UUID NOT NULL REFERENCES nodes(id),
 boot_id UUID NOT NULL,
 logical_bytes BIGINT NOT NULL DEFAULT 64 CHECK(logical_bytes=64)
);
CREATE TABLE profiled_launch_closures (
 attempt_id UUID PRIMARY KEY REFERENCES execution_attempts(attempt_id),
 node_id UUID NOT NULL REFERENCES nodes(id),
 operation_id UUID NOT NULL,
 binding JSONB NOT NULL CHECK(octet_length(binding::text)<=8192),
 grant_id UUID NOT NULL,
 initial_prepare_operation_id UUID NOT NULL,
 registration_id UUID NOT NULL REFERENCES profiled_native_registration_history(registration_id),
 evidence_kind TEXT NOT NULL CHECK(evidence_kind IN ('suppressed','host_stopped')),
 verifier JSONB NOT NULL CHECK(octet_length(verifier::text)<=4096),
 terminal_revision BIGINT NOT NULL CHECK(terminal_revision>0),
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 request_bytes BYTEA NOT NULL CHECK(octet_length(request_bytes) BETWEEN 1 AND 32768),
 context_bytes BYTEA NOT NULL CHECK(octet_length(context_bytes) BETWEEN 1 AND 65536),
 receipt_bytes BYTEA NOT NULL CHECK(octet_length(receipt_bytes) BETWEEN 1 AND 16384),
 request_bytes_digest TEXT GENERATED ALWAYS AS (encode(sha256(request_bytes),'hex')) STORED,
 context_digest TEXT GENERATED ALWAYS AS (encode(sha256(context_bytes),'hex')) STORED,
 receipt_digest TEXT GENERATED ALWAYS AS (encode(sha256(receipt_bytes),'hex')) STORED,
 recorded_at_ms BIGINT NOT NULL CHECK(recorded_at_ms>0),
 authority_revision BIGINT NOT NULL CHECK(authority_revision>0),
 UNIQUE(node_id,operation_id),
 FOREIGN KEY(node_id,initial_prepare_operation_id) REFERENCES profiled_execution_receipts(node_id,operation_id)
);
CREATE INDEX profiled_registration_history_node ON profiled_native_registration_history(node_id,boot_id);
CREATE INDEX profiled_launch_registration_node ON profiled_launch_registrations(node_id);
CREATE INDEX profiled_launch_closure_node ON profiled_launch_closures(node_id);
CREATE FUNCTION refuse_profiled_closure_history_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'immutable profiled launch closure history'; END $$;
CREATE TRIGGER profiled_registration_history_immutable BEFORE UPDATE OR DELETE ON profiled_native_registration_history FOR EACH ROW EXECUTE FUNCTION refuse_profiled_closure_history_change();
CREATE TRIGGER profiled_launch_registration_immutable BEFORE UPDATE OR DELETE ON profiled_launch_registrations FOR EACH ROW EXECUTE FUNCTION refuse_profiled_closure_history_change();
CREATE TRIGGER profiled_launch_closure_immutable BEFORE UPDATE OR DELETE ON profiled_launch_closures FOR EACH ROW EXECUTE FUNCTION refuse_profiled_closure_history_change();
CREATE FUNCTION profiled_registration_history_quota() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE n BIGINT; b BIGINT; per_n BIGINT; per_b BIGINT;
BEGIN
 IF current_setting('transaction_isolation') NOT IN ('read committed','serializable') THEN RAISE EXCEPTION 'history mutation isolation'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('aq-native-registration-history-v1',0));
 SELECT count(*),COALESCE(sum(octet_length(metadata_bytes)),0),count(*) FILTER(WHERE node_id=NEW.node_id),COALESCE(sum(octet_length(metadata_bytes)) FILTER(WHERE node_id=NEW.node_id),0) INTO n,b,per_n,per_b FROM profiled_native_registration_history;
 IF n>=4096 OR b+octet_length(NEW.metadata_bytes)>67108864 OR per_n>=1024 OR per_b+octet_length(NEW.metadata_bytes)>16777216 THEN RAISE EXCEPTION 'registration history quota'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_registration_history_quota BEFORE INSERT ON profiled_native_registration_history FOR EACH ROW EXECUTE FUNCTION profiled_registration_history_quota();
CREATE FUNCTION profiled_launch_registration_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE h profiled_native_registration_history%ROWTYPE; l profiled_launches%ROWTYPE; a execution_attempts%ROWTYPE; n BIGINT; b BIGINT; per_n BIGINT; per_b BIGINT;
BEGIN
 SELECT * INTO STRICT h FROM profiled_native_registration_history WHERE registration_id=NEW.registration_id;
 SELECT * INTO STRICT l FROM profiled_launches WHERE attempt_id=NEW.attempt_id;
 SELECT * INTO STRICT a FROM execution_attempts WHERE attempt_id=NEW.attempt_id;
 IF h.node_id<>NEW.node_id OR h.boot_id<>NEW.boot_id OR a.node_id<>NEW.node_id OR a.kind<>'profiled_v1' OR NOT a.holds_task OR l.launch_binding#>>'{session,node_id}' IS DISTINCT FROM NEW.node_id::TEXT OR l.launch_binding#>>'{session,boot_id}' IS DISTINCT FROM NEW.boot_id::TEXT THEN RAISE EXCEPTION 'attempt registration binding'; END IF;
 IF (SELECT state#>>'{native,semantic_digest}' FROM execution_nodes WHERE node_id=NEW.node_id) IS DISTINCT FROM h.semantic_digest OR (SELECT state#>'{native,qualification}' FROM execution_nodes WHERE node_id=NEW.node_id) IS DISTINCT FROM (convert_from(h.metadata_bytes,'UTF8')::JSONB)->'qualification' THEN RAISE EXCEPTION 'attempt registration is not actual held metadata'; END IF;
 IF current_setting('transaction_isolation') NOT IN ('read committed','serializable') THEN RAISE EXCEPTION 'link mutation isolation'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('aq-native-attempt-registration-link-v1',0));
 SELECT count(*),COALESCE(sum(logical_bytes),0),count(*) FILTER(WHERE node_id=NEW.node_id),COALESCE(sum(logical_bytes) FILTER(WHERE node_id=NEW.node_id),0) INTO n,b,per_n,per_b FROM profiled_launch_registrations;
 IF n>=4096 OR b+64>262144 OR per_n>=256 OR per_b+64>16384 THEN RAISE EXCEPTION 'attempt registration link quota'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_launch_registration_guard BEFORE INSERT ON profiled_launch_registrations FOR EACH ROW EXECUTE FUNCTION profiled_launch_registration_guard();
CREATE FUNCTION profiled_launch_closure_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE a execution_attempts%ROWTYPE; h profiled_authority_hot%ROWTYPE; history profiled_native_registration_history%ROWTYPE; native JSONB; original JSONB; req JSONB; receipt JSONB; n BIGINT; b BIGINT; per_n BIGINT; per_b BIGINT; incoming BIGINT;
BEGIN
 SELECT * INTO STRICT a FROM execution_attempts WHERE attempt_id=NEW.attempt_id;
 SELECT * INTO STRICT h FROM profiled_authority_hot WHERE attempt_id=NEW.attempt_id;
 SELECT response INTO STRICT original FROM profiled_execution_receipts WHERE node_id=NEW.node_id AND operation_id=NEW.initial_prepare_operation_id AND kind='prepare';
 SELECT * INTO STRICT history FROM profiled_native_registration_history WHERE registration_id=NEW.registration_id; native:=convert_from(history.metadata_bytes,'UTF8')::JSONB;
 req:=convert_from(NEW.request_bytes,'UTF8')::JSONB; receipt:=convert_from(NEW.receipt_bytes,'UTF8')::JSONB;
 IF a.node_id<>NEW.node_id OR NOT a.holds_task OR a.kind<>'profiled_v1' OR EXISTS(SELECT 1 FROM profiled_started_receipts WHERE attempt_id=NEW.attempt_id)
 OR h.state#>>'{stop,kind}' IS DISTINCT FROM 'unknown' OR h.state->>'revoked' IS DISTINCT FROM 'true' OR h.state->>'status'='granted' OR h.state#>>'{grant,grant_id}' IS DISTINCT FROM NEW.grant_id::TEXT OR h.state#>'{grant,binding}' IS DISTINCT FROM NEW.binding
 OR original->>'kind' IS DISTINCT FROM 'grant' OR original#>'{body,grant,binding}' IS DISTINCT FROM NEW.binding OR original#>>'{body,grant,grant_id}' IS DISTINCT FROM NEW.grant_id::TEXT
 OR NOT EXISTS(SELECT 1 FROM profiled_launch_registrations WHERE attempt_id=NEW.attempt_id AND registration_id=NEW.registration_id AND node_id=NEW.node_id)
 OR history.node_id<>NEW.node_id OR history.boot_id::TEXT IS DISTINCT FROM NEW.binding#>>'{session,boot_id}' OR native#>'{request,runtime}' IS DISTINCT FROM req->'runtime' OR native->'qualification' IS DISTINCT FROM req->'qualification' OR NEW.verifier->'runtime' IS DISTINCT FROM req->'runtime' OR NEW.verifier->'qualification' IS DISTINCT FROM req->'qualification' OR (NEW.verifier->'kinds' ? NEW.evidence_kind) IS DISTINCT FROM true
 OR req->'binding' IS DISTINCT FROM NEW.binding OR req->>'operation_id' IS DISTINCT FROM NEW.operation_id::TEXT OR req->>'grant_id' IS DISTINCT FROM NEW.grant_id::TEXT OR req#>>'{journal,revision}' IS DISTINCT FROM NEW.terminal_revision::TEXT OR req#>>'{evidence,kind}' IS DISTINCT FROM NEW.evidence_kind
 OR receipt->'binding' IS DISTINCT FROM NEW.binding OR receipt->>'operation_id' IS DISTINCT FROM NEW.operation_id::TEXT OR receipt->>'grant_id' IS DISTINCT FROM NEW.grant_id::TEXT OR receipt->>'request_digest' IS DISTINCT FROM NEW.request_digest OR receipt->>'disposition' IS DISTINCT FROM 'launch_closed_awaiting_settlement' OR receipt->>'recorded_at_ms' IS DISTINCT FROM NEW.recorded_at_ms::TEXT OR receipt->>'authority_revision_at_record' IS DISTINCT FROM NEW.authority_revision::TEXT
 THEN RAISE EXCEPTION 'exact granted closure owner required'; END IF;
 IF current_setting('transaction_isolation')<>'serializable' THEN RAISE EXCEPTION 'closure mutation isolation'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('aq-profiled-launch-closure-quota-v1',0));
 SELECT count(*),COALESCE(sum(octet_length(request_bytes)+octet_length(context_bytes)+octet_length(receipt_bytes)+octet_length(binding::text)+octet_length(verifier::text)),0),count(*) FILTER(WHERE node_id=NEW.node_id),COALESCE(sum(octet_length(request_bytes)+octet_length(context_bytes)+octet_length(receipt_bytes)+octet_length(binding::text)+octet_length(verifier::text)) FILTER(WHERE node_id=NEW.node_id),0) INTO n,b,per_n,per_b FROM profiled_launch_closures;
 incoming:=octet_length(NEW.request_bytes)+octet_length(NEW.context_bytes)+octet_length(NEW.receipt_bytes)+octet_length(NEW.binding::text)+octet_length(NEW.verifier::text);
 IF n>=4096 OR b+incoming>67108864 OR per_n>=256 OR per_b+incoming>4194304 THEN RAISE EXCEPTION 'launch closure history quota'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_launch_closure_guard BEFORE INSERT ON profiled_launch_closures FOR EACH ROW EXECUTE FUNCTION profiled_launch_closure_guard();
CREATE FUNCTION profiled_launch_closed_hot_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM profiled_launch_closures WHERE attempt_id=NEW.attempt_id) AND (NEW.state->>'status'='granted' OR NEW.state->>'revoked' IS DISTINCT FROM 'true' OR NEW.state->'grant' IS DISTINCT FROM OLD.state->'grant' OR NEW.state->'owner' IS DISTINCT FROM OLD.state->'owner') THEN RAISE EXCEPTION 'terminal launch closure cannot reopen'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_launch_closed_hot_guard BEFORE UPDATE ON profiled_authority_hot FOR EACH ROW EXECUTE FUNCTION profiled_launch_closed_hot_guard();
CREATE FUNCTION profiled_launch_closed_started_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM profiled_launch_closures WHERE attempt_id=NEW.attempt_id) THEN RAISE EXCEPTION 'closed launch cannot admit Started'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_launch_closed_started_guard BEFORE INSERT ON profiled_started_receipts FOR EACH ROW EXECUTE FUNCTION profiled_launch_closed_started_guard();
CREATE FUNCTION profiled_launch_closed_credential_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM profiled_launch_closures WHERE attempt_id=NEW.attempt_id) AND (NOT NEW.revoked OR NEW.credential_verifier IS NOT NULL OR NEW.credential_expires_at_ms IS NOT NULL) THEN RAISE EXCEPTION 'closed launch cannot issue credentials'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_launch_closed_credential_guard BEFORE INSERT OR UPDATE ON agent_launch_bindings FOR EACH ROW EXECUTE FUNCTION profiled_launch_closed_credential_guard();
UPDATE meta SET value='28' WHERE key='state_format_version';
