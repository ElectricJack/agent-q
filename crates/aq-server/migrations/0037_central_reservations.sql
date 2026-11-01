-- Reservation-only logical central ownership. No Run/session/generation/host IDs.
-- Frozen historical 5/35/36 remain unchanged. Never apply to canonical from this lane.
CREATE FUNCTION central_global_mailbox_key() RETURNS TEXT LANGUAGE SQL STABLE AS $$
 SELECT encode(sha256(convert_to('{"conversation_id":"'||conversation_id::text||'","kind":"global_supervisor"}','UTF8')),'hex')
 FROM global_supervisor_conversation WHERE singleton
$$;
CREATE INDEX mailbox_global_queued_original ON mailbox_operations((response->>'message_id'))
 WHERE response->>'contract'='human-supervisor-conversation-1' AND response->>'state'='queued';
CREATE FUNCTION central_global_mail_source_valid(p_message UUID) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT count(*)=1 FROM mailbox_messages m JOIN global_supervisor_conversation g ON g.singleton
 JOIN mailbox_operations o ON o.response->>'message_id'=m.message_id::text
  AND o.response->>'contract'='human-supervisor-conversation-1' AND o.response->>'state'='queued'
 WHERE m.message_id=p_message AND m.mailbox_key=central_global_mailbox_key()
 AND m.recipient=jsonb_build_object('kind','global_supervisor','conversation_id',g.conversation_id::text)
 AND m.thread_id=g.conversation_id AND m.created_at_ms>0
 AND m.sender=jsonb_build_object('kind','owner','user_id',m.sender->>'user_id')
 AND m.sender->>'user_id' ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
 AND m.sender->>'user_id'<>'00000000-0000-0000-0000-000000000000'
 AND m.payload_digest ~ '^[0-9a-f]{64}$'
 AND o.sender_key=encode(sha256(convert_to('{"kind":"owner","user_id":"'||(m.sender->>'user_id')||'"}','UTF8')),'hex')
 AND o.payload_digest=m.payload_digest AND o.operation_id<>m.message_id
 AND o.response->>'operation_id'=o.operation_id::text
 AND o.response->>'actor_id'=m.sender->>'user_id'
 AND o.response#>>'{conversation,id}'=g.conversation_id::text
 AND o.response#>>'{conversation,revision}'='1'
 AND o.response->>'cursor'=m.cursor::text
 AND o.response->>'body_digest'='sha256:'||encode(sha256(convert_to(m.body,'UTF8')),'hex')
$$;
-- Do not bless source substitution that predates append-only protection.
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM mailbox_messages m WHERE
  (m.mailbox_key=central_global_mailbox_key() OR m.recipient->>'kind'='global_supervisor')
  AND NOT central_global_mail_source_valid(m.message_id)) THEN
  RAISE EXCEPTION 'global supervisor original mail proof is inconsistent';
 END IF;
END $$;
CREATE FUNCTION guard_central_global_mail() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='TRUNCATE' THEN
  IF TG_TABLE_NAME='mailbox_messages' THEN
   IF EXISTS(SELECT 1 FROM mailbox_messages WHERE mailbox_key=central_global_mailbox_key() OR recipient->>'kind'='global_supervisor') THEN
    RAISE EXCEPTION 'global supervisor original mail cannot be truncated';
   END IF;
  ELSE
   IF EXISTS(SELECT 1 FROM mailbox_operations WHERE response->>'contract'='human-supervisor-conversation-1') THEN
    RAISE EXCEPTION 'global supervisor original receipts cannot be truncated';
   END IF;
  END IF;
  RETURN NULL;
 END IF;
 IF TG_TABLE_NAME='mailbox_messages' THEN
  IF OLD.mailbox_key=central_global_mailbox_key() OR OLD.recipient->>'kind'='global_supervisor'
   OR (TG_OP='UPDATE' AND (NEW.mailbox_key=central_global_mailbox_key() OR NEW.recipient->>'kind'='global_supervisor')) THEN
   RAISE EXCEPTION 'global supervisor original mail is immutable';
  END IF;
 ELSE
  IF OLD.response->>'contract'='human-supervisor-conversation-1'
   OR (TG_OP='UPDATE' AND NEW.response->>'contract'='human-supervisor-conversation-1') THEN
   RAISE EXCEPTION 'global supervisor original receipt is immutable';
  END IF;
 END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER central_global_mail_row_guard BEFORE UPDATE OR DELETE ON mailbox_messages FOR EACH ROW EXECUTE FUNCTION guard_central_global_mail();
