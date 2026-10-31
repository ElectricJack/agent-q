-- Compiler-owned literal startup constituent/final-payload retention.
ALTER TABLE admission_artifacts DROP CONSTRAINT admission_artifacts_owner_kind_check;
ALTER TABLE admission_artifacts ADD CONSTRAINT admission_artifacts_owner_kind_check
 CHECK(owner_kind IN ('catalog','graph_snapshot','composition','selection','outcome','source_retention','startup'));
-- Public definition identity/version names are immutable across configuration
-- revisions and project retention operations. Existing rows remain immutable.
CREATE INDEX admission_definition_identity ON admission_artifacts
 ((descriptor#>>'{artifact,id}'),(descriptor#>>'{artifact,version}'))
 WHERE descriptor->>'kind'='definition' AND body IS NOT NULL;
CREATE FUNCTION admission_definition_identity_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.body IS NOT NULL AND NEW.descriptor->>'kind'='definition' THEN
  PERFORM pg_advisory_xact_lock(hashtextextended('aq-inline-artifact-quota-v1',0));
  IF EXISTS(SELECT 1 FROM admission_artifacts a
    WHERE a.body IS NOT NULL AND a.descriptor->>'kind'='definition'
      AND a.descriptor#>>'{artifact,id}'=NEW.descriptor#>>'{artifact,id}'
      AND a.descriptor#>>'{artifact,version}'=NEW.descriptor#>>'{artifact,version}'
      AND (a.content_digest IS DISTINCT FROM NEW.content_digest OR a.body IS DISTINCT FROM NEW.body))
  THEN RAISE EXCEPTION 'immutable definition identity/version conflict' USING ERRCODE='23514'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER admission_definition_identity BEFORE INSERT ON admission_artifacts
 FOR EACH ROW EXECUTE FUNCTION admission_definition_identity_guard();
UPDATE meta SET value='27' WHERE key='state_format_version';
