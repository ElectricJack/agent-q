-- Bounded immutable inline artifact ownership. Source bundles remain external.
CREATE TABLE admission_inline_receipts (
 artifact_id UUID PRIMARY KEY REFERENCES admission_artifacts(artifact_id),
 receipt_bytes BYTEA NOT NULL CHECK(octet_length(receipt_bytes) BETWEEN 1 AND 8192),
 receipt_digest TEXT NOT NULL CHECK(receipt_digest=encode(sha256(receipt_bytes),'hex'))
);
CREATE INDEX admission_artifact_owner ON admission_artifacts(owner_key,artifact_id);
CREATE FUNCTION admission_inline_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'immutable admission artifact owner'; END $$;
CREATE TRIGGER admission_artifact_immutable BEFORE UPDATE OR DELETE ON admission_artifacts
 FOR EACH ROW EXECUTE FUNCTION admission_inline_immutable();
CREATE TRIGGER admission_inline_receipt_immutable BEFORE UPDATE OR DELETE ON admission_inline_receipts
 FOR EACH ROW EXECUTE FUNCTION admission_inline_immutable();
CREATE TRIGGER admission_artifact_link_immutable BEFORE UPDATE OR DELETE ON profiled_launch_artifacts
 FOR EACH ROW EXECUTE FUNCTION admission_inline_immutable();
CREATE FUNCTION admission_artifact_insert_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE total_count BIGINT; total_bytes BIGINT; project_count BIGINT; project_bytes BIGINT;
BEGIN
 IF current_setting('transaction_isolation') NOT IN ('read committed','serializable') THEN RAISE EXCEPTION 'artifact quota requires current predicate visibility'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('aq-inline-artifact-quota-v1',0));
 SELECT count(*),COALESCE(sum(octet_length(body)),0) INTO total_count,total_bytes FROM admission_artifacts;
 SELECT count(*),COALESCE(sum(octet_length(body)),0) INTO project_count,project_bytes FROM admission_artifacts WHERE owner_key=NEW.owner_key;
 IF total_count>=4096 OR project_count>=512 OR total_bytes+COALESCE(octet_length(NEW.body),0)>268435456
    OR project_bytes+COALESCE(octet_length(NEW.body),0)>67108864 THEN RAISE EXCEPTION 'admission artifact quota'; END IF;
 IF NEW.body IS NULL AND (NEW.owner_kind<>'source_retention' OR NEW.descriptor->>'kind' IS DISTINCT FROM 'source') THEN RAISE EXCEPTION 'non-source artifact requires inline bytes'; END IF;
 IF NEW.body IS NOT NULL AND (
    NEW.byte_length<>octet_length(NEW.body) OR NEW.content_digest<>encode(sha256(NEW.body),'hex')
    OR NEW.descriptor->'artifact'->>'digest' IS DISTINCT FROM NEW.content_digest
    OR (NEW.descriptor->>'bytes')::BIGINT IS DISTINCT FROM NEW.byte_length
    OR (NEW.descriptor->>'retained_at_ms')::BIGINT IS DISTINCT FROM NEW.created_at_ms
    OR (NEW.descriptor->>'retain_until_ms')::BIGINT IS DISTINCT FROM NEW.retain_until_ms
    OR NEW.descriptor->>'kind' IS NULL
    OR NEW.descriptor->>'kind' NOT IN ('context','composition','selection','definition','outcome')
    OR NEW.owner_key !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
 ) THEN RAISE EXCEPTION 'inline artifact byte/descriptor mismatch'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER admission_artifact_insert BEFORE INSERT ON admission_artifacts
 FOR EACH ROW EXECUTE FUNCTION admission_artifact_insert_guard();
CREATE FUNCTION admission_inline_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE a admission_artifacts%ROWTYPE; r admission_inline_receipts%ROWTYPE; proof JSONB;
BEGIN
 SELECT * INTO a FROM admission_artifacts WHERE artifact_id=NEW.artifact_id;
 IF a.body IS NULL THEN RETURN NULL; END IF;
 SELECT * INTO r FROM admission_inline_receipts WHERE artifact_id=a.artifact_id;
 IF NOT FOUND THEN RAISE EXCEPTION 'inline artifact requires actual receipt'; END IF;
 proof:=convert_from(r.receipt_bytes,'UTF8')::JSONB;
 IF proof IS DISTINCT FROM jsonb_build_object('schema_version',1,'artifact_id',a.artifact_id,'project_id',a.owner_key::UUID,
    'owner_kind',a.owner_kind,'artifact',a.descriptor->'artifact','kind',a.descriptor->'kind',
    'bytes',a.byte_length,'retained_at_ms',a.created_at_ms,'retain_until_ms',a.retain_until_ms)
    OR a.descriptor->'retention_receipt' IS DISTINCT FROM jsonb_build_object(
      'id','aq.inline-retention/'||a.artifact_id::TEXT,'version','1','digest',r.receipt_digest)
 THEN RAISE EXCEPTION 'inline receipt owner mismatch'; END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER admission_inline_commit AFTER INSERT ON admission_artifacts
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION admission_inline_commit_guard();
CREATE CONSTRAINT TRIGGER admission_inline_receipt_commit AFTER INSERT ON admission_inline_receipts
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION admission_inline_commit_guard();

CREATE FUNCTION admission_artifact_link_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE project TEXT; item admission_artifacts%ROWTYPE; held BOOLEAN; count_links BIGINT;
BEGIN
 SELECT holds_task INTO held FROM execution_attempts WHERE attempt_id=NEW.attempt_id AND kind='profiled_v1' FOR UPDATE;
 IF NOT FOUND OR NOT held THEN RAISE EXCEPTION 'artifact link needs held profiled attempt'; END IF;
 SELECT launch_manifest->'bindings'->>'project_id' INTO project FROM profiled_launches WHERE attempt_id=NEW.attempt_id;
 SELECT * INTO item FROM admission_artifacts WHERE artifact_id=NEW.artifact_id FOR SHARE;
 IF NOT FOUND OR item.owner_key IS DISTINCT FROM project THEN RAISE EXCEPTION 'artifact project mismatch'; END IF;
 IF NOT EXISTS(SELECT 1 FROM profiled_launch_artifacts WHERE attempt_id=NEW.attempt_id AND artifact_id=NEW.artifact_id) THEN
  SELECT count(*) INTO count_links FROM profiled_launch_artifacts WHERE attempt_id=NEW.attempt_id;
  IF count_links>=128 THEN RAISE EXCEPTION 'artifact link capacity'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER admission_artifact_link_insert BEFORE INSERT ON profiled_launch_artifacts
 FOR EACH ROW EXECUTE FUNCTION admission_artifact_link_guard();
