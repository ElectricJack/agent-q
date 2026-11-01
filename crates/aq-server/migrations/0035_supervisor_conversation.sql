-- Global logical conversation only. AQ-11 mailbox tables retain all mail truth.
CREATE TABLE global_supervisor_conversation (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 conversation_id UUID NOT NULL UNIQUE CHECK(conversation_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 revision BIGINT NOT NULL CHECK(revision=1),
 retained_operations BIGINT NOT NULL DEFAULT 0 CHECK(retained_operations BETWEEN 0 AND 4096),
 retained_bytes BIGINT NOT NULL DEFAULT 0 CHECK(retained_bytes BETWEEN 0 AND 67108864)
);
INSERT INTO global_supervisor_conversation(singleton,conversation_id,revision) VALUES(TRUE,gen_random_uuid(),1);
CREATE FUNCTION guard_global_supervisor_conversation() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'global supervisor conversation history cannot be deleted'; END IF;
 IF ROW(NEW.singleton,NEW.conversation_id,NEW.revision) IS DISTINCT FROM ROW(OLD.singleton,OLD.conversation_id,OLD.revision) THEN
  RAISE EXCEPTION 'global supervisor conversation identity is immutable';
 END IF;
 IF NEW.retained_operations<OLD.retained_operations OR NEW.retained_bytes<OLD.retained_bytes THEN
  RAISE EXCEPTION 'global supervisor conversation retention cannot decrease';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER global_supervisor_conversation_identity_guard BEFORE UPDATE ON global_supervisor_conversation FOR EACH ROW EXECUTE FUNCTION guard_global_supervisor_conversation();
CREATE TRIGGER global_supervisor_conversation_delete_guard BEFORE DELETE ON global_supervisor_conversation FOR EACH ROW EXECUTE FUNCTION guard_global_supervisor_conversation();
CREATE TRIGGER global_supervisor_conversation_truncate_guard BEFORE TRUNCATE ON global_supervisor_conversation FOR EACH STATEMENT EXECUTE FUNCTION guard_global_supervisor_conversation();
UPDATE meta SET value='35' WHERE key='state_format_version';
