-- Real pre-offer source owner. No attempt or grant is created by reservation.
CREATE TABLE profiled_source_exports (
 operation_id UUID PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 project_id UUID NOT NULL REFERENCES project_execution_bindings(project_id),
 request_bytes BYTEA NOT NULL CHECK(octet_length(request_bytes) BETWEEN 1 AND 8192),
 request_digest TEXT NOT NULL CHECK(request_digest=encode(sha256(request_bytes),'hex')),
 reserved_bytes BIGINT NOT NULL CHECK(reserved_bytes BETWEEN 1048577 AND 1074790400),
 charged_bytes BIGINT NOT NULL CHECK(charged_bytes>0 AND charged_bytes<=reserved_bytes),
 state TEXT NOT NULL CHECK(state IN ('pending','verified')),
 verified_bytes BYTEA CHECK(octet_length(verified_bytes) BETWEEN 1 AND 4194304),
 verified_digest TEXT CHECK(verified_digest=encode(sha256(verified_bytes),'hex')),
 CHECK((state='pending' AND verified_bytes IS NULL AND verified_digest IS NULL)
    OR (state='verified' AND verified_bytes IS NOT NULL AND verified_digest IS NOT NULL))
);
CREATE INDEX profiled_source_export_project ON profiled_source_exports(project_id,operation_id);
CREATE FUNCTION profiled_source_export_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE q JSONB; v JSONB; n BIGINT; total_metadata BIGINT; project_metadata BIGINT; incoming_metadata BIGINT;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'source export owner cannot be deleted'; END IF;
 q:=convert_from(NEW.request_bytes,'UTF8')::JSONB;
 IF q->>'schema_version' IS DISTINCT FROM '1' OR q->>'operation_id' IS DISTINCT FROM NEW.operation_id::TEXT
    OR q->>'project_id' IS DISTINCT FROM NEW.project_id::TEXT
 THEN RAISE EXCEPTION 'source export request owner mismatch'; END IF;
 PERFORM pg_advisory_xact_lock(719247031);
 incoming_metadata:=octet_length(NEW.request_bytes)+COALESCE(octet_length(NEW.verified_bytes),4194304);
 SELECT COALESCE(sum(octet_length(request_bytes)+COALESCE(octet_length(verified_bytes),4194304)),0),
    COALESCE(sum(octet_length(request_bytes)+COALESCE(octet_length(verified_bytes),4194304)) FILTER(WHERE project_id=NEW.project_id),0)
 INTO total_metadata,project_metadata FROM profiled_source_exports WHERE operation_id<>NEW.operation_id;
 IF total_metadata+incoming_metadata>268435456 OR project_metadata+incoming_metadata>67108864
 THEN RAISE EXCEPTION 'source export metadata quota'; END IF;
 IF TG_OP='INSERT' THEN
  IF NEW.state<>'pending' OR NEW.charged_bytes<>NEW.reserved_bytes THEN RAISE EXCEPTION 'source export needs prior reservation'; END IF;
  IF current_setting('transaction_isolation') NOT IN ('read committed','serializable') THEN RAISE EXCEPTION 'source export quota needs current visibility'; END IF;
  SELECT count(*) INTO n FROM profiled_source_exports;
  IF n>=4096 OR (SELECT count(*) FROM profiled_source_exports WHERE project_id=NEW.project_id)>=256
  THEN RAISE EXCEPTION 'source export row quota'; END IF;
 ELSE
  IF ROW(NEW.operation_id,NEW.project_id,NEW.request_bytes,NEW.request_digest,NEW.reserved_bytes)
      IS DISTINCT FROM ROW(OLD.operation_id,OLD.project_id,OLD.request_bytes,OLD.request_digest,OLD.reserved_bytes)
    OR (OLD.state='verified' AND NEW IS DISTINCT FROM OLD)
    OR (OLD.state='pending' AND NEW.state NOT IN ('pending','verified'))
    OR (NEW.state='pending' AND NEW.charged_bytes<>OLD.charged_bytes)
  THEN RAISE EXCEPTION 'source export identity/completion is immutable'; END IF;
 END IF;
 IF NEW.state='verified' THEN
  v:=convert_from(NEW.verified_bytes,'UTF8')::JSONB;
  IF v->>'schema_version' IS DISTINCT FROM '1' OR v->>'request_digest' IS DISTINCT FROM NEW.request_digest
     OR v->>'export_operation_id' IS DISTINCT FROM NEW.operation_id::TEXT
     OR COALESCE((v->>'bundle_bytes')::BIGINT,0)<=0
     OR octet_length(COALESCE(v->>'manifest_json','')) NOT BETWEEN 1 AND 1048576
     OR (v->>'bundle_bytes')::BIGINT>1073741824
     OR octet_length(COALESCE((v->'export_response')::TEXT,''))>1048576
     OR octet_length(COALESCE((v->'retention_inspection')::TEXT,''))>16384
     OR COALESCE((v->>'observed_at_ms')::BIGINT,0)<=0
     OR COALESCE((v->>'expires_at_ms')::BIGINT,0)<=COALESCE((v->>'observed_at_ms')::BIGINT,0)
     OR (v->>'expires_at_ms')::BIGINT-(v->>'observed_at_ms')::BIGINT>30000
     OR (v->>'bundle_bytes')::BIGINT+octet_length(v->>'manifest_json')<>NEW.charged_bytes
     OR NEW.charged_bytes>NEW.reserved_bytes
     OR COALESCE(v->>'bundle_digest','') !~ '^[0-9a-f]{64}$'
     OR COALESCE(v->>'producer_installation_id','') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
     OR v->>'producer_installation_id'='00000000-0000-0000-0000-000000000000'
     OR v->>'manifest_digest' IS DISTINCT FROM encode(sha256(convert_to(v->>'manifest_json','UTF8')),'hex')
  THEN RAISE EXCEPTION 'source export verified fact mismatch'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_source_export_guard BEFORE INSERT OR UPDATE OR DELETE ON profiled_source_exports
 FOR EACH ROW EXECUTE FUNCTION profiled_source_export_guard();