CREATE TRIGGER central_global_mail_truncate_guard BEFORE TRUNCATE ON mailbox_messages FOR EACH STATEMENT EXECUTE FUNCTION guard_central_global_mail();
CREATE TRIGGER central_global_receipt_row_guard BEFORE UPDATE OR DELETE ON mailbox_operations FOR EACH ROW EXECUTE FUNCTION guard_central_global_mail();
CREATE TRIGGER central_global_receipt_truncate_guard BEFORE TRUNCATE ON mailbox_operations FOR EACH STATEMENT EXECUTE FUNCTION guard_central_global_mail();

CREATE TABLE central_supervisor_actors (
 actor_id UUID PRIMARY KEY CHECK(actor_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 conversation_id UUID NOT NULL UNIQUE REFERENCES global_supervisor_conversation(conversation_id),
 revision BIGINT NOT NULL CHECK(revision>0), policy_revision BIGINT NOT NULL CHECK(policy_revision>0),
 policy_bytes TEXT NOT NULL CHECK(octet_length(policy_bytes) BETWEEN 1 AND 4096),
 policy_digest TEXT NOT NULL CHECK(policy_digest=encode(sha256(convert_to(policy_bytes,'UTF8')),'hex')),
 approved_by UUID NOT NULL REFERENCES users(id), approved_identity JSONB NOT NULL,
 current_reservation UUID,
 retained_operations BIGINT NOT NULL CHECK(retained_operations BETWEEN 1 AND 4096),
 retained_bytes BIGINT NOT NULL CHECK(retained_bytes BETWEEN 1 AND 67108864),
 CHECK(revision=retained_operations AND policy_revision<=revision),
 CHECK ((jsonb_typeof(policy_bytes::jsonb)='object'
  AND policy_bytes::jsonb=jsonb_build_object('schema_version',1,'kind','global_supervisor_reservation','actor_id',actor_id::text,'conversation_id',conversation_id::text,'enabled',policy_bytes::jsonb->'enabled')
  AND jsonb_typeof(policy_bytes::jsonb->'enabled')='boolean') IS TRUE),
 CHECK ((jsonb_typeof(approved_identity)='object' AND approved_identity->>'id'=approved_by::text AND approved_identity->>'role'='owner'
  AND jsonb_typeof(approved_identity->'email')='string' AND jsonb_typeof(approved_identity->'issuer') IN ('string','null')
  AND jsonb_typeof(approved_identity->'subject') IN ('string','null')) IS TRUE),
 CHECK (current_reservation IS NULL OR (retained_operations<=4095 AND retained_bytes<=66977792))
);
CREATE TABLE central_supervisor_reservations (
 reservation_id UUID PRIMARY KEY CHECK(reservation_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 actor_id UUID NOT NULL REFERENCES central_supervisor_actors(actor_id),
 reserve_operation_id UUID NOT NULL CHECK(reserve_operation_id<>reservation_id AND reserve_operation_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 policy_revision BIGINT NOT NULL CHECK(policy_revision>0), revision BIGINT NOT NULL CHECK(revision>0),
 state TEXT NOT NULL CHECK(state IN ('awaiting_prepared_plan','cancelled_before_plan')),
 through_cursor BIGINT NOT NULL CHECK(through_cursor>0),
 batch_digest TEXT NOT NULL CHECK(batch_digest ~ '^[0-9a-f]{64}$'),
 message_count SMALLINT NOT NULL CHECK(message_count BETWEEN 1 AND 16),
 body_bytes BIGINT NOT NULL CHECK(body_bytes BETWEEN 1 AND 65536),
 CHECK((state='awaiting_prepared_plan' AND revision=1) OR (state='cancelled_before_plan' AND revision=2)),
 UNIQUE(actor_id,reservation_id), UNIQUE(actor_id,reserve_operation_id)
);
CREATE UNIQUE INDEX central_supervisor_one_active ON central_supervisor_reservations(actor_id) WHERE state='awaiting_prepared_plan';
ALTER TABLE central_supervisor_actors ADD CONSTRAINT central_supervisor_current_reservation FOREIGN KEY(actor_id,current_reservation) REFERENCES central_supervisor_reservations(actor_id,reservation_id);
CREATE TABLE central_supervisor_mail_members (
 actor_id UUID NOT NULL REFERENCES central_supervisor_actors(actor_id),
 reservation_id UUID NOT NULL, ordinal SMALLINT NOT NULL CHECK(ordinal BETWEEN 0 AND 15),
 message_id UUID NOT NULL REFERENCES mailbox_messages(message_id),
 mailbox_key TEXT NOT NULL CHECK(mailbox_key ~ '^[0-9a-f]{64}$'), cursor BIGINT NOT NULL CHECK(cursor>0),
 payload_digest TEXT NOT NULL CHECK(payload_digest ~ '^[0-9a-f]{64}$'),
 PRIMARY KEY(reservation_id,ordinal), UNIQUE(reservation_id,message_id), UNIQUE(reservation_id,cursor),
 FOREIGN KEY(actor_id,reservation_id) REFERENCES central_supervisor_reservations(actor_id,reservation_id)
);
CREATE INDEX central_supervisor_member_message ON central_supervisor_mail_members(actor_id,message_id,reservation_id);
CREATE INDEX central_supervisor_member_cursor ON central_supervisor_mail_members(actor_id,mailbox_key,cursor,reservation_id);
CREATE TABLE central_supervisor_operations (
 actor_id UUID NOT NULL REFERENCES central_supervisor_actors(actor_id),
 operation_id UUID NOT NULL CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 kind TEXT NOT NULL CHECK(kind IN ('configure','reserve','cancel')),
 initiating_user_id UUID NOT NULL REFERENCES users(id),
 request_bytes TEXT NOT NULL CHECK(octet_length(request_bytes) BETWEEN 1 AND 16384),
 request_digest TEXT NOT NULL CHECK(request_digest=encode(sha256(convert_to(request_bytes,'UTF8')),'hex')),
 result_bytes TEXT NOT NULL CHECK(octet_length(result_bytes) BETWEEN 1 AND 65536),
 charged_bytes BIGINT NOT NULL CHECK(charged_bytes BETWEEN octet_length(request_bytes)+octet_length(result_bytes)+8192 AND 131072),
 PRIMARY KEY(actor_id,operation_id),
 CHECK ((jsonb_typeof(request_bytes::jsonb)='object' AND request_bytes::jsonb->>'contract'='central-reservation-1'
  AND request_bytes::jsonb->>'kind'=kind AND request_bytes::jsonb#>>'{request,actor_id}'=actor_id::text
  AND request_bytes::jsonb#>>'{request,operation_id}'=operation_id::text
  AND request_bytes::jsonb#>>'{owner,id}'=initiating_user_id::text AND request_bytes::jsonb#>>'{owner,role}'='owner') IS TRUE),
 CHECK ((jsonb_typeof(result_bytes::jsonb)='object' AND result_bytes::jsonb->>'contract'='central-reservation-1'
  AND result_bytes::jsonb->>'actor_id'=actor_id::text AND result_bytes::jsonb->>'operation_id'=operation_id::text
  AND result_bytes::jsonb->>'initiating_user_id'=initiating_user_id::text
  AND result_bytes::jsonb->>'request_digest'=request_digest
  AND ((kind='configure' AND result_bytes::jsonb->>'disposition'='configured')
   OR (kind='reserve' AND result_bytes::jsonb->>'disposition' IN ('reserved','not_reserved'))
   OR (kind='cancel' AND result_bytes::jsonb->>'disposition' IN ('cancelled_before_plan','not_cancelled')))) IS TRUE)
);
CREATE FUNCTION guard_central_reservation_actor() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'central actor history cannot be removed'; END IF;
 IF ROW(NEW.actor_id,NEW.conversation_id) IS DISTINCT FROM ROW(OLD.actor_id,OLD.conversation_id)
  OR NEW.revision<>OLD.revision+1 OR NEW.policy_revision NOT IN (OLD.policy_revision,OLD.policy_revision+1)
  OR NEW.retained_operations<>OLD.retained_operations+1 OR NEW.retained_bytes<=OLD.retained_bytes THEN
  RAISE EXCEPTION 'central actor identity/revision/budget transition refused';
 END IF;
 IF NEW.policy_revision=OLD.policy_revision AND ROW(NEW.policy_bytes,NEW.policy_digest,NEW.approved_by,NEW.approved_identity)
  IS DISTINCT FROM ROW(OLD.policy_bytes,OLD.policy_digest,OLD.approved_by,OLD.approved_identity) THEN
  RAISE EXCEPTION 'central policy update requires next revision';
 END IF;
 RETURN NEW;
END $$;
CREATE FUNCTION guard_central_reservation_state() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'central reservation history cannot be removed'; END IF;
 IF (to_jsonb(NEW)-ARRAY['state','revision']) IS DISTINCT FROM (to_jsonb(OLD)-ARRAY['state','revision'])
  OR OLD.state<>'awaiting_prepared_plan' OR NEW.state<>'cancelled_before_plan' OR NEW.revision<>OLD.revision+1 THEN
  RAISE EXCEPTION 'central reservation is immutable except one pre-plan cancellation';
 END IF;
 RETURN NEW;
END $$;
CREATE FUNCTION guard_central_reservation_immutable() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'central reservation reference/operation history is immutable'; END $$;
CREATE FUNCTION check_central_reservation_consistency() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE a central_supervisor_actors; r central_supervisor_reservations; actual_count BIGINT; actual_bytes BIGINT; actual_digest TEXT;
BEGIN
 SELECT * INTO a FROM central_supervisor_actors WHERE actor_id=NEW.actor_id;
 IF (SELECT count(*) FROM central_supervisor_reservations WHERE actor_id=a.actor_id AND state='awaiting_prepared_plan')
  <>CASE WHEN a.current_reservation IS NULL THEN 0 ELSE 1 END
  OR (a.current_reservation IS NOT NULL AND NOT EXISTS(SELECT 1 FROM central_supervisor_reservations WHERE actor_id=a.actor_id AND reservation_id=a.current_reservation AND state='awaiting_prepared_plan')) THEN
  RAISE EXCEPTION 'central current reservation is inconsistent';
 END IF;
 IF TG_TABLE_NAME IN ('central_supervisor_actors','central_supervisor_operations') THEN
  SELECT count(*),COALESCE(sum(charged_bytes),0) INTO actual_count,actual_bytes FROM central_supervisor_operations WHERE actor_id=a.actor_id;
  IF actual_count<>a.retained_operations OR actual_bytes<>a.retained_bytes THEN
   RAISE EXCEPTION 'central retained operation budget is inconsistent';
  END IF;
  RETURN NULL;
 END IF;
 SELECT * INTO r FROM central_supervisor_reservations WHERE reservation_id=NEW.reservation_id;
 SELECT count(*),COALESCE(sum(octet_length(m.body)),0),
  encode(sha256(convert_to('{"conversation_id":"'||a.conversation_id::text||'","messages":['||
   COALESCE(string_agg('{"cursor":'||b.cursor::text||',"message_id":"'||b.message_id::text||'","payload_digest":"'||b.payload_digest||'"}',',' ORDER BY b.ordinal),'')||
   '],"through_cursor":'||r.through_cursor::text||'}','UTF8')),'hex') INTO actual_count,actual_bytes,actual_digest
  FROM central_supervisor_mail_members b JOIN mailbox_messages m ON m.message_id=b.message_id
  WHERE b.reservation_id=r.reservation_id;
 IF actual_count<>r.message_count OR actual_bytes<>r.body_bytes OR actual_digest<>r.batch_digest
  OR EXISTS(SELECT 1 FROM (SELECT cursor,lag(cursor) OVER(ORDER BY ordinal) AS previous FROM central_supervisor_mail_members WHERE reservation_id=r.reservation_id) ordered WHERE previous>=cursor)
  OR EXISTS(SELECT 1 FROM central_supervisor_mail_members b JOIN mailbox_messages m ON m.message_id=b.message_id
   WHERE b.reservation_id=r.reservation_id AND (b.ordinal>=r.message_count OR b.actor_id<>r.actor_id OR b.mailbox_key<>m.mailbox_key OR b.cursor<>m.cursor OR b.payload_digest<>m.payload_digest
    OR b.mailbox_key<>central_global_mailbox_key() OR b.cursor>r.through_cursor OR NOT central_global_mail_source_valid(m.message_id)))
  OR NOT EXISTS(SELECT 1 FROM central_supervisor_operations o WHERE o.actor_id=r.actor_id AND o.operation_id=r.reserve_operation_id AND o.kind='reserve' AND o.result_bytes::jsonb->>'disposition'='reserved') THEN
  RAISE EXCEPTION 'central original reservation member proof is inconsistent';
 END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER central_actor_row_guard BEFORE UPDATE OR DELETE ON central_supervisor_actors FOR EACH ROW EXECUTE FUNCTION guard_central_reservation_actor();
CREATE TRIGGER central_actor_truncate_guard BEFORE TRUNCATE ON central_supervisor_actors FOR EACH STATEMENT EXECUTE FUNCTION guard_central_reservation_actor();
CREATE TRIGGER central_reservation_row_guard BEFORE UPDATE OR DELETE ON central_supervisor_reservations FOR EACH ROW EXECUTE FUNCTION guard_central_reservation_state();
CREATE TRIGGER central_reservation_truncate_guard BEFORE TRUNCATE ON central_supervisor_reservations FOR EACH STATEMENT EXECUTE FUNCTION guard_central_reservation_state();
CREATE TRIGGER central_member_row_guard BEFORE UPDATE OR DELETE ON central_supervisor_mail_members FOR EACH ROW EXECUTE FUNCTION guard_central_reservation_immutable();
CREATE TRIGGER central_member_truncate_guard BEFORE TRUNCATE ON central_supervisor_mail_members FOR EACH STATEMENT EXECUTE FUNCTION guard_central_reservation_immutable();
CREATE TRIGGER central_operation_row_guard BEFORE UPDATE OR DELETE ON central_supervisor_operations FOR EACH ROW EXECUTE FUNCTION guard_central_reservation_immutable();
CREATE TRIGGER central_operation_truncate_guard BEFORE TRUNCATE ON central_supervisor_operations FOR EACH STATEMENT EXECUTE FUNCTION guard_central_reservation_immutable();
CREATE CONSTRAINT TRIGGER central_operation_consistency AFTER INSERT ON central_supervisor_operations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION check_central_reservation_consistency();
CREATE CONSTRAINT TRIGGER central_actor_consistency AFTER INSERT OR UPDATE ON central_supervisor_actors DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION check_central_reservation_consistency();
CREATE CONSTRAINT TRIGGER central_reservation_consistency AFTER INSERT OR UPDATE ON central_supervisor_reservations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION check_central_reservation_consistency();
CREATE CONSTRAINT TRIGGER central_member_consistency AFTER INSERT ON central_supervisor_mail_members DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION check_central_reservation_consistency();
UPDATE meta SET value='37' WHERE key='state_format_version';
