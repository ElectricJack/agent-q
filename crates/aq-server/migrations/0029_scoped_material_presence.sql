-- Negotiated node inventory and one-grant presence have independent sequences.
-- Facts never issue execution authority or globally installed credentials.
CREATE TABLE profiled_presence_storage (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 operations BIGINT NOT NULL DEFAULT 0 CHECK(operations>=0 AND operations<=65536),
 logical_bytes BIGINT NOT NULL DEFAULT 0 CHECK(logical_bytes>=0 AND logical_bytes<=268435456),
 presence_heads BIGINT NOT NULL DEFAULT 0 CHECK(presence_heads>=0 AND presence_heads<=4096)
);
INSERT INTO profiled_presence_storage(singleton) VALUES(TRUE);
CREATE TABLE profiled_presence_node_storage (
 node_id UUID PRIMARY KEY REFERENCES nodes(id),
 operations BIGINT NOT NULL DEFAULT 0 CHECK(operations>=0 AND operations<=4096),
 logical_bytes BIGINT NOT NULL DEFAULT 0 CHECK(logical_bytes>=0 AND logical_bytes<=16777216),
 presence_heads BIGINT NOT NULL DEFAULT 0 CHECK(presence_heads>=0 AND presence_heads<=128)
);
CREATE TABLE profiled_presence_operations (
 node_id UUID NOT NULL REFERENCES nodes(id),
 operation_id UUID NOT NULL CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 kind TEXT NOT NULL CHECK(kind IN('inventory_v2','presence')),
 boot_id UUID NOT NULL CHECK(boot_id<>'00000000-0000-0000-0000-000000000000'),
 attempt_id UUID REFERENCES execution_attempts(attempt_id),
 grant_id UUID,
 lease_id UUID REFERENCES admission_material_leases(lease_id),
 sequence BIGINT NOT NULL CHECK(sequence>0),
 inventory_revision BIGINT NOT NULL CHECK(inventory_revision>0),
 compatibility_epoch BIGINT NOT NULL CHECK(compatibility_epoch>0),
 snapshot_digest TEXT NOT NULL CHECK(snapshot_digest ~ '^[0-9a-f]{64}$'),
 request_bytes BYTEA NOT NULL CHECK(octet_length(request_bytes)>0 AND octet_length(request_bytes)<=65536),
 request_digest TEXT NOT NULL CHECK(request_digest=encode(sha256(request_bytes),'hex')),
 receipt_bytes BYTEA NOT NULL CHECK(octet_length(receipt_bytes)>0 AND octet_length(receipt_bytes)<=4096),
 receipt_digest TEXT NOT NULL CHECK(receipt_digest=encode(sha256(receipt_bytes),'hex')),
 recorded_at_ms BIGINT NOT NULL CHECK(recorded_at_ms>0),
 expires_at_ms BIGINT NOT NULL CHECK(expires_at_ms>recorded_at_ms AND expires_at_ms-recorded_at_ms<=30000),
 PRIMARY KEY(node_id,operation_id),
 CHECK((kind='inventory_v2' AND attempt_id IS NULL AND grant_id IS NULL AND lease_id IS NULL)
    OR(kind='presence' AND attempt_id IS NOT NULL AND grant_id IS NOT NULL AND lease_id IS NOT NULL
       AND grant_id<>'00000000-0000-0000-0000-000000000000'))
);
CREATE UNIQUE INDEX profiled_inventory_v2_sequence ON profiled_presence_operations(node_id,boot_id,sequence) WHERE kind='inventory_v2';
CREATE UNIQUE INDEX profiled_presence_sequence ON profiled_presence_operations(node_id,boot_id,attempt_id,grant_id,lease_id,sequence) WHERE kind='presence';
CREATE INDEX profiled_presence_attempt ON profiled_presence_operations(attempt_id,lease_id) WHERE kind='presence';
CREATE TABLE profiled_inventory_semantic_heads (
 node_id UUID PRIMARY KEY REFERENCES nodes(id),
 boot_id UUID NOT NULL,
 mode TEXT NOT NULL CHECK(mode IN('legacy','scoped_v1')),
 inventory_revision BIGINT NOT NULL CHECK(inventory_revision>0),
 snapshot_digest TEXT NOT NULL CHECK(snapshot_digest ~ '^[0-9a-f]{64}$'),
 operation_id UUID,
 compatibility_epoch BIGINT NOT NULL CHECK(compatibility_epoch>0),
 revision BIGINT NOT NULL CHECK(revision>0),
 FOREIGN KEY(node_id,operation_id) REFERENCES profiled_presence_operations(node_id,operation_id),
 CHECK((mode='legacy' AND operation_id IS NULL) OR(mode='scoped_v1' AND operation_id IS NOT NULL))
);
CREATE TABLE profiled_material_presence_heads (
 node_id UUID NOT NULL REFERENCES nodes(id),
 attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
 lease_id UUID NOT NULL REFERENCES admission_material_leases(lease_id),
 boot_id UUID NOT NULL,
 grant_id UUID NOT NULL,
 operation_id UUID NOT NULL,
 sequence BIGINT NOT NULL CHECK(sequence>0),
 compatibility_epoch BIGINT NOT NULL CHECK(compatibility_epoch>0),
 revision BIGINT NOT NULL CHECK(revision>0),
 PRIMARY KEY(node_id,attempt_id,lease_id),
 FOREIGN KEY(node_id,operation_id) REFERENCES profiled_presence_operations(node_id,operation_id)
);
CREATE FUNCTION refuse_profiled_presence_history_change() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'immutable scoped presence operation'; END $$;
CREATE TRIGGER profiled_presence_operation_immutable BEFORE UPDATE OR DELETE ON profiled_presence_operations FOR EACH ROW EXECUTE FUNCTION refuse_profiled_presence_history_change();
CREATE FUNCTION profiled_presence_operation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE g profiled_presence_storage%ROWTYPE; n profiled_presence_node_storage%ROWTYPE; incoming BIGINT; q JSONB; a JSONB;
BEGIN
 q:=convert_from(NEW.request_bytes,'UTF8')::JSONB; a:=convert_from(NEW.receipt_bytes,'UTF8')::JSONB;
 IF (q->>'operation_id')::UUID IS DISTINCT FROM NEW.operation_id OR (q#>>'{identity,node_id}')::UUID IS DISTINCT FROM NEW.node_id
 OR (q#>>'{identity,boot_id}')::UUID IS DISTINCT FROM NEW.boot_id OR (q->>'sequence')::BIGINT IS DISTINCT FROM NEW.sequence
 OR (CASE WHEN NEW.kind='inventory_v2' THEN a#>>'{inventory,request_digest}' ELSE a->>'request_digest' END) IS DISTINCT FROM NEW.request_digest
 OR (a#>>'{identity,node_id}')::UUID IS DISTINCT FROM NEW.node_id OR (a#>>'{identity,boot_id}')::UUID IS DISTINCT FROM NEW.boot_id
 OR (a->>'recorded_at_ms')::BIGINT IS DISTINCT FROM NEW.recorded_at_ms
 OR (a->>'expires_at_ms')::BIGINT IS DISTINCT FROM NEW.expires_at_ms OR q->>'schema_version' IS DISTINCT FROM '1' OR a->>'schema_version' IS DISTINCT FROM '1'
 THEN RAISE EXCEPTION 'presence operation body scope'; END IF;
 IF NEW.kind='inventory_v2' AND ((a#>>'{inventory,revision}')::BIGINT IS DISTINCT FROM NEW.inventory_revision OR a#>>'{inventory,snapshot_digest}' IS DISTINCT FROM NEW.snapshot_digest)
 THEN RAISE EXCEPTION 'inventory body receipt scope'; END IF;
 IF NEW.kind='presence' AND ((q#>>'{binding,attempt_id}')::UUID IS DISTINCT FROM NEW.attempt_id OR (q->>'lease_id')::UUID IS DISTINCT FROM NEW.lease_id OR (q->>'grant_id')::UUID IS DISTINCT FROM NEW.grant_id
 OR a->'binding' IS DISTINCT FROM q->'binding' OR (a->>'operation_id')::UUID IS DISTINCT FROM NEW.operation_id OR (a->>'sequence')::BIGINT IS DISTINCT FROM NEW.sequence)
 THEN RAISE EXCEPTION 'presence attempt lease scope'; END IF;
 incoming:=octet_length(NEW.request_bytes)::BIGINT+octet_length(NEW.receipt_bytes)::BIGINT+1024;
 SELECT * INTO g FROM profiled_presence_storage WHERE singleton FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'presence storage owner missing'; END IF;
 INSERT INTO profiled_presence_node_storage(node_id) VALUES(NEW.node_id) ON CONFLICT DO NOTHING;
 SELECT * INTO n FROM profiled_presence_node_storage WHERE node_id=NEW.node_id FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'presence node storage missing'; END IF;
 IF g.operations>=65536 OR n.operations>=4096 OR g.logical_bytes+incoming>268435456 OR n.logical_bytes+incoming>16777216
 THEN RAISE EXCEPTION 'scoped presence retention capacity' USING ERRCODE='54000'; END IF;
 UPDATE profiled_presence_storage SET operations=operations+1,logical_bytes=logical_bytes+incoming WHERE singleton;
 UPDATE profiled_presence_node_storage SET operations=operations+1,logical_bytes=logical_bytes+incoming WHERE node_id=NEW.node_id;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_presence_operation_guard BEFORE INSERT ON profiled_presence_operations FOR EACH ROW EXECUTE FUNCTION profiled_presence_operation_guard();
CREATE FUNCTION profiled_inventory_semantic_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE incoming profiled_presence_operations%ROWTYPE; previous profiled_presence_operations%ROWTYPE; expected BIGINT; oldbody JSONB; newbody JSONB;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'inventory semantic fence retained'; END IF;
 IF NEW.mode='scoped_v1' THEN
  SELECT * INTO incoming FROM profiled_presence_operations WHERE node_id=NEW.node_id AND operation_id=NEW.operation_id;
  IF NOT FOUND OR incoming.kind<>'inventory_v2' OR incoming.boot_id<>NEW.boot_id OR incoming.inventory_revision<>NEW.inventory_revision OR incoming.snapshot_digest<>NEW.snapshot_digest
  THEN RAISE EXCEPTION 'inventory semantic receipt scope'; END IF;
  newbody:=convert_from(incoming.request_bytes,'UTF8')::JSONB;
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
   IF (oldbody->'runtime',oldbody->'runtime_qualification',oldbody->'negotiation') IS DISTINCT FROM (newbody->'runtime',newbody->'runtime_qualification',newbody->'negotiation') THEN expected:=expected+1; END IF;
  END IF;
  NEW.revision:=OLD.revision+1;
 END IF;
 NEW.compatibility_epoch:=expected;
 IF NEW.mode='scoped_v1' AND incoming.compatibility_epoch<>expected THEN RAISE EXCEPTION 'inventory semantic epoch differs from immutable receipt'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_inventory_semantic_guard BEFORE INSERT OR UPDATE OR DELETE ON profiled_inventory_semantic_heads FOR EACH ROW EXECUTE FUNCTION profiled_inventory_semantic_guard();
CREATE FUNCTION profiled_material_presence_head_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE incoming profiled_presence_operations%ROWTYPE; g profiled_presence_storage%ROWTYPE; n profiled_presence_node_storage%ROWTYPE;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'presence sequence/negative fence retained'; END IF;
 SELECT * INTO incoming FROM profiled_presence_operations WHERE node_id=NEW.node_id AND operation_id=NEW.operation_id;
 IF NOT FOUND OR incoming.kind<>'presence' OR (incoming.boot_id,incoming.attempt_id,incoming.lease_id,incoming.grant_id,incoming.sequence,incoming.compatibility_epoch) IS DISTINCT FROM (NEW.boot_id,NEW.attempt_id,NEW.lease_id,NEW.grant_id,NEW.sequence,NEW.compatibility_epoch)
 THEN RAISE EXCEPTION 'presence head receipt scope'; END IF;
 IF TG_OP='UPDATE' THEN
  IF (NEW.node_id,NEW.attempt_id,NEW.lease_id,NEW.boot_id,NEW.grant_id) IS DISTINCT FROM (OLD.node_id,OLD.attempt_id,OLD.lease_id,OLD.boot_id,OLD.grant_id) OR NEW.sequence<OLD.sequence OR NEW.compatibility_epoch<OLD.compatibility_epoch THEN RAISE EXCEPTION 'presence head scope or sequence regression'; END IF;
  IF NEW.sequence=OLD.sequence THEN
   IF NEW.operation_id<>OLD.operation_id THEN RAISE EXCEPTION 'presence head sequence conflict'; END IF;
   NEW.revision:=OLD.revision;
  ELSE NEW.revision:=OLD.revision+1; END IF;
 ELSE
  SELECT * INTO g FROM profiled_presence_storage WHERE singleton FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'presence storage owner missing'; END IF;
  SELECT * INTO n FROM profiled_presence_node_storage WHERE node_id=NEW.node_id FOR UPDATE;
 IF NOT FOUND THEN RAISE EXCEPTION 'presence node storage missing'; END IF;
  IF NOT FOUND OR g.presence_heads>=4096 OR n.presence_heads>=128 THEN RAISE EXCEPTION 'presence head capacity' USING ERRCODE='54000'; END IF;
  UPDATE profiled_presence_storage SET presence_heads=presence_heads+1 WHERE singleton;
  UPDATE profiled_presence_node_storage SET presence_heads=presence_heads+1 WHERE node_id=NEW.node_id;
  NEW.revision:=1;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER profiled_material_presence_head_guard BEFORE INSERT OR UPDATE OR DELETE ON profiled_material_presence_heads FOR EACH ROW EXECUTE FUNCTION profiled_material_presence_head_guard();
UPDATE meta SET value='29' WHERE key='state_format_version';
