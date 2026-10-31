-- AQ08 common identity bridge; actual profiled grants remain separately gated.
ALTER TABLE execution_attempts
 ADD COLUMN kind TEXT NOT NULL DEFAULT 'legacy_fixture'
   CHECK (kind IN ('legacy_fixture','profiled_v1')),
 ADD COLUMN record_revision BIGINT NOT NULL DEFAULT 1 CHECK(record_revision>0),
 ADD COLUMN authority_revision BIGINT NOT NULL DEFAULT 1 CHECK(authority_revision>0),
 ADD COLUMN grant_revision BIGINT NOT NULL DEFAULT 0 CHECK(grant_revision>=0),
 ADD COLUMN execution_deadline_ms BIGINT CHECK(execution_deadline_ms>0);
ALTER TABLE execution_attempts ADD CONSTRAINT execution_profiled_state_shape
 CHECK (kind='legacy_fixture' OR
   (jsonb_typeof(state)='object' AND state ? 'schema_version'
    AND state->>'schema_version'='1' AND state ? 'status'));
ALTER TABLE execution_attempts ADD CONSTRAINT execution_state_bytes CHECK(octet_length(state::text)<=4194304);
CREATE INDEX execution_kind_node ON execution_attempts(kind,node_id,attempt_id);

CREATE TABLE admission_source_policies (
 project_id UUID PRIMARY KEY REFERENCES project_execution_bindings(project_id),
 revision BIGINT NOT NULL CHECK(revision>0),
 policy_digest TEXT NOT NULL CHECK(policy_digest ~ '^[0-9a-f]{64}$'),
 policy JSONB NOT NULL CHECK(octet_length(policy::text)<=65536)
);
CREATE TABLE admission_source_observations (
 observation_id UUID PRIMARY KEY,
 project_id UUID NOT NULL REFERENCES project_execution_bindings(project_id),
 source_policy_revision BIGINT NOT NULL CHECK(source_policy_revision>0),
 observation JSONB NOT NULL CHECK(octet_length(observation::text)<=65536),
 observed_at_ms BIGINT NOT NULL CHECK(observed_at_ms>0),
 expires_at_ms BIGINT NOT NULL CHECK(expires_at_ms>observed_at_ms)
);

CREATE TABLE admission_artifacts (
 artifact_id UUID PRIMARY KEY,
 descriptor JSONB NOT NULL CHECK(octet_length(descriptor::text)<=65536),
 content_digest TEXT NOT NULL CHECK(content_digest ~ '^[0-9a-f]{64}$'),
 byte_length BIGINT NOT NULL CHECK(byte_length>0 AND byte_length<=1073741824),
 owner_kind TEXT NOT NULL CHECK(owner_kind IN
   ('catalog','graph_snapshot','composition','selection','outcome','source_retention')),
 owner_key TEXT NOT NULL CHECK(octet_length(owner_key) BETWEEN 1 AND 512),
 -- Actual central bytes only for bounded nonsecret snapshots. Large Git source
 -- remains with the existing source owner and its real retention receipt.
 body BYTEA CHECK(octet_length(body) BETWEEN 1 AND 2097152),
 created_at_ms BIGINT NOT NULL CHECK(created_at_ms>0),
 retain_until_ms BIGINT NOT NULL CHECK(retain_until_ms>created_at_ms)
);
CREATE INDEX admission_artifact_digest ON admission_artifacts(content_digest);

