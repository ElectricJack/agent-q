-- Internal, finite trusted-node pilot. Frozen migrations1..38 are unchanged.
-- This extends actual owner joins, not DTO/JSON authority or a hostile-DBA proof.
ALTER TABLE review_subjects ADD COLUMN subject_profile TEXT NOT NULL DEFAULT 'code_review_v1'
 CHECK(subject_profile IN ('code_review_v1','source_and_effects_review_v1'));
ALTER TABLE human_decision_finality ADD COLUMN combined_actor_digest TEXT,
 ADD COLUMN combined_actor_revision TEXT,
 ADD CONSTRAINT review_combined_actor_pair CHECK(COALESCE((combined_actor_digest IS NULL AND combined_actor_revision IS NULL)
 OR (actor_id IS NOT NULL AND combined_actor_digest ~ '^[0-9a-f]{64}$' AND combined_actor_revision ~ '^[0-9]{1,20}$'),FALSE));
CREATE INDEX review_combined_attempt ON review_subjects(report_id) WHERE subject_profile='source_and_effects_review_v1';
CREATE FUNCTION settlement_review_profile_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE profile TEXT;
BEGIN
 IF TG_TABLE_NAME='review_subjects' THEN
  IF TG_OP='UPDATE' AND NEW.subject_profile IS DISTINCT FROM OLD.subject_profile THEN RAISE EXCEPTION 'immutable review profile'; END IF;
  IF TG_OP='DELETE' AND OLD.subject_profile='source_and_effects_review_v1' THEN RAISE EXCEPTION 'retained combined review subject'; END IF;
  IF TG_OP='INSERT' AND NEW.subject_profile='source_and_effects_review_v1' AND current_setting('aq.settlement_owner',true) IS DISTINCT FROM 'aq.settlement-owner.1' THEN RAISE EXCEPTION 'combined review requires actual owner'; END IF;
 ELSE
  SELECT subject_profile INTO STRICT profile FROM review_subjects s JOIN human_decision_requests d USING(subject_digest) WHERE d.decision_id=NEW.decision_id;
  IF ((profile='code_review_v1' OR NEW.actor_id IS NULL) AND (NEW.combined_actor_digest IS NOT NULL OR NEW.combined_actor_revision IS NOT NULL))
   OR (profile='source_and_effects_review_v1' AND NEW.actor_id IS NOT NULL AND (NEW.combined_actor_digest IS NULL OR NEW.combined_actor_revision IS NULL OR current_setting('aq.settlement_owner',true) IS DISTINCT FROM 'aq.settlement-owner.1'))
  THEN RAISE EXCEPTION 'exact combined review actor owner required'; END IF;
 END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END $$;
CREATE TRIGGER settlement_review_profile_guard BEFORE INSERT OR UPDATE OR DELETE ON review_subjects FOR EACH ROW EXECUTE FUNCTION settlement_review_profile_guard();
CREATE TRIGGER settlement_review_actor_guard BEFORE INSERT ON human_decision_finality FOR EACH ROW EXECUTE FUNCTION settlement_review_profile_guard();

