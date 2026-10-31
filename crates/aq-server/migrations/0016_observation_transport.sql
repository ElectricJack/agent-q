-- Additive draft, installed explicitly in tests until migration015 is integrated.
-- A prepared observation store cannot admit source facts. Active remains sticky
-- at sequence zero, so loss of undelivered local records never implies fresh state.
CREATE TABLE observation_transport_owners (
 node_id UUID PRIMARY KEY REFERENCES observation_nodes(node_id),
 owner_id UUID NOT NULL UNIQUE,
 state JSONB NOT NULL CHECK(octet_length(state::text)<=4096),
 state_digest TEXT NOT NULL CHECK(length(state_digest)=64)
);
CREATE TABLE observation_transport_receipts (
 node_id UUID NOT NULL REFERENCES observation_transport_owners(node_id),
 operation_id UUID NOT NULL,
 claimed_boot_id UUID,
 owner_revision BIGINT NOT NULL CHECK(owner_revision BETWEEN 1 AND 66),
 request JSONB NOT NULL CHECK(octet_length(request::text)<=4096),
 receipt JSONB NOT NULL CHECK(octet_length(receipt::text)<=8192),
 PRIMARY KEY(node_id,operation_id),
 UNIQUE(node_id,claimed_boot_id),
 UNIQUE(node_id,owner_revision)
);
CREATE FUNCTION observation_transport_owner_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE revision BIGINT; transitions BIGINT;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'observation owner retirement requires explicit recovery'; END IF;
 revision:=(NEW.state->>'revision')::bigint;
 transitions:=(NEW.state->>'boot_transitions')::bigint;
 IF NEW.state->'marker'->>'owner_id' IS DISTINCT FROM NEW.owner_id::text
 OR NEW.state->'marker'->>'node_id' IS DISTINCT FROM NEW.node_id::text
 OR revision IS NULL OR transitions IS NULL OR transitions NOT BETWEEN 0 AND 64
 THEN RAISE EXCEPTION 'observation owner identity'; END IF;
 IF TG_OP='INSERT' THEN
   IF NEW.state->>'phase' IS DISTINCT FROM 'prepared' OR revision<>1 OR transitions<>0
   OR NEW.state->>'current_boot_id' IS DISTINCT FROM NEW.state->'marker'->>'initial_boot_id'
   THEN RAISE EXCEPTION 'observation owner must start prepared'; END IF;
 ELSE
   IF NEW.node_id<>OLD.node_id OR NEW.owner_id<>OLD.owner_id
   OR NEW.state->'marker' IS DISTINCT FROM OLD.state->'marker'
   OR revision<>(OLD.state->>'revision')::bigint+1
   OR NEW.state->>'phase' IS DISTINCT FROM 'active'
   THEN RAISE EXCEPTION 'observation owner is monotonic'; END IF;
   IF OLD.state->>'phase'='prepared' THEN
     IF revision<>2 OR transitions<>0 OR NEW.state->>'current_boot_id' IS DISTINCT FROM OLD.state->>'current_boot_id'
     THEN RAISE EXCEPTION 'invalid store activation'; END IF;
   ELSE
     IF transitions<>(OLD.state->>'boot_transitions')::bigint+1
     OR NEW.state->>'current_boot_id' IS NOT DISTINCT FROM OLD.state->>'current_boot_id'
     THEN RAISE EXCEPTION 'invalid observation boot transition'; END IF;
   END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER observation_transport_owner_monotonic BEFORE INSERT OR UPDATE OR DELETE ON observation_transport_owners
 FOR EACH ROW EXECUTE FUNCTION observation_transport_owner_guard();
CREATE TRIGGER observation_transport_receipt_immutable BEFORE UPDATE OR DELETE ON observation_transport_receipts
 FOR EACH ROW EXECUTE FUNCTION refuse_observation_history_mutation();
CREATE FUNCTION observation_transport_receipt_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE current_owner JSONB; claimed UUID;
BEGIN
 SELECT state INTO current_owner FROM observation_transport_owners WHERE node_id=NEW.node_id;
 IF NEW.request->>'version' IS DISTINCT FROM '1' OR NEW.receipt->>'version' IS DISTINCT FROM '1'
 OR NEW.request->>'node_id' IS DISTINCT FROM NEW.node_id::text
 OR NEW.request->>'operation_id' IS DISTINCT FROM NEW.operation_id::text
 OR NEW.receipt->>'operation_id' IS DISTINCT FROM NEW.operation_id::text
 OR NEW.receipt->'resulting_owner' IS DISTINCT FROM current_owner
 OR NEW.owner_revision IS DISTINCT FROM (current_owner->>'revision')::bigint
 THEN RAISE EXCEPTION 'observation receipt must match current transition'; END IF;
 CASE NEW.request->'operation'->>'kind'
 WHEN 'prepare' THEN claimed:=(NEW.request->'operation'->>'boot_id')::uuid;
 WHEN 'advance_boot' THEN claimed:=(NEW.request->'operation'->>'new_boot_id')::uuid;
 WHEN 'activate' THEN claimed:=NULL;
 ELSE RAISE EXCEPTION 'unknown observation transition';
 END CASE;
 IF NEW.claimed_boot_id IS DISTINCT FROM claimed THEN RAISE EXCEPTION 'boot receipt claim mismatch'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER observation_transport_receipt_matches_owner BEFORE INSERT ON observation_transport_receipts
 FOR EACH ROW EXECUTE FUNCTION observation_transport_receipt_guard();
UPDATE meta SET value='16' WHERE key='state_format_version';
