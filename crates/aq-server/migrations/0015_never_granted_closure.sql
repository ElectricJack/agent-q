-- Distinct central absence-of-authorization proof. No guardian stop is inferred.
CREATE TABLE profiled_never_granted_closures (
 attempt_id UUID PRIMARY KEY REFERENCES profiled_authority_hot(attempt_id),
 closure_id UUID NOT NULL UNIQUE,
 operation_id UUID NOT NULL UNIQUE,
 binding JSONB NOT NULL CHECK(octet_length(binding::text)<=65536),
 observed_record_revision BIGINT NOT NULL CHECK(observed_record_revision>0),
 closed_at_ms BIGINT NOT NULL CHECK(closed_at_ms>0),
 reason TEXT NOT NULL CHECK(reason IN ('offer_expired','preparation_expired')),
 proof_bytes BYTEA NOT NULL CHECK(octet_length(proof_bytes) BETWEEN 1 AND 16384),
 proof_digest TEXT NOT NULL CHECK(proof_digest ~ '^[0-9a-f]{64}$'),
 CHECK(proof_digest=encode(sha256(proof_bytes),'hex')),
 CHECK((convert_from(proof_bytes,'UTF8')::jsonb)->'binding'=binding)
);
CREATE TRIGGER profiled_never_granted_immutable BEFORE UPDATE OR DELETE ON profiled_never_granted_closures
 FOR EACH ROW EXECUTE FUNCTION profiled_sealed_row_guard();