CREATE TABLE profiled_settlement_evaluations (
 operation_id UUID PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 attempt_id UUID NOT NULL REFERENCES profiled_stopped_receipts(attempt_id),
 report_id UUID NOT NULL, source_operation_id UUID NOT NULL REFERENCES source_transfers(import_operation_id),
 decision_id UUID NOT NULL REFERENCES human_decision_finality(decision_id),
 actor_id UUID NOT NULL REFERENCES users(id), actor_identity_digest TEXT NOT NULL CHECK(actor_identity_digest ~ '^[0-9a-f]{64}$'),
 request JSONB NOT NULL CHECK(jsonb_typeof(request)='object' AND octet_length(request::text)<=8192),
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 receipt JSONB NOT NULL CHECK(jsonb_typeof(receipt)='object'), receipt_digest TEXT NOT NULL CHECK(receipt_digest ~ '^[0-9a-f]{64}$'),
 evidence JSONB NOT NULL CHECK(jsonb_typeof(evidence)='object'),
 disposition TEXT NOT NULL CHECK(disposition IN ('accepted','rejected','stale','needs_reconciliation')),
 created_at_ms BIGINT NOT NULL CHECK(created_at_ms BETWEEN 1 AND 253402300799999), release_id UUID,
 reserved_bytes BIGINT NOT NULL CHECK(reserved_bytes=196608),
 CHECK(octet_length(request::text)+octet_length(receipt::text)+octet_length(evidence::text)<=131072),
 CHECK((disposition='accepted')=(release_id IS NOT NULL)),
 UNIQUE(operation_id,attempt_id),UNIQUE(release_id),
 FOREIGN KEY(attempt_id,report_id) REFERENCES profiled_result_reports(attempt_id,report_id)
);
CREATE UNIQUE INDEX settlement_one_accepted ON profiled_settlement_evaluations(attempt_id) WHERE disposition='accepted';
CREATE INDEX settlement_attempt_history ON profiled_settlement_evaluations(attempt_id,operation_id);
ALTER TABLE profiled_verified_releases ADD COLUMN settlement_operation_id UUID,
 ADD CONSTRAINT settlement_release_evaluation_fk FOREIGN KEY(settlement_operation_id,attempt_id) REFERENCES profiled_settlement_evaluations(operation_id,attempt_id) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE profiled_settlement_evaluations ADD CONSTRAINT settlement_evaluation_release_fk FOREIGN KEY(release_id) REFERENCES profiled_verified_releases(release_id) DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE profiled_task_completions (
 operation_id UUID PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 attempt_id UUID NOT NULL UNIQUE REFERENCES profiled_stopped_receipts(attempt_id),
 evaluation_operation_id UUID NOT NULL UNIQUE,release_id UUID NOT NULL UNIQUE REFERENCES profiled_verified_releases(release_id),
 workspace_id UUID NOT NULL,task_id UUID NOT NULL,expected_version BIGINT NOT NULL CHECK(expected_version BETWEEN 1 AND 9223372036854775806),
 expected_content_revision BIGINT NOT NULL CHECK(expected_content_revision BETWEEN 1 AND 9223372036854775806),
 payload_bytes BYTEA NOT NULL CHECK(octet_length(payload_bytes) BETWEEN 1 AND 8192),
 payload_digest TEXT NOT NULL CHECK(payload_digest=encode(sha256(payload_bytes),'hex')),
 service_identity JSONB NOT NULL CHECK(jsonb_typeof(service_identity)='object' AND octet_length(service_identity::text)<=4096),
 state TEXT NOT NULL CHECK(state IN ('prepared','possible','confirmed','conflict','blocked')),
 created_at_ms BIGINT NOT NULL CHECK(created_at_ms BETWEEN 1 AND 253402300739999),
 dispatch_count INTEGER NOT NULL DEFAULT 0 CHECK(dispatch_count BETWEEN 0 AND 8),
 lease_id UUID,lease_expires_at_ms BIGINT,receipt JSONB,outbox JSONB,outbox_acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
 FOREIGN KEY(evaluation_operation_id,attempt_id) REFERENCES profiled_settlement_evaluations(operation_id,attempt_id),
 CHECK((lease_id IS NULL)=(lease_expires_at_ms IS NULL)),
 CHECK(lease_expires_at_ms IS NULL OR (lease_expires_at_ms>created_at_ms AND lease_expires_at_ms<=created_at_ms+75000)),
 CHECK((state='confirmed')=(receipt IS NOT NULL AND outbox IS NOT NULL)),
 CHECK(state='confirmed' OR (receipt IS NULL AND outbox IS NULL AND NOT outbox_acknowledged)),
 CHECK(octet_length(payload_bytes)+octet_length(service_identity::text)+COALESCE(octet_length(receipt::text),0)+COALESCE(octet_length(outbox::text),0)<=65536),
 CHECK(receipt IS NULL OR (jsonb_typeof(receipt)='object' AND octet_length(receipt::text)<=16384)),
 CHECK(outbox IS NULL OR (jsonb_typeof(outbox)='object' AND octet_length(outbox::text)<=8192)),
 CHECK(state<>'prepared' OR (dispatch_count=0 AND lease_id IS NULL)),
 CHECK(state<>'confirmed' OR (dispatch_count>0 AND lease_id IS NULL))
);
CREATE INDEX settlement_pending_completions ON profiled_task_completions(created_at_ms,operation_id) WHERE state IN ('prepared','possible');
CREATE FUNCTION settlement_evaluation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE r profiled_result_reports%ROWTYPE; s source_transfers%ROWTYPE; d human_decision_requests%ROWTYPE; h JSONB; n BIGINT; used BIGINT;
BEGIN
 IF TG_OP<>'INSERT' THEN RAISE EXCEPTION 'immutable retained settlement evaluation'; END IF;
 IF current_setting('aq.settlement_owner',true) IS DISTINCT FROM 'aq.settlement-owner.1' THEN RAISE EXCEPTION 'actual settlement owner required'; END IF;
 SELECT * INTO STRICT r FROM profiled_result_reports WHERE report_id=NEW.report_id;
 SELECT * INTO STRICT s FROM source_transfers WHERE import_operation_id=NEW.source_operation_id;
 SELECT * INTO STRICT d FROM human_decision_requests WHERE decision_id=NEW.decision_id;
 SELECT state INTO STRICT h FROM profiled_authority_hot WHERE attempt_id=NEW.attempt_id;
 SELECT count(*) INTO n FROM profiled_settlement_evaluations WHERE attempt_id=NEW.attempt_id;
 SELECT COALESCE(sum(reserved_bytes),0) INTO used FROM profiled_settlement_evaluations;
 IF n>=16 OR used>67108864-196608 OR r.attempt_id<>NEW.attempt_id OR s.attempt_id<>NEW.attempt_id OR s.node_id<>r.node_id OR s.state<>'retained'
 OR h->>'status' IS DISTINCT FROM 'finished' OR h#>>'{stop,kind}' IS DISTINCT FROM 'stopped'
 OR NOT EXISTS(SELECT 1 FROM execution_attempts WHERE attempt_id=NEW.attempt_id AND holds_task)
 OR NOT EXISTS(SELECT 1 FROM profiled_result_heads WHERE attempt_id=NEW.attempt_id AND report_id=NEW.report_id)
 OR NOT EXISTS(SELECT 1 FROM review_subjects WHERE subject_digest=d.subject_digest AND report_id=NEW.report_id AND subject_profile='source_and_effects_review_v1')
 OR NEW.request->>'schema_version' IS DISTINCT FROM '1' OR NEW.request->>'operation_id' IS DISTINCT FROM NEW.operation_id::TEXT
 OR NEW.request->>'report_id' IS DISTINCT FROM NEW.report_id::TEXT OR NEW.request->>'source_operation_id' IS DISTINCT FROM NEW.source_operation_id::TEXT
 OR NEW.request->>'decision_id' IS DISTINCT FROM NEW.decision_id::TEXT OR NEW.request#>>'{subject,digest}' IS DISTINCT FROM d.subject_digest
 OR NEW.receipt->>'operation_id' IS DISTINCT FROM NEW.operation_id::TEXT OR NEW.receipt->>'attempt_id' IS DISTINCT FROM NEW.attempt_id::TEXT
 OR NEW.receipt->>'disposition' IS DISTINCT FROM NEW.disposition OR NEW.receipt->>'recorded_at_ms' IS DISTINCT FROM NEW.created_at_ms::TEXT
 OR NEW.receipt->>'release_id' IS DISTINCT FROM NEW.release_id::TEXT OR NEW.receipt->'subject' IS DISTINCT FROM NEW.request->'subject'
 OR NEW.receipt#>>'{report,id}' IS DISTINCT FROM 'aq.result-record/'||NEW.report_id::TEXT
 THEN RAISE EXCEPTION 'settlement owner cross-link, stopped basis or retained quota invalid'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER settlement_evaluation_guard BEFORE INSERT OR UPDATE OR DELETE ON profiled_settlement_evaluations FOR EACH ROW EXECUTE FUNCTION settlement_evaluation_guard();