CREATE TABLE profiled_launch_source_exports (
 attempt_id UUID PRIMARY KEY REFERENCES profiled_launches(attempt_id),
 operation_id UUID NOT NULL UNIQUE REFERENCES profiled_source_exports(operation_id)
);
CREATE FUNCTION profiled_source_export_link_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE e profiled_source_exports%ROWTYPE; q JSONB; l profiled_launches%ROWTYPE; held BOOLEAN;
BEGIN
 IF TG_OP<>'INSERT' THEN RAISE EXCEPTION 'source export link is immutable'; END IF;
 SELECT holds_task INTO held FROM execution_attempts WHERE attempt_id=NEW.attempt_id AND kind='profiled_v1' FOR UPDATE;
 SELECT * INTO STRICT l FROM profiled_launches WHERE attempt_id=NEW.attempt_id;
 SELECT * INTO STRICT e FROM profiled_source_exports WHERE operation_id=NEW.operation_id FOR SHARE;
 q:=convert_from(e.request_bytes,'UTF8')::JSONB;
 IF held IS DISTINCT FROM TRUE OR e.state<>'verified'
    OR q->>'project_id' IS DISTINCT FROM l.launch_manifest#>>'{bindings,project_id}'
    OR q->>'project_revision' IS DISTINCT FROM l.launch_manifest#>>'{bindings,project_revision}'
    OR q->>'repository_id' IS DISTINCT FROM l.source_binding->>'repository_id'
    OR q->>'binding_generation' IS DISTINCT FROM l.source_binding->>'binding_generation'
    OR q->>'logical_identity' IS DISTINCT FROM l.source_binding->>'logical_identity'
    OR q->'base' IS DISTINCT FROM l.source_binding->'base'
    OR q->>'repository_id' IS DISTINCT FROM l.execution_spec#>>'{source,repository_id}'
    OR q->>'binding_generation' IS DISTINCT FROM l.execution_spec#>>'{source,repository_binding_generation}'
    OR l.execution_spec#>'{source,resolved_object}' IS DISTINCT FROM jsonb_build_object('format','sha1','hex',q#>>'{base,hex}')
 THEN RAISE EXCEPTION 'source export launch binding mismatch'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_source_export_link_guard BEFORE INSERT OR UPDATE OR DELETE ON profiled_launch_source_exports
 FOR EACH ROW EXECUTE FUNCTION profiled_source_export_link_guard();

