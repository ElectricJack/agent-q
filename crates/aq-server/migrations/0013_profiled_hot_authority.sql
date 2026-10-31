-- Additive migration: 0010 remains immutable historical compatibility.
-- Runtime producers must verify actual stop/source/effect owner receipts before
-- clearing the common task claim. JSON receipt references cannot prove release.
CREATE TABLE profiled_authority_hot (
 attempt_id UUID PRIMARY KEY REFERENCES profiled_launches(attempt_id),
 state JSONB NOT NULL CHECK(jsonb_typeof(state)='object' AND octet_length(state::text)<=16384),
 record_revision BIGINT NOT NULL CHECK(record_revision>0),
 authority_revision BIGINT NOT NULL CHECK(authority_revision>0),
 grant_revision BIGINT NOT NULL CHECK(grant_revision>=0)
);
-- Written only by the internal release owner after authenticating the exact
-- qualified host journal and verifying actual central source/effect receipts.
-- No HTTP DTO or projection can create this record. Until that producer exists,
-- adopted task claims remain held even when structural projections look complete.
CREATE TABLE profiled_verified_releases (
 attempt_id UUID PRIMARY KEY REFERENCES profiled_authority_hot(attempt_id),
 release_id UUID NOT NULL UNIQUE,
 stop_operation_id UUID NOT NULL,
 stop_evidence_digest TEXT NOT NULL CHECK(stop_evidence_digest ~ '^[0-9a-f]{64}$'),
 host_owner_qualification JSONB NOT NULL CHECK(jsonb_typeof(host_owner_qualification)='object' AND octet_length(host_owner_qualification::text)<=4096),
 source_receipt_digest TEXT NOT NULL CHECK(source_receipt_digest ~ '^[0-9a-f]{64}$'),
 effect_receipt_digest TEXT NOT NULL CHECK(effect_receipt_digest ~ '^[0-9a-f]{64}$'),
 verified_at_ms BIGINT NOT NULL CHECK(verified_at_ms>0)
);
CREATE FUNCTION profiled_hot_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE base execution_attempts%ROWTYPE; previous_status TEXT; current_status TEXT; current_stop TEXT;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'immutable hot authority adoption' USING ERRCODE='23514'; END IF;
 current_status:=NEW.state->>'status';
 current_stop:=COALESCE(NEW.state#>>'{stop,kind}','');
 IF NEW.state->>'schema_version' IS DISTINCT FROM '1'
    OR NOT (NEW.state ?& ARRAY['schema_version','status','grant','execution_deadline_ms','owner','cancellation_id','revoked','stop','retention','effects'])
    OR jsonb_typeof(NEW.state->'revoked') IS DISTINCT FROM 'boolean'
    OR current_status IS NULL OR current_status NOT IN ('offered','preparing','granted','cancelling','finished','declined','unknown')
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
   IF (previous_status IN ('finished','declined') AND current_status<>previous_status)
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
 IF (current_status='offered' AND (NEW.state->'owner'<>'null'::jsonb OR NEW.state->'grant'<>'null'::jsonb OR current_stop<>'unknown'))
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
CREATE TRIGGER profiled_hot_guard BEFORE INSERT OR UPDATE OR DELETE ON profiled_authority_hot
 FOR EACH ROW EXECUTE FUNCTION profiled_hot_guard();

CREATE FUNCTION profiled_historical_snapshot_guard() RETURNS trigger LANGUAGE plpgsql AS $$
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
 THEN RAISE EXCEPTION 'actual owner-verified release record required' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_historical_snapshot_guard BEFORE UPDATE ON execution_attempts
 FOR EACH ROW EXECUTE FUNCTION profiled_historical_snapshot_guard();

-- Every immutable row used by the verified-facts cache is guarded, including
-- deletion/recreation under the same key. No provider health or ACL is cached.
CREATE FUNCTION profiled_sealed_row_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'sealed admission record cannot be deleted' USING ERRCODE='23514'; END IF;
 IF NEW IS DISTINCT FROM OLD THEN RAISE EXCEPTION 'sealed admission record cannot change' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_launch_delete_guard BEFORE DELETE ON profiled_launches
 FOR EACH ROW EXECUTE FUNCTION profiled_sealed_row_guard();
CREATE TRIGGER profiled_selection_guard BEFORE UPDATE OR DELETE ON intelligence_attempt_selections
 FOR EACH ROW EXECUTE FUNCTION profiled_sealed_row_guard();
CREATE TRIGGER profiled_decision_guard BEFORE UPDATE OR DELETE ON intelligence_decisions
 FOR EACH ROW EXECUTE FUNCTION profiled_sealed_row_guard();
CREATE FUNCTION profiled_release_completeness_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE current_hot JSONB;
BEGIN
 SELECT state INTO STRICT current_hot FROM profiled_authority_hot WHERE attempt_id=NEW.attempt_id;
 IF (current_hot->>'status' IN ('finished','declined')
     AND current_hot#>>'{stop,kind}' IN ('not_started','stopped')
     AND jsonb_typeof(current_hot->'owner')='object'
     AND (current_hot#>>'{retention,kind}'='retained' OR (current_hot#>>'{retention,kind}'='not_required' AND current_hot#>>'{stop,kind}'='not_started'))
     AND current_hot#>>'{effects,kind}'='quiescent'
     AND NEW.stop_evidence_digest=current_hot#>>'{stop,receipt,digest}'
     AND NEW.effect_receipt_digest=current_hot#>>'{effects,receipt,digest}'
     AND (current_hot#>>'{retention,kind}'='not_required' OR NEW.source_receipt_digest=current_hot#>>'{retention,receipt,digest}')) IS NOT TRUE
 THEN RAISE EXCEPTION 'release requires terminal owner/stop/source/effect completeness' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_release_completeness_guard BEFORE INSERT ON profiled_verified_releases
 FOR EACH ROW EXECUTE FUNCTION profiled_release_completeness_guard();
CREATE TRIGGER profiled_verified_release_guard BEFORE UPDATE OR DELETE ON profiled_verified_releases
 FOR EACH ROW EXECUTE FUNCTION profiled_sealed_row_guard();
UPDATE meta SET value='13' WHERE key='state_format_version';
