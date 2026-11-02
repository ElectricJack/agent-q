-- Claims only. Phase release remains closed until B2/C owners qualify.
-- Never relabel unqualified preexisting phase rows as legacy on upgrade.
DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM profiled_result_reports r WHERE EXISTS(SELECT 1 FROM profiled_v2_accepts a WHERE a.attempt_id=r.attempt_id) OR EXISTS(SELECT 1 FROM profiled_phase_execution_grants g WHERE g.attempt_id=r.attempt_id)) THEN
  RAISE EXCEPTION 'unqualified phase result rows cannot be backfilled';
 END IF;
END $$;
ALTER TABLE profiled_phase_execution_grants ADD CONSTRAINT profiled_phase_result_execution_key UNIQUE(attempt_id,receipt_digest);
ALTER TABLE profiled_stopped_receipts ADD CONSTRAINT profiled_phase_result_stop_key UNIQUE(attempt_id,phase_ack_digest);
ALTER TABLE profiled_result_reports
 ADD COLUMN result_origin_kind TEXT NOT NULL DEFAULT 'legacy_prepare_v1',
 ADD COLUMN origin_execution_receipt_digest TEXT,
 ADD COLUMN origin_stopped_ack_bytes_digest TEXT,
 ADD COLUMN result_origin_phase_attempt_id UUID GENERATED ALWAYS AS (CASE WHEN result_origin_kind='legacy_prepare_v1' THEN NULL ELSE attempt_id END) STORED,
 ADD CONSTRAINT profiled_result_origin_check CHECK(COALESCE(
  (result_origin_kind='legacy_prepare_v1' AND origin_execution_receipt_digest IS NULL AND origin_stopped_ack_bytes_digest IS NULL)
  OR (result_origin_kind IN ('phase_execution_v2','phase_refresh_execution_v1') AND origin_execution_receipt_digest ~ '^[0-9a-f]{64}$' AND origin_stopped_ack_bytes_digest ~ '^[0-9a-f]{64}$'),FALSE)),
 ADD CONSTRAINT profiled_result_execution_origin_fk FOREIGN KEY(result_origin_phase_attempt_id,origin_execution_receipt_digest) REFERENCES profiled_phase_execution_grants(attempt_id,receipt_digest),
 ADD CONSTRAINT profiled_result_stop_origin_fk FOREIGN KEY(result_origin_phase_attempt_id,origin_stopped_ack_bytes_digest) REFERENCES profiled_stopped_receipts(attempt_id,phase_ack_digest);
CREATE FUNCTION profiled_result_origin_valid(attempt UUID,owner_node UUID,origin_kind TEXT,execution_digest TEXT,stopped_ack_digest TEXT) RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT EXISTS(
  SELECT 1 FROM profiled_phase_execution_grants g
  JOIN profiled_v2_proposals p USING(attempt_id)
  JOIN profiled_started_receipts started USING(attempt_id)
  JOIN profiled_stopped_receipts stopped USING(attempt_id)
  WHERE g.attempt_id=attempt AND g.node_id=owner_node AND started.node_id=owner_node AND stopped.node_id=owner_node
   AND g.receipt_digest=execution_digest AND stopped.phase_ack_digest=stopped_ack_digest
   AND started.initial_authority_kind='phase_execution_v2' AND stopped.initial_authority_kind='phase_execution_v2'
   AND started.initial_authority_receipt_digest=g.receipt_digest AND stopped.initial_authority_receipt_digest=g.receipt_digest
   AND started.phase_terminal_qualification=stopped.phase_terminal_qualification
   AND convert_from(started.phase_envelope_bytes,'UTF8')::JSONB->'negotiation'=convert_from(stopped.phase_envelope_bytes,'UTF8')::JSONB->'negotiation'
   AND convert_from(started.phase_ack_bytes,'UTF8')::JSONB->'negotiation'=convert_from(started.phase_envelope_bytes,'UTF8')::JSONB->'negotiation'
   AND convert_from(stopped.phase_ack_bytes,'UTF8')::JSONB->'negotiation'=convert_from(stopped.phase_envelope_bytes,'UTF8')::JSONB->'negotiation'
   AND convert_from(stopped.phase_ack_bytes,'UTF8')::JSONB#>>'{origin,execution_receipt_digest}'=g.receipt_digest
   AND convert_from(stopped.phase_ack_bytes,'UTF8')::JSONB#>>'{origin,accepted_receipt_digest}'=g.accepted_receipt_digest
   AND convert_from(stopped.phase_ack_bytes,'UTF8')::JSONB->'original'=convert_from(stopped.receipt_bytes,'UTF8')::JSONB
   AND ((origin_kind='phase_execution_v2' AND p.proposal#>'{negotiation,contract}'='{"digest":"ff60400da09f3de166b7931bc386c89218cb76a6c9e5276dff37c5fce28ca3e5","id":"aq.admission.preparation-v2-contract","version":"1"}'::JSONB AND convert_from(stopped.phase_ack_bytes,'UTF8')::JSONB#>'{negotiation,contract}'='{"digest":"292c30eba95b8dd6f1d9a7bf75775bc5d0f9ecb54644047747f9593d538c0a4a","id":"aq.phase-terminal.contract","version":"1"}'::JSONB)
    OR (origin_kind='phase_refresh_execution_v1' AND p.proposal#>'{negotiation,contract}'='{"digest":"e41c08e3c8200f84b75824bff779833c9b8e5279ac63d2d91af12d9879deef56","id":"aq.admission.preparation-registry-refresh-v1-contract","version":"1"}'::JSONB AND convert_from(stopped.phase_ack_bytes,'UTF8')::JSONB#>'{negotiation,contract}'='{"digest":"f9af55c49127a8cb93f03ef5bbb28313d13f94a221affa3e26d91d45bffd6c2d","id":"aq.phase-terminal.registry-refresh-v1.contract","version":"1"}'::JSONB))
 )
$$;
CREATE OR REPLACE FUNCTION profiled_phase_unqualified_result_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE phase BOOLEAN;
BEGIN
 phase:=EXISTS(SELECT 1 FROM profiled_v2_accepts WHERE attempt_id=NEW.attempt_id) OR EXISTS(SELECT 1 FROM profiled_phase_execution_grants WHERE attempt_id=NEW.attempt_id);
 -- Release remains a separate closed authority boundary, including new writers.
 IF TG_TABLE_NAME='profiled_verified_releases' THEN
  IF phase THEN RAISE EXCEPTION 'phase settlement protocol not qualified' USING ERRCODE='23514'; END IF;
  RETURN NEW;
 END IF;
 IF phase THEN
  IF current_setting('transaction_isolation') NOT IN ('read committed','serializable')
   OR NOT profiled_result_origin_valid(NEW.attempt_id,NEW.node_id,NEW.result_origin_kind,NEW.origin_execution_receipt_digest,NEW.origin_stopped_ack_bytes_digest)
   OR current_setting('aq.result_origin',true) IS DISTINCT FROM 'aq.result-origin.1:'||NEW.result_origin_kind||':'||NEW.origin_execution_receipt_digest||':'||NEW.origin_stopped_ack_bytes_digest
  THEN RAISE EXCEPTION 'qualified exact phase result origin required' USING ERRCODE='23514'; END IF;
 ELSIF NEW.result_origin_kind<>'legacy_prepare_v1' OR NEW.origin_execution_receipt_digest IS NOT NULL OR NEW.origin_stopped_ack_bytes_digest IS NOT NULL THEN
  RAISE EXCEPTION 'legacy result cannot claim phase origin' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
UPDATE meta SET value='38' WHERE key='state_format_version';
