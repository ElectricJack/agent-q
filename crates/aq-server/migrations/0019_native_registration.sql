-- Native registration operation history is distinct from the legacy fixture
-- receipt namespace. A historical acknowledgment never reinstalls an identity.
CREATE TABLE native_registration_receipts (
 node_id UUID NOT NULL REFERENCES nodes(id),
 operation_id UUID NOT NULL,
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 response JSONB NOT NULL CHECK(octet_length(response::text)<=65536),
 PRIMARY KEY(node_id,operation_id)
);
CREATE FUNCTION native_registration_receipt_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 RAISE EXCEPTION 'native registration receipt is immutable';
END $$;
CREATE TRIGGER native_registration_receipt_immutable BEFORE UPDATE OR DELETE
 ON native_registration_receipts FOR EACH ROW EXECUTE FUNCTION native_registration_receipt_immutable();
-- The common incarnation row must not provide a back door around the native
-- writer's claim fence, including accidental writes by fixture-only callers.
CREATE FUNCTION native_incarnation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='UPDATE' AND OLD.state->'native' IS NOT NULL AND OLD.state->'native'<>'null'::jsonb
    AND (NEW.state->'native' IS NULL OR NEW.state->'native'='null'::jsonb) THEN
  RAISE EXCEPTION 'native incarnation cannot become a fixture registration';
 END IF;
 IF NEW.state->'native' IS NOT NULL AND NEW.state->'native'<>'null'::jsonb THEN
  IF octet_length(NEW.state::text)>65536 OR NEW.state->'identity' IS DISTINCT FROM NEW.state#>'{native,request,identity}' THEN
   RAISE EXCEPTION 'invalid native incarnation metadata';
  END IF;
  IF (TG_OP='INSERT' OR NEW.state IS DISTINCT FROM OLD.state)
     AND EXISTS(SELECT 1 FROM execution_attempts WHERE node_id=NEW.node_id AND holds_task) THEN
   RAISE EXCEPTION 'execution claims fence native incarnation changes';
  END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER native_incarnation_guard BEFORE INSERT OR UPDATE ON execution_nodes
 FOR EACH ROW EXECUTE FUNCTION native_incarnation_guard();

UPDATE meta SET value='19' WHERE key='state_format_version';
