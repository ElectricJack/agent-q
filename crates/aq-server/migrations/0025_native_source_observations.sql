-- Bounded actual source-admission observation owner; large Git bytes stay separate.
CREATE OR REPLACE FUNCTION admission_artifact_insert_guard() RETURNS trigger LANGUAGE plpgsql AS $$
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
    OR NEW.descriptor->>'kind' NOT IN ('context','composition','selection','definition','outcome','source')
    OR NEW.owner_key !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
 ) THEN RAISE EXCEPTION 'inline artifact byte/descriptor mismatch'; END IF;
 IF NEW.body IS NOT NULL AND NEW.descriptor->>'kind'='source' AND (
    NEW.owner_kind<>'source_retention'
    OR NEW.descriptor#>>'{artifact,id}' IS DISTINCT FROM 'aq.source-admission-observation/'||NEW.content_digest
    OR NEW.descriptor#>>'{artifact,version}' IS DISTINCT FROM '1'
    OR convert_from(NEW.body,'UTF8')::JSONB->>'schema_version' IS DISTINCT FROM '1'
    OR convert_from(NEW.body,'UTF8')::JSONB#>>'{admission,schema_version}' IS DISTINCT FROM '1'
    OR convert_from(NEW.body,'UTF8')::JSONB#>>'{admission,project_id}' IS DISTINCT FROM NEW.owner_key
 ) THEN RAISE EXCEPTION 'typed source admission owner mismatch'; END IF;
 RETURN NEW;
END $$;

UPDATE meta SET value='25' WHERE key='state_format_version';
