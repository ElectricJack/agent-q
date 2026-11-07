-- Spec 0006 phase 2: an agent's task.create runs the single-task Graph action
-- under the attempt's own journal identity (principal_id = attempt_id). 0061
-- bound every non-user principal to a supervisor session generation, so the
-- journal insert failed its foreign key and every agent create was refused
-- with 503 before the Graph write. Agents get their own, disjoint principal
-- domain, limited to the two create kinds, as supervisors are.
ALTER TABLE dashboard_operations ADD COLUMN agent_attempt_id UUID REFERENCES agent_launch_bindings(attempt_id);
ALTER TABLE dashboard_operations DROP CONSTRAINT dashboard_operation_principal;
ALTER TABLE dashboard_operations ADD CONSTRAINT dashboard_operation_principal CHECK (
 (owner_id IS NOT NULL AND owner_id=principal_id AND supervisor_session_id IS NULL AND agent_attempt_id IS NULL)
 OR (owner_id IS NULL AND supervisor_session_id IS NOT NULL AND supervisor_session_id=principal_id
     AND agent_attempt_id IS NULL AND kind IN ('task.create','task.graph.create'))
 OR (owner_id IS NULL AND supervisor_session_id IS NULL AND agent_attempt_id IS NOT NULL
     AND agent_attempt_id=principal_id AND kind IN ('task.create','task.graph.create'))
);
CREATE OR REPLACE FUNCTION bind_dashboard_principal() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='UPDATE' THEN
  IF ROW(NEW.owner_id,NEW.supervisor_session_id,NEW.agent_attempt_id)
     IS DISTINCT FROM ROW(OLD.owner_id,OLD.supervisor_session_id,OLD.agent_attempt_id) THEN
   RAISE EXCEPTION 'operation principal is immutable';
  END IF;
 ELSIF EXISTS(SELECT 1 FROM users WHERE id=NEW.principal_id) THEN
  NEW.owner_id=NEW.principal_id;
  NEW.supervisor_session_id=NULL;
  NEW.agent_attempt_id=NULL;
 ELSIF EXISTS(SELECT 1 FROM agent_launch_bindings WHERE attempt_id=NEW.principal_id) THEN
  NEW.owner_id=NULL;
  NEW.supervisor_session_id=NULL;
  NEW.agent_attempt_id=NEW.principal_id;
 ELSE
  NEW.owner_id=NULL;
  NEW.supervisor_session_id=NEW.principal_id;
  NEW.agent_attempt_id=NULL;
 END IF;
 RETURN NEW;
END $$;
UPDATE meta SET value='99' WHERE key='state_format_version';
