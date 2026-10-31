-- Candidate only until ordered native0025 is registered. No fallback/gap repair.
-- Review owner receipts grant no graph/task completion eligibility.
CREATE TABLE review_owner_budget (
 singleton boolean PRIMARY KEY CHECK(singleton),
 retained_operations bigint NOT NULL CHECK(retained_operations BETWEEN 0 AND 10000),
 retained_bytes bigint NOT NULL CHECK(retained_bytes BETWEEN 0 AND 67108864)
);
INSERT INTO review_owner_budget VALUES(true,0,0);
CREATE TABLE review_lineage_budget (
 project_id uuid NOT NULL, workspace_id uuid NOT NULL, task_id uuid NOT NULL,
 retained_operations bigint NOT NULL CHECK(retained_operations BETWEEN 0 AND 256),
 PRIMARY KEY(project_id,workspace_id,task_id)
);
CREATE TABLE review_subjects (
 subject_digest text PRIMARY KEY CHECK(subject_digest ~ '^[0-9a-f]{64}$'),
 report_id uuid NOT NULL REFERENCES profiled_result_reports(report_id),
 project_id uuid NOT NULL, workspace_id uuid NOT NULL, task_id uuid NOT NULL,
 decision_count bigint NOT NULL DEFAULT 0 CHECK(decision_count BETWEEN 0 AND 16),
 sealed_version integer NOT NULL CHECK(sealed_version=1),
 key_id text NOT NULL CHECK(octet_length(key_id) BETWEEN 1 AND 64),
 nonce bytea NOT NULL CHECK(octet_length(nonce)=12),
 ciphertext bytea NOT NULL CHECK(octet_length(ciphertext) BETWEEN 16 AND 65552)
);
CREATE TABLE human_decision_requests (
 decision_id uuid PRIMARY KEY,
 requester_id uuid NOT NULL REFERENCES users(id),
 operation_id uuid NOT NULL,
 request_digest text NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 subject_digest text NOT NULL REFERENCES review_subjects(subject_digest),
 decision_revision bigint NOT NULL CHECK(decision_revision IN (1,2)),
 state text NOT NULL CHECK(state IN ('pending','answered','expired','cancelled')),
 expires_at_ms bigint NOT NULL CHECK(expires_at_ms>0),
 sealed_version integer NOT NULL CHECK(sealed_version=1),
 key_id text NOT NULL CHECK(octet_length(key_id) BETWEEN 1 AND 64),
 nonce bytea NOT NULL CHECK(octet_length(nonce)=12),
 ciphertext bytea NOT NULL CHECK(octet_length(ciphertext) BETWEEN 16 AND 65552),
 UNIQUE(requester_id,operation_id),
 CHECK((decision_revision=1)=(state='pending'))
);
CREATE TABLE human_decision_finality (
 decision_id uuid PRIMARY KEY REFERENCES human_decision_requests(decision_id),
 actor_id uuid REFERENCES users(id),
 operation_id uuid NOT NULL,
 request_digest text NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 receipt_digest text NOT NULL CHECK(receipt_digest ~ '^[0-9a-f]{64}$'),
 sealed_version integer NOT NULL CHECK(sealed_version=1),
 key_id text NOT NULL CHECK(octet_length(key_id) BETWEEN 1 AND 64),
 nonce bytea NOT NULL CHECK(octet_length(nonce)=12),
 ciphertext bytea NOT NULL CHECK(octet_length(ciphertext) BETWEEN 16 AND 65552)
);
CREATE UNIQUE INDEX human_decision_actor_operation ON human_decision_finality(actor_id,operation_id);
CREATE TABLE review_owner_outbox (
 event_id uuid PRIMARY KEY,
 decision_id uuid NOT NULL UNIQUE REFERENCES human_decision_finality(decision_id),
 producer text NOT NULL CHECK(producer='aq.human.decisions@1'),
 receipt_digest text NOT NULL CHECK(receipt_digest ~ '^[0-9a-f]{64}$'),
 acknowledged boolean NOT NULL DEFAULT false,
 sealed_version integer NOT NULL CHECK(sealed_version=1),
 key_id text NOT NULL CHECK(octet_length(key_id) BETWEEN 1 AND 64),
 nonce bytea NOT NULL CHECK(octet_length(nonce)=12),
 ciphertext bytea NOT NULL CHECK(octet_length(ciphertext) BETWEEN 16 AND 65552)
);
CREATE FUNCTION review_immutable_record() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'immutable review owner record'; END $$;
CREATE TRIGGER review_finality_immutable BEFORE UPDATE OR DELETE ON human_decision_finality
 FOR EACH ROW EXECUTE FUNCTION review_immutable_record();
-- Subject decision_count is mutable only as a quota counter; identities/bytes are fixed.
CREATE FUNCTION review_subject_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF ROW(NEW.subject_digest,NEW.report_id,NEW.project_id,NEW.workspace_id,NEW.task_id,NEW.sealed_version,NEW.key_id,NEW.nonce,NEW.ciphertext)
 IS DISTINCT FROM ROW(OLD.subject_digest,OLD.report_id,OLD.project_id,OLD.workspace_id,OLD.task_id,OLD.sealed_version,OLD.key_id,OLD.nonce,OLD.ciphertext)
 THEN RAISE EXCEPTION 'immutable review subject'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER review_subject_immutable BEFORE UPDATE ON review_subjects
 FOR EACH ROW EXECUTE FUNCTION review_subject_immutable();
UPDATE meta SET value='26' WHERE key='state_format_version';