CREATE FUNCTION settlement_release_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE e profiled_settlement_evaluations%ROWTYPE;
BEGIN
 IF current_setting('aq.settlement_owner',true) IS DISTINCT FROM 'aq.settlement-owner.1' OR NEW.settlement_operation_id IS NULL THEN RAISE EXCEPTION 'qualified settlement release owner required'; END IF;
 SELECT * INTO STRICT e FROM profiled_settlement_evaluations WHERE operation_id=NEW.settlement_operation_id;
 IF e.attempt_id<>NEW.attempt_id OR e.disposition<>'accepted' OR e.release_id<>NEW.release_id OR NEW.verified_at_ms<>e.created_at_ms
 OR NOT EXISTS(SELECT 1 FROM profiled_stopped_receipts WHERE attempt_id=NEW.attempt_id AND operation_id=NEW.stop_operation_id)
 THEN RAISE EXCEPTION 'release requires exact accepted evaluation'; END IF;
 RETURN NEW;
END $$;
DROP TRIGGER profiled_phase_unqualified_release_guard ON profiled_verified_releases;
CREATE TRIGGER profiled_phase_qualified_release_guard BEFORE INSERT ON profiled_verified_releases FOR EACH ROW EXECUTE FUNCTION settlement_release_guard();
CREATE FUNCTION settlement_completion_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE e profiled_settlement_evaluations%ROWTYPE; l JSONB; p JSONB;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'retained completion intent'; END IF;
 IF current_setting('aq.settlement_owner',true) IS DISTINCT FROM 'aq.settlement-owner.1' THEN RAISE EXCEPTION 'actual completion owner required'; END IF;
 SELECT * INTO STRICT e FROM profiled_settlement_evaluations WHERE operation_id=NEW.evaluation_operation_id;
 SELECT execution_spec INTO STRICT l FROM profiled_launches WHERE attempt_id=NEW.attempt_id;
 p:=convert_from(NEW.payload_bytes,'UTF8')::JSONB;
 IF e.disposition<>'accepted' OR e.attempt_id<>NEW.attempt_id OR e.release_id<>NEW.release_id OR e.receipt->>'completion_operation_id' IS DISTINCT FROM NEW.operation_id::TEXT
 OR NEW.workspace_id::TEXT IS DISTINCT FROM l#>>'{task,workspace_id}' OR NEW.task_id::TEXT IS DISTINCT FROM l#>>'{task,task_id}'
 OR NEW.expected_version::TEXT IS DISTINCT FROM l#>>'{task,version}' OR NEW.expected_content_revision::TEXT IS DISTINCT FROM l#>>'{task,content_revision}'
 OR p IS DISTINCT FROM jsonb_build_object('idempotency_key','aq-settlement-v1:'||NEW.operation_id::TEXT,'reason','Human-reviewed retained result '||e.report_id::TEXT,'operations',jsonb_build_array(jsonb_build_object('op','update_node','id',NEW.task_id,'expected_version',NEW.expected_version,'patch',jsonb_build_object('task_state','completed'))))
 THEN RAISE EXCEPTION 'completion cannot retarget accepted original task'; END IF;
 IF TG_OP='INSERT' THEN
  IF NEW.state<>'prepared' OR NEW.dispatch_count<>0 OR NEW.receipt IS NOT NULL OR NEW.outbox IS NOT NULL OR NEW.lease_id IS NOT NULL THEN RAISE EXCEPTION 'new completion must be undispatched'; END IF;
 ELSE
  IF (to_jsonb(NEW)-ARRAY['state','dispatch_count','lease_id','lease_expires_at_ms','receipt','outbox','outbox_acknowledged']) IS DISTINCT FROM (to_jsonb(OLD)-ARRAY['state','dispatch_count','lease_id','lease_expires_at_ms','receipt','outbox','outbox_acknowledged'])
   OR NEW.dispatch_count<OLD.dispatch_count OR NEW.dispatch_count>OLD.dispatch_count+1
   OR (OLD.outbox_acknowledged AND NOT NEW.outbox_acknowledged)
   OR (OLD.state IN ('confirmed','conflict','blocked') AND (to_jsonb(NEW)-'outbox_acknowledged') IS DISTINCT FROM (to_jsonb(OLD)-'outbox_acknowledged'))
   OR (NEW.dispatch_count=OLD.dispatch_count+1 AND (NEW.state<>'possible' OR NEW.lease_id IS NULL OR NEW.lease_expires_at_ms IS NULL))
   OR (NEW.dispatch_count=OLD.dispatch_count AND NEW.lease_id IS NOT NULL AND ROW(NEW.lease_id,NEW.lease_expires_at_ms) IS DISTINCT FROM ROW(OLD.lease_id,OLD.lease_expires_at_ms))
   OR (NEW.state IN ('confirmed','conflict') AND (OLD.state<>'possible' OR OLD.lease_id IS NULL OR NEW.lease_id IS NOT NULL))
  THEN RAISE EXCEPTION 'completion transition or immutable intent changed'; END IF;
 END IF;
 IF NEW.state='confirmed' AND (NEW.receipt->>'workspace_id' IS DISTINCT FROM NEW.workspace_id::TEXT OR NEW.receipt->>'task_id' IS DISTINCT FROM NEW.task_id::TEXT
  OR NEW.receipt->>'version' IS DISTINCT FROM (NEW.expected_version+1)::TEXT OR NEW.receipt->>'content_revision' IS DISTINCT FROM (NEW.expected_content_revision+1)::TEXT
  OR COALESCE(NEW.receipt->>'change_set_id','') !~ '^[0-9a-f-]{36}$' OR COALESCE(NEW.receipt->>'response_digest','') !~ '^[0-9a-f]{64}$'
  OR NEW.outbox->>'event_id' IS DISTINCT FROM NEW.operation_id::TEXT OR NEW.outbox->>'producer' IS DISTINCT FROM 'aq.task.settlement@1'
  OR NEW.outbox->>'event' IS DISTINCT FROM 'task.completed' OR NEW.outbox->>'attempt_id' IS DISTINCT FROM NEW.attempt_id::TEXT
  OR NEW.outbox->>'evaluation_operation_id' IS DISTINCT FROM NEW.evaluation_operation_id::TEXT OR NEW.outbox->'report' IS DISTINCT FROM e.receipt->'report'
  OR NEW.outbox->>'release_id' IS DISTINCT FROM NEW.release_id::TEXT OR NEW.outbox->'receipt' IS DISTINCT FROM NEW.receipt)
 THEN RAISE EXCEPTION 'completion receipt/outbox must match actual original task'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER settlement_completion_guard BEFORE INSERT OR UPDATE OR DELETE ON profiled_task_completions FOR EACH ROW EXECUTE FUNCTION settlement_completion_guard();