CREATE INDEX execution_task_history ON execution_attempts(task_id,attempt_id);
CREATE INDEX source_transfer_attempt ON source_transfers(attempt_id);
CREATE INDEX observation_source_attempt ON observation_sources(attempt_id);
CREATE INDEX mailbox_receipt_attempt ON mailbox_receipts(attempt_id);
CREATE INDEX mailbox_session_sender_attempt ON mailbox_messages((sender#>>'{binding,attempt_id}')) WHERE sender->>'kind'='session';
CREATE INDEX profiled_grant_receipt_attempt ON profiled_execution_receipts((response#>>'{body,grant,binding,attempt_id}')) WHERE kind IN ('prepare','renew') AND response->>'kind'='grant';
CREATE FUNCTION profiled_no_execution_effects(p_attempt UUID) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT NOT EXISTS(SELECT 1 FROM profiled_execution_receipts WHERE kind IN ('prepare','renew') AND response->>'kind'='grant' AND response#>>'{body,grant,binding,attempt_id}'=p_attempt::text)
 AND NOT EXISTS(SELECT 1 FROM source_transfers WHERE attempt_id=p_attempt)
 AND NOT EXISTS(SELECT 1 FROM agent_graph_operations WHERE attempt_id=p_attempt)
 AND NOT EXISTS(SELECT 1 FROM agent_created_records WHERE attempt_id=p_attempt)
 AND NOT EXISTS(SELECT 1 FROM observation_sources WHERE attempt_id=p_attempt)
 AND NOT EXISTS(SELECT 1 FROM observation_events WHERE attempt_id=p_attempt)
 AND NOT EXISTS(SELECT 1 FROM mailbox_receipts WHERE attempt_id=p_attempt)
 AND NOT EXISTS(SELECT 1 FROM mailbox_deliveries WHERE attempt_id=p_attempt AND disposition NOT IN ('pending','stale'))
 AND NOT EXISTS(SELECT 1 FROM mailbox_messages WHERE sender->>'kind'='session' AND sender#>>'{binding,attempt_id}'=p_attempt::text)
$$;
CREATE FUNCTION profiled_never_granted_valid(p_attempt UUID) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM profiled_never_granted_closures c
 JOIN profiled_authority_hot h ON h.attempt_id=c.attempt_id
 JOIN profiled_launches l ON l.attempt_id=c.attempt_id
 JOIN execution_attempts a ON a.attempt_id=c.attempt_id
 WHERE c.attempt_id=p_attempt AND a.kind='profiled_v1'
 AND h.state->>'status'='never_granted' AND h.state->'grant'='null'::jsonb
 AND c.binding=l.launch_binding
 AND c.proof_digest=encode(sha256(c.proof_bytes),'hex')
 AND NOT EXISTS(SELECT 1 FROM admission_material_leases WHERE prospective_attempt_id=p_attempt AND state IN ('pending','admitted'))
 AND profiled_no_execution_effects(p_attempt))
$$;
CREATE OR REPLACE FUNCTION profiled_hot_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE base execution_attempts%ROWTYPE; previous_status TEXT; current_status TEXT; current_stop TEXT;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'immutable hot authority adoption' USING ERRCODE='23514'; END IF;
 current_status:=NEW.state->>'status';
 current_stop:=COALESCE(NEW.state#>>'{stop,kind}','');
 IF NEW.state->>'schema_version' IS DISTINCT FROM '1'
    OR NOT (NEW.state ?& ARRAY['schema_version','status','grant','execution_deadline_ms','owner','cancellation_id','revoked','stop','retention','effects'])
    OR jsonb_typeof(NEW.state->'revoked') IS DISTINCT FROM 'boolean'
    OR current_status IS NULL OR current_status NOT IN ('offered','preparing','granted','cancelling','finished','declined','unknown','never_granted')
 THEN RAISE EXCEPTION 'invalid hot authority state' USING ERRCODE='23514'; END IF;
 IF TG_OP='INSERT' THEN
   SELECT * INTO STRICT base FROM execution_attempts WHERE attempt_id=NEW.attempt_id FOR UPDATE;
   IF base.kind<>'profiled_v1' THEN RAISE EXCEPTION 'profiled adoption only' USING ERRCODE='23514'; END IF;
   IF NEW.state - 'owner' IS DISTINCT FROM jsonb_build_object(
      'schema_version',base.state->'schema_version','status',base.state->'status',
      'grant',base.state->'grant','execution_deadline_ms',base.state->'execution_deadline_ms',
      'cancellation_id',base.state->'cancellation_id','revoked',base.state->'revoked',
      'stop',base.state->'stop','retention',base.state->'retention','effects',base.state->'effects')
      OR NEW.state->'owner' IS DISTINCT FROM 'null'::jsonb
      OR NEW.state->'grant' IS DISTINCT FROM 'null'::jsonb
   THEN RAISE EXCEPTION 'adoption must preserve actual compatibility authority without inventing owner' USING ERRCODE='23514'; END IF;
   NEW.record_revision:=base.record_revision;
   NEW.authority_revision:=base.authority_revision;
   NEW.grant_revision:=base.grant_revision;
 ELSE
   -- The producer already owns node->attempt->hot locks. Do not acquire the
   -- attempt lock backwards from a raw UPDATE of hot; the common loader also
   -- checks holds_task freshly and refuses released active authority.
   SELECT * INTO STRICT base FROM execution_attempts WHERE attempt_id=OLD.attempt_id;
   previous_status:=OLD.state->>'status';
   IF NEW.attempt_id IS DISTINCT FROM OLD.attempt_id THEN RAISE EXCEPTION 'immutable hot attempt' USING ERRCODE='23514'; END IF;
   IF OLD.state->'execution_deadline_ms'<>'null'::jsonb AND NEW.state->'execution_deadline_ms' IS DISTINCT FROM OLD.state->'execution_deadline_ms'
   THEN RAISE EXCEPTION 'immutable hot hard deadline' USING ERRCODE='23514'; END IF;
   IF OLD.state->'owner'<>'null'::jsonb AND NEW.state->'owner' IS DISTINCT FROM OLD.state->'owner'
   THEN RAISE EXCEPTION 'immutable hot launch owner' USING ERRCODE='23514'; END IF;
   IF OLD.state->'revoked'='true'::jsonb AND NEW.state->'revoked'<>'true'::jsonb
   THEN RAISE EXCEPTION 'hot revocation is monotonic' USING ERRCODE='23514'; END IF;
   IF OLD.state->'cancellation_id'<>'null'::jsonb AND NEW.state->'cancellation_id' IS DISTINCT FROM OLD.state->'cancellation_id'
   THEN RAISE EXCEPTION 'immutable hot cancellation' USING ERRCODE='23514'; END IF;
   IF OLD.state#>>'{stop,kind}' IN ('not_started','stopped') AND NEW.state->'stop' IS DISTINCT FROM OLD.state->'stop'
   THEN RAISE EXCEPTION 'known stop cannot be undone or rewritten' USING ERRCODE='23514'; END IF;
   IF (previous_status IN ('finished','declined','never_granted') AND current_status<>previous_status)
      OR (previous_status IN ('cancelling','unknown') AND current_status IN ('offered','preparing','granted'))
      OR (previous_status='preparing' AND current_status='offered')
      OR (previous_status='granted' AND current_status IN ('offered','preparing','declined'))
   THEN RAISE EXCEPTION 'hot authority cannot reactivate' USING ERRCODE='23514'; END IF;
   IF OLD.state->'grant'<>'null'::jsonb THEN
     IF NEW.state->'grant'='null'::jsonb OR (NEW.state->'grant')-'expires_at_ms' IS DISTINCT FROM (OLD.state->'grant')-'expires_at_ms'
     THEN RAISE EXCEPTION 'issued grant identity and binding immutable' USING ERRCODE='23514'; END IF;
     IF NEW.state->'grant' IS DISTINCT FROM OLD.state->'grant'
        AND (previous_status<>'granted' OR current_status<>'granted' OR NEW.state->'revoked'<>'false'::jsonb OR NEW.state->'cancellation_id'<>'null'::jsonb)
     THEN RAISE EXCEPTION 'renewal requires current live granted status' USING ERRCODE='23514'; END IF;
   ELSIF NEW.state->'grant'<>'null'::jsonb AND (previous_status<>'preparing' OR current_status<>'granted')
   THEN RAISE EXCEPTION 'first grant requires durable preparation' USING ERRCODE='23514'; END IF;
   NEW.record_revision:=OLD.record_revision;
   NEW.authority_revision:=OLD.authority_revision;
   NEW.grant_revision:=OLD.grant_revision;
   IF NEW.state IS DISTINCT FROM OLD.state THEN NEW.record_revision:=OLD.record_revision+1; END IF;
   IF ROW(NEW.state->'status',NEW.state->'owner',NEW.state->'cancellation_id',NEW.state->'revoked',NEW.state->'stop',NEW.state->'execution_deadline_ms',NEW.state->'grant'->'grant_id')
      IS DISTINCT FROM ROW(OLD.state->'status',OLD.state->'owner',OLD.state->'cancellation_id',OLD.state->'revoked',OLD.state->'stop',OLD.state->'execution_deadline_ms',OLD.state->'grant'->'grant_id')
   THEN NEW.authority_revision:=OLD.authority_revision+1; END IF;
   IF NEW.state->'grant' IS DISTINCT FROM OLD.state->'grant' THEN NEW.grant_revision:=OLD.grant_revision+1; END IF;
 END IF;
 IF (current_status='never_granted' AND (NEW.state->'grant'<>'null'::jsonb OR current_stop<>'unknown' OR NOT EXISTS(SELECT 1 FROM profiled_never_granted_closures WHERE attempt_id=NEW.attempt_id)))
    OR (current_status='offered' AND (NEW.state->'owner'<>'null'::jsonb OR NEW.state->'grant'<>'null'::jsonb OR current_stop<>'unknown'))
    OR (current_status='preparing' AND (jsonb_typeof(NEW.state->'owner') IS DISTINCT FROM 'object' OR NEW.state->'grant'<>'null'::jsonb OR current_stop<>'unknown'))
    OR (current_status IN ('finished','declined') AND current_stop NOT IN ('not_started','stopped'))
    OR (current_status='declined' AND NEW.state->'grant'<>'null'::jsonb)
    OR (current_stop IN ('not_started','stopped') AND (jsonb_typeof(NEW.state->'owner') IS DISTINCT FROM 'object' OR current_status IN ('offered','preparing','granted')))
    OR (current_status='granted' AND (NOT base.holds_task OR NEW.state->'revoked'<>'false'::jsonb OR NEW.state->'cancellation_id'<>'null'::jsonb OR current_stop NOT IN ('unknown','running')
        OR jsonb_typeof(NEW.state->'owner') IS DISTINCT FROM 'object' OR jsonb_typeof(NEW.state->'grant') IS DISTINCT FROM 'object'
        OR COALESCE((NEW.state->>'execution_deadline_ms')::BIGINT,0)<=0
        OR COALESCE((NEW.state#>>'{grant,expires_at_ms}')::BIGINT,0)<=0
        OR (NEW.state#>>'{grant,expires_at_ms}')::BIGINT>(NEW.state->>'execution_deadline_ms')::BIGINT))
 THEN RAISE EXCEPTION 'inconsistent hot status/claim/owner' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE OR REPLACE FUNCTION profiled_historical_snapshot_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.kind='profiled_v1' AND EXISTS(SELECT 1 FROM profiled_authority_hot WHERE attempt_id=OLD.attempt_id)
    AND (NEW.state IS DISTINCT FROM OLD.state OR NEW.execution_deadline_ms IS DISTINCT FROM OLD.execution_deadline_ms)
 THEN RAISE EXCEPTION 'hot authority owns current profiled state' USING ERRCODE='23514'; END IF;
 IF OLD.kind='profiled_v1' AND NOT OLD.holds_task AND NEW.holds_task
    AND EXISTS(SELECT 1 FROM profiled_authority_hot WHERE attempt_id=OLD.attempt_id)
 THEN RAISE EXCEPTION 'released profiled claim cannot be reacquired' USING ERRCODE='23514'; END IF;
 IF OLD.kind='profiled_v1' AND OLD.holds_task AND NOT NEW.holds_task
    AND EXISTS(SELECT 1 FROM profiled_authority_hot WHERE attempt_id=OLD.attempt_id)
    AND NOT EXISTS(SELECT 1 FROM profiled_verified_releases WHERE attempt_id=OLD.attempt_id)
    AND NOT profiled_never_granted_valid(OLD.attempt_id)
 THEN RAISE EXCEPTION 'actual owner-verified release record required' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;

-- Internal owner only: caller supplies server clock AFTER acquiring node lock.
-- Material/source/effect writers use the same node/attempt authority fence.
CREATE FUNCTION profiled_close_expired_ungranted(p_node UUID,p_attempt UUID,p_now BIGINT,p_closure UUID,p_operation UUID)
RETURNS TABLE(body_bytes BYTEA,body_digest TEXT,changed BOOLEAN) LANGUAGE plpgsql AS $$
DECLARE a execution_attempts%ROWTYPE; h profiled_authority_hot%ROWTYPE;
 l profiled_launches%ROWTYPE; deadline BIGINT; why TEXT; ids JSONB;
 doc JSONB; raw BYTEA; hashed TEXT; lease_count INTEGER; accepted_plan JSONB; pending_ids JSONB; pending_count INTEGER;
BEGIN
 IF p_now<=0 OR p_closure='00000000-0000-0000-0000-000000000000'::uuid OR p_operation='00000000-0000-0000-0000-000000000000'::uuid
 THEN RAISE EXCEPTION 'invalid internal closure identity/time' USING ERRCODE='23514'; END IF;
 PERFORM 1 FROM nodes WHERE id=p_node FOR UPDATE;
 SELECT * INTO STRICT a FROM execution_attempts WHERE attempt_id=p_attempt AND node_id=p_node FOR UPDATE;
 IF a.kind<>'profiled_v1' THEN RAISE EXCEPTION 'native ungranted owner only' USING ERRCODE='23514'; END IF;
 SELECT * INTO STRICT h FROM profiled_authority_hot WHERE attempt_id=p_attempt FOR UPDATE;
 SELECT c.proof_bytes,c.proof_digest INTO raw,hashed FROM profiled_never_granted_closures c WHERE c.attempt_id=p_attempt;
 IF FOUND THEN
   IF a.holds_task OR NOT profiled_never_granted_valid(p_attempt) THEN RAISE EXCEPTION 'closure proof/current authority conflict' USING ERRCODE='23514'; END IF;
   RETURN QUERY SELECT raw,hashed,FALSE; RETURN;
 END IF;
 SELECT * INTO STRICT l FROM profiled_launches WHERE attempt_id=p_attempt FOR SHARE;
 IF l.launch_binding IS DISTINCT FROM a.state#>'{offer,binding}'
    OR l.launch_binding->>'attempt_id' IS DISTINCT FROM a.attempt_id::text
    OR l.launch_binding->>'offer_id' IS DISTINCT FROM a.offer_id::text
    OR l.launch_binding#>>'{session,node_id}' IS DISTINCT FROM a.node_id::text
    OR l.launch_binding#>>'{session,attempt_id}' IS DISTINCT FROM a.attempt_id::text
    OR l.launch_binding#>>'{session,session_id}' IS DISTINCT FROM a.session_id::text
 THEN RAISE EXCEPTION 'actual sealed/common assignment binding mismatch' USING ERRCODE='23514'; END IF;
 IF NOT a.holds_task OR h.state->'grant'<>'null'::jsonb OR h.state#>>'{stop,kind}'<>'unknown'
 THEN RAISE EXCEPTION 'issued/unknown-start or released authority cannot use no-grant closure' USING ERRCODE='23514'; END IF;
 IF h.state->>'status'='offered' THEN deadline:=l.offer_expires_at_ms; why:='offer_expired';
 ELSIF h.state->>'status'='preparing' THEN
   SELECT response->'body' INTO accepted_plan FROM profiled_execution_receipts
    WHERE node_id=p_node AND operation_id=(h.state->>'preparation_operation_id')::UUID
      AND kind='accept' AND response->>'kind'='preparation_plan';
   IF accepted_plan IS NULL OR accepted_plan->'binding' IS DISTINCT FROM l.launch_binding
      OR accepted_plan->>'operation_id' IS DISTINCT FROM h.state->>'preparation_operation_id'
      OR accepted_plan->>'guardian_id' IS DISTINCT FROM h.state#>>'{owner,guardian_id}'
      OR accepted_plan->>'launch_id' IS DISTINCT FROM h.state#>>'{owner,launch_id}'
      OR accepted_plan#>>'{reservation,reservation_id}' IS DISTINCT FROM h.state#>>'{owner,reservation_id}'
      OR jsonb_typeof(accepted_plan->'materials') IS DISTINCT FROM 'array' OR jsonb_array_length(accepted_plan->'materials') NOT BETWEEN 1 AND 16
      OR COALESCE((accepted_plan#>>'{clock,valid_for_ms}')::BIGINT,0) NOT BETWEEN 1 AND 120000
      OR COALESCE((accepted_plan#>>'{clock,server_time_ms}')::BIGINT,0)<=0
      OR (accepted_plan#>>'{clock,server_time_ms}')::BIGINT+(accepted_plan#>>'{clock,valid_for_ms}')::BIGINT IS DISTINCT FROM (accepted_plan#>>'{clock,expires_at_ms}')::BIGINT
   THEN RAISE EXCEPTION 'accepted preparation authority unavailable/unsupported' USING ERRCODE='23514'; END IF;
   deadline:=(accepted_plan#>>'{clock,expires_at_ms}')::BIGINT;
   why:='preparation_expired';
 ELSE RAISE EXCEPTION 'not an expirable pre-grant state' USING ERRCODE='23514'; END IF;
 IF deadline IS NULL THEN RAISE EXCEPTION 'preparation deadline unavailable' USING ERRCODE='23514'; END IF;
 IF p_now<deadline THEN RETURN; END IF;
 IF EXISTS(SELECT 1 FROM profiled_execution_receipts WHERE node_id=p_node AND kind IN ('prepare','renew') AND response->>'kind'='grant' AND response#>>'{body,grant,binding,attempt_id}'=p_attempt::text)
    OR NOT profiled_no_execution_effects(p_attempt)
 THEN RAISE EXCEPTION 'actual grant/source/effect records contradict no-execution proof' USING ERRCODE='23514'; END IF;
 PERFORM 1 FROM mailbox_deliveries WHERE attempt_id=p_attempt ORDER BY delivery_id FOR UPDATE;
 SELECT count(*),COALESCE(jsonb_agg(delivery_id ORDER BY delivery_id),'[]'::jsonb) INTO pending_count,pending_ids FROM mailbox_deliveries WHERE attempt_id=p_attempt AND disposition='pending';
 IF pending_count>256 THEN RAISE EXCEPTION 'no-grant pending delivery closure bound' USING ERRCODE='23514'; END IF;
 IF NOT profiled_no_execution_effects(p_attempt) THEN RAISE EXCEPTION 'delivery uncertainty contradicts no-execution proof' USING ERRCODE='23514'; END IF;
 UPDATE mailbox_deliveries SET disposition='stale',updated_at_ms=p_now WHERE attempt_id=p_attempt AND disposition='pending';
 PERFORM 1 FROM admission_material_leases WHERE prospective_attempt_id=p_attempt ORDER BY lease_id FOR UPDATE;
 SELECT count(*),COALESCE(jsonb_agg(lease_id ORDER BY lease_id),'[]'::jsonb) INTO lease_count,ids FROM admission_material_leases WHERE prospective_attempt_id=p_attempt;
 IF lease_count>64 THEN RAISE EXCEPTION 'no-grant material history bound' USING ERRCODE='23514'; END IF;
 UPDATE admission_material_leases SET state='revoked' WHERE prospective_attempt_id=p_attempt AND state IN ('pending','admitted');
 UPDATE agent_launch_bindings SET revoked=TRUE,credential_verifier=NULL,credential_expires_at_ms=NULL,credential_version=credential_version+1,authority_revision=authority_revision+1 WHERE attempt_id=p_attempt;
 doc:=jsonb_build_object('schema_version',1,'closure_id',p_closure,'operation_id',p_operation,
  'binding',l.launch_binding,'observed_record_revision',h.record_revision,'closed_at_ms',p_now,'reason',why,
  'applicable_deadline_ms',deadline,'material_leases_revoked',ids,'pending_deliveries_staled',pending_ids,'pending_deliveries_staled_count',pending_count,
  'source_checkpoint',jsonb_build_object('task_checkpoint_transfers',0),
  'effect_checkpoint',jsonb_build_object('graph_operations',0,'created_records',0,'observation_sources',0,'observation_events',0,'mailbox_receipts',0,'uncertain_deliveries',0,'session_messages',0));
 raw:=convert_to(doc::text,'UTF8'); hashed:=encode(sha256(raw),'hex');
 INSERT INTO profiled_never_granted_closures(attempt_id,closure_id,operation_id,binding,observed_record_revision,closed_at_ms,reason,proof_bytes,proof_digest)
 VALUES(p_attempt,p_closure,p_operation,l.launch_binding,h.record_revision,p_now,why,raw,hashed);
 UPDATE profiled_authority_hot SET state=jsonb_set(state,'{status}','"never_granted"') WHERE attempt_id=p_attempt;
 UPDATE execution_attempts SET holds_task=FALSE WHERE attempt_id=p_attempt;
 RETURN QUERY SELECT raw,hashed,TRUE;
END $$;
CREATE FUNCTION profiled_no_grant_material_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.state IN ('pending','admitted') AND EXISTS(SELECT 1 FROM profiled_never_granted_closures WHERE attempt_id=NEW.prospective_attempt_id)
 THEN RAISE EXCEPTION 'unissued assignment permanently closed' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_no_grant_material_guard BEFORE INSERT OR UPDATE ON admission_material_leases FOR EACH ROW EXECUTE FUNCTION profiled_no_grant_material_guard();
CREATE FUNCTION profiled_no_grant_receipt_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.kind IN ('prepare','renew') AND NEW.response->>'kind'='grant'
    AND EXISTS(SELECT 1 FROM profiled_never_granted_closures WHERE attempt_id::text=NEW.response#>>'{body,grant,binding,attempt_id}')
 THEN RAISE EXCEPTION 'closed unissued assignment cannot emit a grant' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_no_grant_receipt_guard BEFORE INSERT OR UPDATE ON profiled_execution_receipts FOR EACH ROW EXECUTE FUNCTION profiled_no_grant_receipt_guard();
CREATE FUNCTION profiled_no_grant_proof_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE h profiled_authority_hot%ROWTYPE; l profiled_launches%ROWTYPE; doc JSONB;
BEGIN
 SELECT * INTO STRICT h FROM profiled_authority_hot WHERE attempt_id=NEW.attempt_id;
 SELECT * INTO STRICT l FROM profiled_launches WHERE attempt_id=NEW.attempt_id;
 doc:=convert_from(NEW.proof_bytes,'UTF8')::jsonb;
 IF h.state->'grant'<>'null'::jsonb OR h.state#>>'{stop,kind}'<>'unknown'
    OR NEW.binding IS DISTINCT FROM l.launch_binding OR NEW.observed_record_revision<>h.record_revision
    OR doc->>'closure_id' IS DISTINCT FROM NEW.closure_id::text
    OR doc->>'operation_id' IS DISTINCT FROM NEW.operation_id::text
    OR doc->>'reason' IS DISTINCT FROM NEW.reason
    OR doc->>'closed_at_ms' IS DISTINCT FROM NEW.closed_at_ms::text
    OR doc->>'observed_record_revision' IS DISTINCT FROM NEW.observed_record_revision::text
    OR NOT profiled_no_execution_effects(NEW.attempt_id)
    OR EXISTS(SELECT 1 FROM admission_material_leases WHERE prospective_attempt_id=NEW.attempt_id AND state IN ('pending','admitted'))
 THEN RAISE EXCEPTION 'central no-grant proof contradicts current owners' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_no_grant_proof_guard BEFORE INSERT ON profiled_never_granted_closures FOR EACH ROW EXECUTE FUNCTION profiled_no_grant_proof_guard();
CREATE FUNCTION profiled_no_grant_delivery_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM profiled_never_granted_closures WHERE attempt_id=NEW.attempt_id) THEN
   IF NEW.disposition='pending' THEN NEW.disposition:='stale';
   ELSIF NEW.disposition<>'stale' THEN RAISE EXCEPTION 'closed unissued assignment cannot receive mail' USING ERRCODE='23514'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_no_grant_delivery_guard BEFORE INSERT OR UPDATE ON mailbox_deliveries FOR EACH ROW EXECUTE FUNCTION profiled_no_grant_delivery_guard();
UPDATE meta SET value='15' WHERE key='state_format_version';
