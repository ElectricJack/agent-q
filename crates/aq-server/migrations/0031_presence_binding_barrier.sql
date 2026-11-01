-- Additive semantic barrier; state29/30 source bytes remain immutable.
CREATE FUNCTION profiled_provisionable_binding_set(snapshot JSONB) RETURNS JSONB
LANGUAGE SQL IMMUTABLE STRICT AS $$
 SELECT COALESCE(jsonb_agg(binding ORDER BY binding::TEXT),'[]'::JSONB)
 FROM (SELECT (b-'observed_at_ms'-'expires_at_ms') ||
  CASE WHEN b->'preparation' IS NOT NULL AND b->'preparation'<>'null'::JSONB
   THEN jsonb_build_object('preparation',(b->'preparation')-'observed_at_ms'-'expires_at_ms')
   ELSE '{}'::JSONB END AS binding
 FROM jsonb_array_elements(snapshot->'credential_bindings') b) normalized
$$;
ALTER TABLE profiled_inventory_semantic_heads ADD COLUMN provisionable_binding_set JSONB
 CHECK(provisionable_binding_set IS NULL OR (jsonb_typeof(provisionable_binding_set)='array' AND octet_length(provisionable_binding_set::TEXT)<=65536));
-- Existing matching current snapshots supply facts, not a synthesized report.
-- The migration owns this one-time backfill before installing the new guard.
ALTER TABLE profiled_inventory_semantic_heads DISABLE TRIGGER profiled_inventory_semantic_guard;
UPDATE profiled_inventory_semantic_heads h SET provisionable_binding_set=profiled_provisionable_binding_set(s.snapshot)
 FROM intelligence_node_snapshots s WHERE h.node_id=s.node_id AND h.boot_id=s.boot_id
 AND h.inventory_revision=s.revision AND h.mode='scoped_v1';
ALTER TABLE profiled_inventory_semantic_heads ENABLE TRIGGER profiled_inventory_semantic_guard;
CREATE OR REPLACE FUNCTION profiled_inventory_semantic_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE incoming profiled_presence_operations%ROWTYPE; previous profiled_presence_operations%ROWTYPE; expected BIGINT; oldbody JSONB; newbody JSONB;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'inventory semantic fence retained'; END IF;
 IF NEW.mode='scoped_v1' THEN
  SELECT * INTO incoming FROM profiled_presence_operations WHERE node_id=NEW.node_id AND operation_id=NEW.operation_id;
  IF NOT FOUND OR incoming.kind<>'inventory_v2' OR incoming.boot_id<>NEW.boot_id OR incoming.inventory_revision<>NEW.inventory_revision OR incoming.snapshot_digest<>NEW.snapshot_digest
  THEN RAISE EXCEPTION 'inventory semantic receipt scope'; END IF;
  newbody:=convert_from(incoming.request_bytes,'UTF8')::JSONB;
  SELECT profiled_provisionable_binding_set(snapshot) INTO NEW.provisionable_binding_set
   FROM intelligence_node_snapshots WHERE node_id=NEW.node_id AND boot_id=NEW.boot_id AND revision=NEW.inventory_revision;
  IF NOT FOUND THEN RAISE EXCEPTION 'current normalized inventory missing'; END IF;
 ELSE NEW.provisionable_binding_set:=NULL;
 END IF;
 IF TG_OP='INSERT' THEN expected:=1; NEW.revision:=1;
 ELSE
  IF NEW.node_id<>OLD.node_id OR NEW.inventory_revision<OLD.inventory_revision THEN RAISE EXCEPTION 'inventory semantic revision regression'; END IF;
  IF NEW.inventory_revision=OLD.inventory_revision THEN
   IF (NEW.boot_id,NEW.mode,NEW.snapshot_digest,NEW.operation_id) IS DISTINCT FROM (OLD.boot_id,OLD.mode,OLD.snapshot_digest,OLD.operation_id) THEN RAISE EXCEPTION 'inventory semantic exact replay changed'; END IF;
   NEW.compatibility_epoch:=OLD.compatibility_epoch; NEW.revision:=OLD.revision;
   IF NEW.mode='scoped_v1' AND incoming.compatibility_epoch<>OLD.compatibility_epoch THEN RAISE EXCEPTION 'inventory replay epoch changed'; END IF;
   RETURN NEW;
  END IF;
  expected:=OLD.compatibility_epoch;
  IF NEW.boot_id<>OLD.boot_id OR NEW.mode<>OLD.mode THEN expected:=expected+1;
  ELSIF NEW.mode='scoped_v1' THEN
   SELECT * INTO previous FROM profiled_presence_operations WHERE node_id=OLD.node_id AND operation_id=OLD.operation_id;
   oldbody:=convert_from(previous.request_bytes,'UTF8')::JSONB;
   IF OLD.provisionable_binding_set IS DISTINCT FROM NEW.provisionable_binding_set OR (oldbody->'runtime',oldbody->'runtime_qualification',oldbody->'negotiation') IS DISTINCT FROM (newbody->'runtime',newbody->'runtime_qualification',newbody->'negotiation') THEN expected:=expected+1; END IF;
  END IF;
  NEW.revision:=OLD.revision+1;
 END IF;
 NEW.compatibility_epoch:=expected;
 IF NEW.mode='scoped_v1' AND incoming.compatibility_epoch<>expected THEN RAISE EXCEPTION 'inventory semantic epoch differs from immutable receipt'; END IF;
 RETURN NEW;
END $$;

UPDATE meta SET value='31' WHERE key='state_format_version';