CREATE FUNCTION settlement_atomic_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.disposition='accepted' AND NOT EXISTS(SELECT 1 FROM profiled_verified_releases r JOIN profiled_task_completions c USING(attempt_id,release_id) WHERE r.attempt_id=NEW.attempt_id AND r.release_id=NEW.release_id AND r.settlement_operation_id=NEW.operation_id AND c.evaluation_operation_id=NEW.operation_id AND c.operation_id::TEXT=NEW.receipt->>'completion_operation_id') THEN RAISE EXCEPTION 'accepted evaluation/release/completion must commit together'; END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER settlement_atomic_guard AFTER INSERT ON profiled_settlement_evaluations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION settlement_atomic_guard();
CREATE FUNCTION settlement_claim_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.kind='profiled_v1' AND OLD.holds_task AND NOT NEW.holds_task AND EXISTS(SELECT 1 FROM profiled_authority_hot WHERE attempt_id=OLD.attempt_id)
 AND NOT EXISTS(SELECT 1 FROM profiled_task_completions c JOIN profiled_settlement_evaluations e ON e.operation_id=c.evaluation_operation_id JOIN profiled_verified_releases r ON r.attempt_id=c.attempt_id AND r.release_id=c.release_id WHERE c.attempt_id=OLD.attempt_id AND c.state='confirmed' AND e.disposition='accepted' AND e.operation_id=r.settlement_operation_id AND c.receipt IS NOT NULL AND c.outbox IS NOT NULL)
 THEN RAISE EXCEPTION 'claim remains held until actual Graph completion receipt'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER settlement_claim_guard BEFORE UPDATE ON execution_attempts FOR EACH ROW EXECUTE FUNCTION settlement_claim_guard();