CREATE TABLE admission_material_leases (
 lease_id UUID PRIMARY KEY,
 operation_id UUID NOT NULL,
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 node_id UUID NOT NULL REFERENCES nodes(id),
 boot_id UUID NOT NULL,
 prospective_attempt_id UUID NOT NULL,
 admitted_attempt_id UUID REFERENCES execution_attempts(attempt_id),
 offer_id UUID NOT NULL,
 credential_identity TEXT NOT NULL CHECK(octet_length(credential_identity) BETWEEN 1 AND 512),
 credential_revision BIGINT NOT NULL CHECK(credential_revision>0),
 purpose TEXT NOT NULL CHECK(octet_length(purpose) BETWEEN 1 AND 512),
 lease JSONB NOT NULL CHECK(octet_length(lease::text)<=65536),
 verifier TEXT NOT NULL UNIQUE CHECK(octet_length(verifier)=64 AND verifier ~ '^[0-9a-f]{64}$'),
 state TEXT NOT NULL CHECK(state IN ('pending','admitted','revoked','expired')),
 issued_at_ms BIGINT NOT NULL CHECK(issued_at_ms>0),
 expires_at_ms BIGINT NOT NULL CHECK(expires_at_ms>issued_at_ms
   AND expires_at_ms-issued_at_ms<=120000),
 CHECK(admitted_attempt_id IS NULL OR admitted_attempt_id=prospective_attempt_id),
 UNIQUE(node_id,operation_id)
);
CREATE UNIQUE INDEX admission_one_live_material_purpose
 ON admission_material_leases(offer_id,credential_identity,purpose)
 WHERE state IN ('pending','admitted');
CREATE TABLE admission_material_receipts (
 lease_id UUID PRIMARY KEY REFERENCES admission_material_leases(lease_id),
 operation_id UUID NOT NULL,
 receipt_digest TEXT NOT NULL CHECK(receipt_digest ~ '^[0-9a-f]{64}$'),
 receipt JSONB NOT NULL CHECK(octet_length(receipt::text)<=65536)
);