ALTER TABLE source_bases ADD COLUMN profiled_export_operation_id UUID REFERENCES profiled_source_exports(operation_id);
CREATE FUNCTION source_base_reservation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE e profiled_source_exports%ROWTYPE; v JSONB; l profiled_launches%ROWTYPE; assigned_node UUID;
BEGIN
 IF TG_OP='DELETE' THEN
  IF OLD.profiled_export_operation_id IS NOT NULL THEN RAISE EXCEPTION 'native source wrapper is immutable'; END IF;
  RETURN OLD;
 END IF;
 IF TG_OP='UPDATE' AND OLD.profiled_export_operation_id IS NOT NULL AND NEW IS DISTINCT FROM OLD
 THEN RAISE EXCEPTION 'native source wrapper is immutable'; END IF;
 IF NEW.profiled_export_operation_id IS NULL THEN
  IF NEW.reserved_bytes<=0 THEN RAISE EXCEPTION 'legacy source base needs positive reservation'; END IF;
  RETURN NEW;
 END IF;
 SELECT * INTO STRICT e FROM profiled_source_exports WHERE operation_id=NEW.profiled_export_operation_id;
 v:=convert_from(e.verified_bytes,'UTF8')::JSONB;
 SELECT * INTO STRICT l FROM profiled_launches WHERE attempt_id=NEW.attempt_id;
 SELECT node_id INTO STRICT assigned_node FROM execution_attempts WHERE attempt_id=NEW.attempt_id AND kind='profiled_v1';
 IF NEW.node_id<>assigned_node OR NEW.retention_handle::TEXT IS DISTINCT FROM (convert_from(e.request_bytes,'UTF8')::JSONB)->>'source_retention_handle'
    OR NEW.reserved_bytes<>0 OR e.state<>'verified' OR NEW.operation_id<>e.operation_id
    OR NOT EXISTS(SELECT 1 FROM profiled_launch_source_exports x WHERE x.attempt_id=NEW.attempt_id AND x.operation_id=e.operation_id)
    OR NEW.manifest IS NULL OR NEW.manifest->>'role' IS DISTINCT FROM 'base'
    OR NEW.manifest->'source' IS DISTINCT FROM l.source_binding
    OR NEW.manifest->>'resolved_spec_digest' IS DISTINCT FROM l.execution_spec_digest
    OR NEW.manifest->'attempt' IS DISTINCT FROM jsonb_build_object('attempt_id',NEW.attempt_id,'session_id',l.launch_binding#>>'{session,session_id}','generation',(l.launch_binding->>'assignment_generation')::BIGINT)
    OR NEW.manifest->'checkpoint' IS DISTINCT FROM l.source_binding->'base'
    OR NEW.manifest->>'transfer_id' IS DISTINCT FROM e.operation_id::TEXT
    OR NEW.manifest->>'artifact_id' IS DISTINCT FROM e.operation_id::TEXT
    OR NEW.manifest->>'manifest_digest' IS DISTINCT FROM v->>'manifest_digest'
    OR NEW.manifest->>'bundle_digest' IS DISTINCT FROM v->>'bundle_digest'
    OR NEW.manifest->>'manifest_bytes' IS DISTINCT FROM octet_length(v->>'manifest_json')::TEXT
    OR NEW.manifest->>'bundle_bytes' IS DISTINCT FROM v->>'bundle_bytes'
 THEN RAISE EXCEPTION 'shared source wrapper needs exact owned export'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER source_base_reservation_guard BEFORE INSERT OR UPDATE OR DELETE ON source_bases
 FOR EACH ROW EXECUTE FUNCTION source_base_reservation_guard();

UPDATE meta SET value='22' WHERE key='state_format_version';