CREATE FUNCTION settlement_frozen_basis_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE a UUID;
BEGIN
 IF TG_OP='TRUNCATE' THEN
  IF EXISTS(SELECT 1 FROM profiled_settlement_evaluations) THEN RAISE EXCEPTION 'retained settlement basis cannot truncate'; END IF;
  RETURN NULL;
 END IF;
 IF TG_TABLE_NAME='profiled_result_heads' THEN a:=NEW.attempt_id;
 ELSIF TG_OP='INSERT' THEN a:=NEW.attempt_id;
 ELSE a:=OLD.attempt_id; END IF;
 IF EXISTS(SELECT 1 FROM profiled_settlement_evaluations WHERE disposition='accepted' AND (attempt_id=a OR (TG_OP<>'DELETE' AND attempt_id=NEW.attempt_id))) THEN
  IF TG_OP='UPDATE' AND NEW IS NOT DISTINCT FROM OLD THEN RETURN NEW; END IF;
  RAISE EXCEPTION 'accepted report/source/ledger basis cannot change';
 END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END $$;
CREATE TRIGGER settlement_head_freeze BEFORE UPDATE ON profiled_result_heads FOR EACH ROW EXECUTE FUNCTION settlement_frozen_basis_guard();
CREATE TRIGGER settlement_report_freeze BEFORE INSERT ON profiled_result_reports FOR EACH ROW EXECUTE FUNCTION settlement_frozen_basis_guard();
CREATE TRIGGER settlement_graph_basis_freeze BEFORE INSERT OR UPDATE OR DELETE ON agent_graph_operations FOR EACH ROW EXECUTE FUNCTION settlement_frozen_basis_guard();
CREATE TRIGGER settlement_binding_freeze BEFORE UPDATE OR DELETE ON agent_launch_bindings FOR EACH ROW EXECUTE FUNCTION settlement_frozen_basis_guard();
CREATE TRIGGER settlement_evaluations_no_truncate BEFORE TRUNCATE ON profiled_settlement_evaluations FOR EACH STATEMENT EXECUTE FUNCTION review_immutable_record();
CREATE TRIGGER settlement_completions_no_truncate BEFORE TRUNCATE ON profiled_task_completions FOR EACH STATEMENT EXECUTE FUNCTION review_immutable_record();
CREATE TRIGGER settlement_graph_no_truncate BEFORE TRUNCATE ON agent_graph_operations FOR EACH STATEMENT EXECUTE FUNCTION settlement_frozen_basis_guard();
CREATE TRIGGER settlement_bindings_no_truncate BEFORE TRUNCATE ON agent_launch_bindings FOR EACH STATEMENT EXECUTE FUNCTION settlement_frozen_basis_guard();
CREATE FUNCTION settlement_sender_key(binding JSONB) RETURNS TEXT LANGUAGE SQL IMMUTABLE STRICT AS $$
 SELECT encode(sha256(convert_to('{"binding":{"attempt_id":'||(binding->'attempt_id')::TEXT||',"boot_id":'||(binding->'boot_id')::TEXT||',"context_digest":'||(binding->'context_digest')::TEXT||',"generation":'||(binding->'generation')::TEXT||',"node_id":'||(binding->'node_id')::TEXT||',"profile_digest":'||(binding->'profile_digest')::TEXT||',"project_id":'||(binding->'project_id')::TEXT||',"resolved_spec_digest":'||(binding->'resolved_spec_digest')::TEXT||',"session_id":'||(binding->'session_id')::TEXT||',"task_id":'||(binding->'task_id')::TEXT||',"workspace_id":'||(binding->'workspace_id')::TEXT||'},"kind":"session"}','UTF8')),'hex')