CREATE TABLE profiled_launches (
 attempt_id UUID PRIMARY KEY REFERENCES execution_attempts(attempt_id),
 decision_id UUID NOT NULL REFERENCES intelligence_decisions(decision_id),
 source_observation_id UUID NOT NULL REFERENCES admission_source_observations(observation_id),
 launch_binding JSONB NOT NULL CHECK(octet_length(launch_binding::text)<=65536),
 launch_manifest JSONB NOT NULL CHECK(octet_length(launch_manifest::text)<=2097152),
 manifest_digest TEXT NOT NULL CHECK(manifest_digest ~ '^[0-9a-f]{64}$'),
 execution_spec JSONB NOT NULL CHECK(octet_length(execution_spec::text)<=2097152),
 execution_spec_digest TEXT NOT NULL CHECK(execution_spec_digest ~ '^[0-9a-f]{64}$'),
 graph_task_observation JSONB NOT NULL CHECK(octet_length(graph_task_observation::text)<=1048576),
 source_binding JSONB NOT NULL CHECK(octet_length(source_binding::text)<=65536),
 offer_expires_at_ms BIGINT NOT NULL CHECK(offer_expires_at_ms>0)
);
CREATE TABLE profiled_launch_artifacts (
 attempt_id UUID NOT NULL REFERENCES profiled_launches(attempt_id),
 artifact_id UUID NOT NULL REFERENCES admission_artifacts(artifact_id),
 PRIMARY KEY(attempt_id,artifact_id)
);
CREATE TABLE profiled_launch_materials (
 attempt_id UUID NOT NULL REFERENCES profiled_launches(attempt_id),
 lease_id UUID NOT NULL REFERENCES admission_material_leases(lease_id),
 PRIMARY KEY(attempt_id,lease_id)
);
CREATE TABLE profiled_execution_receipts (
 node_id UUID NOT NULL REFERENCES nodes(id),
 operation_id UUID NOT NULL,
 kind TEXT NOT NULL CHECK(kind IN
  ('accept','prepare','renew','observation','stop','result','reconcile','cancel')),
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 response JSONB NOT NULL CHECK(octet_length(response::text)<=262144),
 PRIMARY KEY(node_id,operation_id)
);
CREATE TABLE scheduler_projects (
 project_id UUID PRIMARY KEY REFERENCES project_execution_bindings(project_id),
 policy_revision BIGINT NOT NULL CHECK(policy_revision>0),
 policy JSONB NOT NULL CHECK(octet_length(policy::text)<=65536),
 next_graph_cursor TEXT CHECK(octet_length(next_graph_cursor)<=4096)
);
CREATE TABLE scheduler_cursor (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 revision BIGINT NOT NULL CHECK(revision>0),
 after_project UUID REFERENCES project_execution_bindings(project_id)
);
INSERT INTO scheduler_cursor(singleton,revision) VALUES(TRUE,1);
CREATE FUNCTION execution_freeze_identity() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 NEW.record_revision:=OLD.record_revision;
 NEW.authority_revision:=OLD.authority_revision;
 NEW.grant_revision:=OLD.grant_revision;
 IF ROW(NEW.attempt_id,NEW.task_id,NEW.node_id,NEW.offer_id,NEW.session_id,NEW.kind)
    IS DISTINCT FROM ROW(OLD.attempt_id,OLD.task_id,OLD.node_id,OLD.offer_id,OLD.session_id,OLD.kind)
 THEN RAISE EXCEPTION 'immutable execution identity' USING ERRCODE='23514'; END IF;
 IF OLD.kind='legacy_fixture' AND
   (NEW.state->'offer'->'identity' IS DISTINCT FROM OLD.state->'offer'->'identity'
    OR NEW.state->'offer'->'attempt' IS DISTINCT FROM OLD.state->'offer'->'attempt')
 THEN RAISE EXCEPTION 'immutable legacy boot/generation' USING ERRCODE='23514'; END IF;
 IF OLD.kind='profiled_v1' AND
   (NEW.state->'offer' IS DISTINCT FROM OLD.state->'offer'
    OR NEW.state->'source' IS DISTINCT FROM OLD.state->'source'
    OR NEW.state->'graph' IS DISTINCT FROM OLD.state->'graph')
 THEN RAISE EXCEPTION 'immutable profiled launch' USING ERRCODE='23514'; END IF;
 IF OLD.execution_deadline_ms IS NOT NULL AND NEW.execution_deadline_ms IS DISTINCT FROM OLD.execution_deadline_ms
 THEN RAISE EXCEPTION 'immutable execution deadline' USING ERRCODE='23514'; END IF;
 IF NEW.state IS DISTINCT FROM OLD.state OR NEW.holds_task IS DISTINCT FROM OLD.holds_task
 THEN NEW.record_revision:=OLD.record_revision+1; END IF;
 IF NEW.state->'status' IS DISTINCT FROM OLD.state->'status'
   OR NEW.state->'cancellation_id' IS DISTINCT FROM OLD.state->'cancellation_id'
   OR NEW.state->'revoked' IS DISTINCT FROM OLD.state->'revoked'
   OR NEW.state->'stopped' IS DISTINCT FROM OLD.state->'stopped'
   OR NEW.state->'stop' IS DISTINCT FROM OLD.state->'stop'
   OR (NEW.kind='profiled_v1' AND NEW.state->'grant'->'grant_id' IS DISTINCT FROM OLD.state->'grant'->'grant_id')
 THEN NEW.authority_revision:=OLD.authority_revision+1; END IF;
 IF NEW.state->'grant' IS DISTINCT FROM OLD.state->'grant'
 THEN NEW.grant_revision:=OLD.grant_revision+1; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER execution_identity_guard BEFORE UPDATE ON execution_attempts
 FOR EACH ROW EXECUTE FUNCTION execution_freeze_identity();
CREATE FUNCTION profiled_launch_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW IS DISTINCT FROM OLD THEN RAISE EXCEPTION 'immutable profiled launch artifact' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_launch_guard BEFORE UPDATE ON profiled_launches
 FOR EACH ROW EXECUTE FUNCTION profiled_launch_immutable();
UPDATE meta SET value='10' WHERE key='state_format_version';