$$;
CREATE FUNCTION settlement_mail_source_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='TRUNCATE' THEN
  IF EXISTS(SELECT 1 FROM profiled_settlement_evaluations WHERE disposition='accepted') THEN RAISE EXCEPTION 'retained settlement messaging basis'; END IF;
  RETURN NULL;
 END IF;
 IF EXISTS(SELECT 1 FROM profiled_settlement_evaluations e JOIN agent_launch_bindings b USING(attempt_id) WHERE e.disposition='accepted' AND (settlement_sender_key(b.binding)=NEW.sender_key OR (TG_OP<>'INSERT' AND settlement_sender_key(b.binding)=OLD.sender_key))) THEN RAISE EXCEPTION 'accepted messaging-disabled basis immutable'; END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END $$;
CREATE TRIGGER settlement_mail_source_guard BEFORE INSERT OR UPDATE OR DELETE ON mailbox_operations FOR EACH ROW EXECUTE FUNCTION settlement_mail_source_guard();
CREATE TRIGGER settlement_mail_no_truncate BEFORE TRUNCATE ON mailbox_operations FOR EACH STATEMENT EXECUTE FUNCTION settlement_mail_source_guard();
CREATE FUNCTION settlement_source_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM profiled_settlement_evaluations WHERE source_operation_id=OLD.import_operation_id AND disposition='accepted') THEN
  IF TG_OP='DELETE' OR ROW(NEW.transfer_id,NEW.attempt_id,NEW.node_id,NEW.repository_id,NEW.artifact_id,NEW.import_operation_id,NEW.manifest,NEW.state,NEW.receipt,NEW.central_receipt) IS DISTINCT FROM ROW(OLD.transfer_id,OLD.attempt_id,OLD.node_id,OLD.repository_id,OLD.artifact_id,OLD.import_operation_id,OLD.manifest,OLD.state,OLD.receipt,OLD.central_receipt) THEN RAISE EXCEPTION 'accepted source confirmation immutable'; END IF;
 END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END $$;
CREATE TRIGGER settlement_source_guard BEFORE UPDATE OR DELETE ON source_transfers FOR EACH ROW EXECUTE FUNCTION settlement_source_guard();
CREATE TRIGGER settlement_source_no_truncate BEFORE TRUNCATE ON source_transfers FOR EACH STATEMENT EXECUTE FUNCTION settlement_frozen_basis_guard();
UPDATE meta SET value='39' WHERE key='state_format_version';
