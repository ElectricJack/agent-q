-- Internal verified-wait storage only. No admission path is bypassed or mounted.
-- Frozen migrations1..37 are unchanged. One full envelope is reserved per history.
CREATE TABLE wait_owner_budgets (
 scope_key TEXT PRIMARY KEY CHECK(octet_length(scope_key) BETWEEN 1 AND 128),
 envelope_limit BIGINT NOT NULL CHECK(envelope_limit BETWEEN 1 AND 2048),
 envelopes BIGINT NOT NULL DEFAULT 0 CHECK(envelopes>=0),
 reserved_bytes BIGINT NOT NULL DEFAULT 0,
 CONSTRAINT wait_owner_budget_capacity CHECK(envelopes<=envelope_limit AND reserved_bytes=envelopes*1048576),
 CONSTRAINT wait_owner_budget_metadata CHECK(octet_length(to_jsonb(ROW(scope_key,envelope_limit,envelopes,reserved_bytes))::text)<=4096)
);
INSERT INTO wait_owner_budgets(scope_key,envelope_limit) VALUES('installation',2048);
CREATE TABLE wait_owner_projects (
 workspace_id UUID NOT NULL CHECK(workspace_id<>'00000000-0000-0000-0000-000000000000'),
 project_id UUID NOT NULL CHECK(project_id<>'00000000-0000-0000-0000-000000000000'),
 revision BIGINT NOT NULL CHECK(revision>0),
 budget_scope TEXT NOT NULL REFERENCES wait_owner_budgets(scope_key),
 PRIMARY KEY(workspace_id,project_id),
 CHECK(budget_scope='project/'||workspace_id::text||'/'||project_id::text),
 CHECK(octet_length(to_jsonb(ROW(workspace_id,project_id,revision,budget_scope))::text)<=4096)
);
CREATE TABLE wait_owner_tasks (
 workspace_id UUID NOT NULL,
 project_id UUID NOT NULL,
 task_id UUID NOT NULL CHECK(task_id<>'00000000-0000-0000-0000-000000000000'),
 lineage_id UUID NOT NULL CHECK(lineage_id<>'00000000-0000-0000-0000-000000000000'),
 generation BIGINT NOT NULL CHECK(generation>0),
 active_wait_id UUID UNIQUE,
 PRIMARY KEY(workspace_id,project_id,task_id),
 FOREIGN KEY(workspace_id,project_id) REFERENCES wait_owner_projects(workspace_id,project_id),
 CHECK(octet_length(to_jsonb(ROW(workspace_id,project_id,task_id,lineage_id,generation,active_wait_id))::text)<=8192)
);
CREATE TABLE wait_owner_records (
 wait_id UUID PRIMARY KEY CHECK(wait_id<>'00000000-0000-0000-0000-000000000000'),
 operation_id UUID NOT NULL UNIQUE CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 workspace_id UUID NOT NULL,
 project_id UUID NOT NULL,
 task_id UUID NOT NULL,
 lineage_id UUID NOT NULL,
 node_id UUID NOT NULL REFERENCES nodes(id),
 attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
 binding_digest TEXT NOT NULL CHECK(binding_digest ~ '^[0-9a-f]{64}$'),
 source_access_digest TEXT NOT NULL CHECK(source_access_digest ~ '^[0-9a-f]{64}$'),
 policy_digest TEXT NOT NULL CHECK(policy_digest ~ '^[0-9a-f]{64}$'),
 source_profile TEXT NOT NULL CHECK(source_profile IN ('local_v1','test_fixture_v1')),
 wait_generation BIGINT NOT NULL CHECK(wait_generation>0),
 continuation_id UUID NOT NULL UNIQUE CHECK(continuation_id<>'00000000-0000-0000-0000-000000000000'),
 continuation_generation BIGINT NOT NULL CHECK(continuation_generation>0),
 outcome_id UUID NOT NULL UNIQUE CHECK(outcome_id<>'00000000-0000-0000-0000-000000000000'),
 resume_intent_id UUID NOT NULL UNIQUE CHECK(resume_intent_id<>'00000000-0000-0000-0000-000000000000'),
 member_count INTEGER NOT NULL CHECK(member_count BETWEEN 1 AND 32),
 envelope_bytes BIGINT NOT NULL CHECK(envelope_bytes=1048576),
 created_at_ms BIGINT NOT NULL CHECK(created_at_ms BETWEEN 1 AND 9007199254740991),
 deadline_ms BIGINT NOT NULL CHECK(deadline_ms>created_at_ms AND deadline_ms<=9007199254740991 AND deadline_ms-created_at_ms<=2592000000),
 revision BIGINT NOT NULL CHECK(revision>0),
 logical_now_ms BIGINT NOT NULL CHECK(logical_now_ms BETWEEN created_at_ms AND 9007199254740991),
 next_due_ms BIGINT NOT NULL CHECK(next_due_ms BETWEEN 1 AND 9007199254740991),
 phase TEXT NOT NULL CHECK(phase IN ('installing','pending','recovery_held','terminal')),
 dirty_generation BIGINT NOT NULL CHECK(dirty_generation>=0),
 work_generation BIGINT NOT NULL CHECK(work_generation BETWEEN 1 AND 65536),
 retired BOOLEAN NOT NULL DEFAULT FALSE,
 cancel_operation_id UUID,
 terminal_digest TEXT CHECK(terminal_digest ~ '^[0-9a-f]{64}$'),
 resume_state TEXT NOT NULL CHECK(resume_state IN ('none','pending','retired')),
 FOREIGN KEY(workspace_id,project_id,task_id) REFERENCES wait_owner_tasks(workspace_id,project_id,task_id),
 CONSTRAINT wait_owner_record_terminal CHECK((phase='terminal')=(terminal_digest IS NOT NULL) AND retired=(cancel_operation_id IS NOT NULL) AND (NOT retired OR (resume_state='retired' AND phase='terminal')) AND (cancel_operation_id IS NULL OR cancel_operation_id<>'00000000-0000-0000-0000-000000000000')),
 CONSTRAINT wait_owner_record_metadata CHECK(octet_length(to_jsonb(ROW(wait_id,operation_id,request_digest,workspace_id,project_id,task_id,lineage_id,node_id,attempt_id,binding_digest,source_access_digest,policy_digest,source_profile,wait_generation,continuation_id,continuation_generation,outcome_id,resume_intent_id,member_count,envelope_bytes,created_at_ms,deadline_ms,revision,logical_now_ms,next_due_ms,phase,dirty_generation,work_generation,retired,cancel_operation_id,terminal_digest,resume_state))::text)<=8192)
);
CREATE UNIQUE INDEX wait_owner_one_live_task ON wait_owner_records(workspace_id,project_id,task_id) WHERE NOT retired;
CREATE INDEX wait_owner_scope ON wait_owner_records(workspace_id,project_id,wait_id);
CREATE INDEX wait_owner_attempt ON wait_owner_records(node_id,attempt_id,wait_id);
ALTER TABLE wait_owner_tasks ADD CONSTRAINT wait_owner_active_wait FOREIGN KEY(active_wait_id) REFERENCES wait_owner_records(wait_id) DEFERRABLE INITIALLY DEFERRED;
CREATE TABLE wait_owner_members (
 wait_id UUID NOT NULL REFERENCES wait_owner_records(wait_id),
 ordinal INTEGER NOT NULL CHECK(ordinal BETWEEN 1 AND 32),
 member_id UUID NOT NULL CHECK(member_id<>'00000000-0000-0000-0000-000000000000'),
 owner TEXT NOT NULL CHECK(owner IN ('task_graph','mailbox','human_decision','clock','operation','playbook','wait_registry','scheduler')),
 condition_digest TEXT NOT NULL CHECK(condition_digest ~ '^[0-9a-f]{64}$'),
 subscription_digest TEXT NOT NULL CHECK(subscription_digest ~ '^[0-9a-f]{64}$'),
 blocker_kind TEXT CHECK(blocker_kind IN ('task','wait')),
 blocker_id UUID,
 local_original JSONB,
 local_current JSONB,
 PRIMARY KEY(wait_id,ordinal),
 UNIQUE(wait_id,member_id),
 CHECK((blocker_kind IS NULL)=(blocker_id IS NULL)),
 CHECK(octet_length(to_jsonb(ROW(wait_id,ordinal,member_id,owner,condition_digest,subscription_digest,blocker_kind,blocker_id,local_original,local_current))::text)<=2048)
);
CREATE TABLE wait_owner_edges (
 wait_id UUID NOT NULL,
 ordinal INTEGER NOT NULL,
 waiter_key TEXT NOT NULL CHECK(octet_length(waiter_key) BETWEEN 1 AND 128),
 blocker_key TEXT NOT NULL CHECK(octet_length(blocker_key) BETWEEN 1 AND 128),
 active BOOLEAN NOT NULL,
 activated_revision BIGINT NOT NULL CHECK(activated_revision>0),
 retired_revision BIGINT,
 PRIMARY KEY(wait_id,ordinal),
 FOREIGN KEY(wait_id,ordinal) REFERENCES wait_owner_members(wait_id,ordinal),
 CHECK(waiter_key<>blocker_key),
 CHECK(active=(retired_revision IS NULL) AND (retired_revision IS NULL OR retired_revision>activated_revision)),
 CHECK(octet_length(to_jsonb(ROW(wait_id,ordinal,waiter_key,blocker_key,active,activated_revision,retired_revision))::text)<=1024)
);
CREATE INDEX wait_owner_active_edges ON wait_owner_edges(waiter_key,blocker_key) WHERE active;
CREATE TABLE wait_owner_work (
 wait_id UUID PRIMARY KEY REFERENCES wait_owner_records(wait_id),
 generation BIGINT NOT NULL CHECK(generation BETWEEN 1 AND 65536),
 state TEXT NOT NULL CHECK(state IN ('pending','leased','idle','held','done')),
 next_due_ms BIGINT NOT NULL CHECK(next_due_ms BETWEEN 1 AND 9007199254740991),
 lease_id UUID,
 lease_expires_ms BIGINT,
 transition_at_ms BIGINT NOT NULL CHECK(transition_at_ms BETWEEN 1 AND 9007199254740991),
 CHECK((state='leased')=(lease_id IS NOT NULL) AND (lease_id IS NULL)=(lease_expires_ms IS NULL)),
 CHECK(lease_expires_ms IS NULL OR (lease_expires_ms>transition_at_ms AND lease_expires_ms<=9007199254740991)),
 CHECK(octet_length(to_jsonb(ROW(wait_id,generation,state,next_due_ms,lease_id,lease_expires_ms,transition_at_ms))::text)<=8192)
);
CREATE INDEX wait_owner_due_work ON wait_owner_work(next_due_ms,wait_id) WHERE state IN ('pending','idle','leased');
CREATE TABLE wait_owner_slots (
 wait_id UUID NOT NULL REFERENCES wait_owner_records(wait_id),
 kind TEXT NOT NULL CHECK(kind IN ('original','snapshot','work','registration','cancellation','outcome','resume','completion')),
 revision BIGINT NOT NULL CHECK(revision>=0),
 plaintext_bytes BIGINT NOT NULL CHECK(plaintext_bytes>=0),
 plaintext_digest TEXT,
 sealed_version INTEGER,
 key_id TEXT,
 nonce BYTEA,
 ciphertext BYTEA,
 ciphertext_digest TEXT,
 PRIMARY KEY(wait_id,kind),
 CONSTRAINT wait_owner_slot_bound CHECK(plaintext_bytes<=CASE kind WHEN 'original' THEN 131072 WHEN 'snapshot' THEN 262144 WHEN 'work' THEN 262144 WHEN 'registration' THEN 16384 WHEN 'cancellation' THEN 16384 WHEN 'outcome' THEN 131072 WHEN 'resume' THEN 8192 WHEN 'completion' THEN 16384 END),
 CONSTRAINT wait_owner_slot_sealed CHECK(COALESCE((plaintext_bytes=0 AND revision=0 AND plaintext_digest IS NULL AND sealed_version IS NULL AND key_id IS NULL AND nonce IS NULL AND ciphertext IS NULL AND ciphertext_digest IS NULL) OR (plaintext_bytes>0 AND revision>0 AND plaintext_digest ~ '^[0-9a-f]{64}$' AND sealed_version=1 AND key_id ~ '^[A-Za-z0-9_-]{1,64}$' AND octet_length(nonce)=12 AND octet_length(ciphertext)=plaintext_bytes+16 AND ciphertext_digest=encode(sha256(ciphertext),'hex')),FALSE)),
 CONSTRAINT wait_owner_slot_metadata CHECK(octet_length((to_jsonb(ROW(wait_id,kind,revision,plaintext_bytes,plaintext_digest,sealed_version,key_id,nonce,ciphertext_digest)))::text)+16<=1024)
);
CREATE FUNCTION wait38_budget_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'wait budgets are retained'; END IF;
 IF TG_OP='UPDATE' AND (NEW.scope_key<>OLD.scope_key OR ((NEW.envelopes,NEW.reserved_bytes) IS DISTINCT FROM (OLD.envelopes,OLD.reserved_bytes) AND pg_trigger_depth()<2)) THEN RAISE EXCEPTION 'wait budget accounting is owner controlled'; END IF;
 IF NEW.scope_key<>'installation' AND (NEW.scope_key !~ '^project/[0-9a-f-]{36}/[0-9a-f-]{36}$' OR NEW.envelope_limit>256) THEN RAISE EXCEPTION 'wait project budget bound'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER wait_owner_budget_guard BEFORE INSERT OR UPDATE OR DELETE ON wait_owner_budgets FOR EACH ROW EXECUTE FUNCTION wait38_budget_guard();
CREATE TRIGGER wait_owner_budget_truncate BEFORE TRUNCATE ON wait_owner_budgets FOR EACH STATEMENT EXECUTE FUNCTION wait38_budget_guard();
CREATE FUNCTION wait38_record_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'wait identities and envelopes are retained'; END IF;
 IF TG_OP='UPDATE' THEN
  IF (NEW.wait_id,NEW.operation_id,NEW.request_digest,NEW.workspace_id,NEW.project_id,NEW.task_id,NEW.lineage_id,NEW.node_id,NEW.attempt_id,NEW.binding_digest,NEW.source_access_digest,NEW.policy_digest,NEW.source_profile,NEW.wait_generation,NEW.continuation_id,NEW.continuation_generation,NEW.outcome_id,NEW.resume_intent_id,NEW.member_count,NEW.envelope_bytes,NEW.created_at_ms,NEW.deadline_ms) IS DISTINCT FROM (OLD.wait_id,OLD.operation_id,OLD.request_digest,OLD.workspace_id,OLD.project_id,OLD.task_id,OLD.lineage_id,OLD.node_id,OLD.attempt_id,OLD.binding_digest,OLD.source_access_digest,OLD.policy_digest,OLD.source_profile,OLD.wait_generation,OLD.continuation_id,OLD.continuation_generation,OLD.outcome_id,OLD.resume_intent_id,OLD.member_count,OLD.envelope_bytes,OLD.created_at_ms,OLD.deadline_ms)
   OR NEW.revision<>OLD.revision+1 OR NEW.logical_now_ms<OLD.logical_now_ms OR NEW.dirty_generation<OLD.dirty_generation OR NEW.work_generation<OLD.work_generation OR (OLD.retired AND NOT NEW.retired)
   OR (OLD.cancel_operation_id IS NOT NULL AND NEW.cancel_operation_id IS DISTINCT FROM OLD.cancel_operation_id)
   OR (OLD.terminal_digest IS NOT NULL AND NEW.terminal_digest IS DISTINCT FROM OLD.terminal_digest)
   OR (OLD.phase='terminal' AND NEW.phase<>'terminal') OR (OLD.phase='recovery_held' AND NEW.phase NOT IN ('recovery_held','terminal')) OR (OLD.phase='pending' AND NEW.phase='installing') THEN RAISE EXCEPTION 'wait immutable or monotonic state changed'; END IF;
 ELSE
  IF NEW.revision<>1 OR NEW.logical_now_ms<>NEW.created_at_ms OR NEW.phase<>'installing' OR NEW.retired OR NEW.cancel_operation_id IS NOT NULL OR NEW.terminal_digest IS NOT NULL OR NEW.resume_state<>'none' THEN RAISE EXCEPTION 'new wait must install under original envelope'; END IF;
  UPDATE wait_owner_budgets SET envelopes=envelopes+1,reserved_bytes=reserved_bytes+1048576 WHERE scope_key='installation';
  IF NOT FOUND THEN RAISE EXCEPTION 'missing wait installation budget'; END IF;
  UPDATE wait_owner_budgets SET envelopes=envelopes+1,reserved_bytes=reserved_bytes+1048576 WHERE scope_key='project/'||NEW.workspace_id::text||'/'||NEW.project_id::text;
  IF NOT FOUND THEN RAISE EXCEPTION 'missing wait project budget'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER wait_owner_record_guard BEFORE INSERT OR UPDATE OR DELETE ON wait_owner_records FOR EACH ROW EXECUTE FUNCTION wait38_record_guard();
CREATE TRIGGER wait_owner_record_truncate BEFORE TRUNCATE ON wait_owner_records FOR EACH STATEMENT EXECUTE FUNCTION wait38_record_guard();
CREATE FUNCTION wait38_child_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP IN ('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'wait owner children are retained'; END IF;
 IF TG_OP='UPDATE' THEN
  IF TG_TABLE_NAME='wait_owner_slots' THEN
   IF (NEW.wait_id,NEW.kind) IS DISTINCT FROM (OLD.wait_id,OLD.kind) OR (OLD.plaintext_bytes>0 AND NEW.kind NOT IN ('snapshot','work')) OR NEW.revision<=OLD.revision OR NEW.plaintext_bytes=0 THEN RAISE EXCEPTION 'wait slot identity, original or terminal changed'; END IF;
  ELSIF TG_TABLE_NAME='wait_owner_members' THEN
   IF (NEW.wait_id,NEW.ordinal,NEW.member_id,NEW.owner,NEW.condition_digest,NEW.subscription_digest,NEW.blocker_kind,NEW.blocker_id) IS DISTINCT FROM (OLD.wait_id,OLD.ordinal,OLD.member_id,OLD.owner,OLD.condition_digest,OLD.subscription_digest,OLD.blocker_kind,OLD.blocker_id) OR (OLD.local_original IS NOT NULL AND NEW.local_original IS DISTINCT FROM OLD.local_original) THEN RAISE EXCEPTION 'wait source identity changed'; END IF;
  ELSIF TG_TABLE_NAME='wait_owner_edges' THEN
   IF (NEW.wait_id,NEW.ordinal,NEW.waiter_key,NEW.blocker_key) IS DISTINCT FROM (OLD.wait_id,OLD.ordinal,OLD.waiter_key,OLD.blocker_key) OR (NOT OLD.active AND NEW.active) THEN RAISE EXCEPTION 'wait edge identity or retirement changed'; END IF;
  ELSIF TG_TABLE_NAME='wait_owner_work' THEN
   IF NEW.wait_id<>OLD.wait_id OR NEW.generation<OLD.generation OR NEW.transition_at_ms<OLD.transition_at_ms OR (OLD.state='done' AND NEW.state<>'done') OR (NEW.lease_id IS NOT NULL AND NEW.lease_id IS DISTINCT FROM OLD.lease_id AND OLD.lease_expires_ms IS NOT NULL AND NEW.transition_at_ms<OLD.lease_expires_ms) THEN RAISE EXCEPTION 'wait work identity or live lease changed'; END IF;
  ELSIF TG_TABLE_NAME='wait_owner_tasks' THEN
   IF (NEW.workspace_id,NEW.project_id,NEW.task_id,NEW.lineage_id) IS DISTINCT FROM (OLD.workspace_id,OLD.project_id,OLD.task_id,OLD.lineage_id) OR NEW.generation<>OLD.generation+1 THEN RAISE EXCEPTION 'wait task gate identity changed'; END IF;
  ELSIF TG_TABLE_NAME='wait_owner_projects' THEN
   IF (NEW.workspace_id,NEW.project_id,NEW.budget_scope) IS DISTINCT FROM (OLD.workspace_id,OLD.project_id,OLD.budget_scope) OR NEW.revision<>OLD.revision+1 THEN RAISE EXCEPTION 'wait project gate identity changed'; END IF;
  END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER wait_owner_slot_guard BEFORE UPDATE OR DELETE ON wait_owner_slots FOR EACH ROW EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_member_guard BEFORE UPDATE OR DELETE ON wait_owner_members FOR EACH ROW EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_edge_guard BEFORE UPDATE OR DELETE ON wait_owner_edges FOR EACH ROW EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_work_guard BEFORE UPDATE OR DELETE ON wait_owner_work FOR EACH ROW EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_task_guard BEFORE UPDATE OR DELETE ON wait_owner_tasks FOR EACH ROW EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_project_guard BEFORE UPDATE OR DELETE ON wait_owner_projects FOR EACH ROW EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_slot_truncate BEFORE TRUNCATE ON wait_owner_slots FOR EACH STATEMENT EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_member_truncate BEFORE TRUNCATE ON wait_owner_members FOR EACH STATEMENT EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_edge_truncate BEFORE TRUNCATE ON wait_owner_edges FOR EACH STATEMENT EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_work_truncate BEFORE TRUNCATE ON wait_owner_work FOR EACH STATEMENT EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_task_truncate BEFORE TRUNCATE ON wait_owner_tasks FOR EACH STATEMENT EXECUTE FUNCTION wait38_child_guard();
CREATE TRIGGER wait_owner_project_truncate BEFORE TRUNCATE ON wait_owner_projects FOR EACH STATEMENT EXECUTE FUNCTION wait38_child_guard();
CREATE FUNCTION wait38_edge_revision() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE current_revision BIGINT;
BEGIN
 IF TG_OP='INSERT' THEN
  IF NOT NEW.active THEN RAISE EXCEPTION 'new wait edge must be active'; END IF;
  UPDATE wait_owner_projects p SET revision=p.revision+1 FROM wait_owner_records w WHERE w.wait_id=NEW.wait_id AND p.workspace_id=w.workspace_id AND p.project_id=w.project_id RETURNING p.revision INTO current_revision;
  IF NOT FOUND THEN RAISE EXCEPTION 'missing wait project revision owner'; END IF;
  NEW.activated_revision:=current_revision; NEW.retired_revision:=NULL;
 ELSIF OLD.active AND NOT NEW.active THEN
  IF NEW.activated_revision<>OLD.activated_revision OR NEW.retired_revision IS DISTINCT FROM OLD.retired_revision THEN RAISE EXCEPTION 'wait edge revision is owner assigned'; END IF;
  UPDATE wait_owner_projects p SET revision=p.revision+1 FROM wait_owner_records w WHERE w.wait_id=NEW.wait_id AND p.workspace_id=w.workspace_id AND p.project_id=w.project_id RETURNING p.revision INTO current_revision;
  IF NOT FOUND THEN RAISE EXCEPTION 'missing wait project revision owner'; END IF;
  NEW.retired_revision:=current_revision;
 ELSIF (NEW.activated_revision,NEW.retired_revision) IS DISTINCT FROM (OLD.activated_revision,OLD.retired_revision) THEN RAISE EXCEPTION 'wait edge revision changed';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER wait_owner_edge_revision BEFORE INSERT OR UPDATE ON wait_owner_edges FOR EACH ROW EXECUTE FUNCTION wait38_edge_revision();
CREATE FUNCTION wait38_check_wait(p_wait UUID) RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE w wait_owner_records%ROWTYPE; task_pointer UUID; kinds BIGINT; filled BIGINT;
BEGIN
 SELECT * INTO w FROM wait_owner_records WHERE wait_id=p_wait;
 IF NOT FOUND THEN RAISE EXCEPTION 'orphan wait child or gate'; END IF;
 SELECT active_wait_id INTO task_pointer FROM wait_owner_tasks WHERE workspace_id=w.workspace_id AND project_id=w.project_id AND task_id=w.task_id AND lineage_id=w.lineage_id;
 IF NOT FOUND OR (NOT w.retired AND task_pointer IS DISTINCT FROM w.wait_id) OR (w.retired AND task_pointer=w.wait_id) THEN RAISE EXCEPTION 'wait task gate mismatch'; END IF;
 IF (SELECT count(*) FROM wait_owner_members WHERE wait_id=w.wait_id)<>w.member_count OR (SELECT min(ordinal) FROM wait_owner_members WHERE wait_id=w.wait_id)<>1 OR (SELECT max(ordinal) FROM wait_owner_members WHERE wait_id=w.wait_id)<>w.member_count THEN RAISE EXCEPTION 'wait member index incomplete'; END IF;
 SELECT count(*),count(*) FILTER(WHERE plaintext_bytes>0) INTO kinds,filled FROM wait_owner_slots WHERE wait_id=w.wait_id;
 IF kinds<>8 OR NOT EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='original' AND plaintext_bytes>0 AND revision=1) OR NOT EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='registration' AND plaintext_bytes>0 AND revision=1) OR NOT EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='snapshot' AND plaintext_bytes>0 AND revision=w.revision AND (w.phase='terminal' OR plaintext_bytes<=131072)) OR NOT EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='work' AND plaintext_bytes>0) THEN RAISE EXCEPTION 'wait reserved slots or snapshot projection incomplete'; END IF;
 IF (w.phase='terminal') IS DISTINCT FROM EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='outcome' AND plaintext_bytes>0) OR (w.phase='terminal') IS DISTINCT FROM EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='completion' AND plaintext_bytes>0) THEN RAISE EXCEPTION 'wait terminal headroom or receipt incomplete'; END IF;
 IF (w.cancel_operation_id IS NOT NULL) IS DISTINCT FROM EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='cancellation' AND plaintext_bytes>0) THEN RAISE EXCEPTION 'wait cancellation receipt incomplete'; END IF;
 IF (w.phase<>'terminal' AND (w.resume_state<>'none' OR EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='resume' AND plaintext_bytes>0))) OR (w.phase='terminal' AND NOT w.retired AND w.resume_state<>'pending') THEN RAISE EXCEPTION 'wait resume state does not match registry phase'; END IF;
 IF w.resume_state='pending' AND (w.phase<>'terminal' OR w.retired OR NOT EXISTS(SELECT 1 FROM wait_owner_slots WHERE wait_id=w.wait_id AND kind='resume' AND plaintext_bytes>0)) THEN RAISE EXCEPTION 'wait resume intent not linked to terminal'; END IF;
 IF NOT EXISTS(SELECT 1 FROM wait_owner_work WHERE wait_id=w.wait_id AND generation=w.work_generation AND next_due_ms=w.next_due_ms AND transition_at_ms BETWEEN w.created_at_ms AND w.logical_now_ms AND ((w.phase='terminal' AND state='done') OR (w.phase='recovery_held' AND state='held') OR (w.phase IN ('installing','pending') AND state IN ('pending','leased','idle')))) THEN RAISE EXCEPTION 'wait work projection mismatch'; END IF;
 IF EXISTS(SELECT 1 FROM wait_owner_members m LEFT JOIN wait_owner_edges e USING(wait_id,ordinal) WHERE m.wait_id=w.wait_id AND ((m.blocker_kind IS NULL AND e.wait_id IS NOT NULL) OR (m.blocker_kind IS NOT NULL AND (e.wait_id IS NULL OR e.waiter_key<>'task/'||w.project_id::text||'/'||w.task_id::text OR e.blocker_key<>CASE m.blocker_kind WHEN 'task' THEN 'task/'||w.project_id::text||'/'||m.blocker_id::text ELSE 'wait/'||m.blocker_id::text END OR e.active<>(w.phase<>'terminal' AND NOT w.retired))))) THEN RAISE EXCEPTION 'wait dependency edge projection mismatch'; END IF;
END $$;
CREATE FUNCTION wait38_consistency() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE candidate UUID; b wait_owner_budgets%ROWTYPE; n BIGINT;
BEGIN
 IF TG_TABLE_NAME='wait_owner_budgets' THEN
  SELECT * INTO b FROM wait_owner_budgets WHERE scope_key=NEW.scope_key;
  IF b.scope_key='installation' THEN SELECT count(*) INTO n FROM wait_owner_records;
  ELSE SELECT count(*) INTO n FROM wait_owner_records WHERE 'project/'||workspace_id::text||'/'||project_id::text=b.scope_key; END IF;
  IF b.envelopes<>n OR b.reserved_bytes<>n*1048576 OR (b.scope_key<>'installation' AND n=0) THEN RAISE EXCEPTION 'wait envelope accounting mismatch'; END IF;
 ELSIF TG_TABLE_NAME='wait_owner_projects' THEN
  IF NOT EXISTS(SELECT 1 FROM wait_owner_records WHERE workspace_id=NEW.workspace_id AND project_id=NEW.project_id) THEN RAISE EXCEPTION 'orphan wait project gate'; END IF;
 ELSIF TG_TABLE_NAME='wait_owner_tasks' THEN
  IF NOT EXISTS(SELECT 1 FROM wait_owner_records WHERE workspace_id=NEW.workspace_id AND project_id=NEW.project_id AND task_id=NEW.task_id AND lineage_id=NEW.lineage_id) THEN RAISE EXCEPTION 'orphan wait task gate'; END IF;
  IF NEW.active_wait_id IS NOT NULL THEN
   IF NOT EXISTS(SELECT 1 FROM wait_owner_records WHERE wait_id=NEW.active_wait_id AND workspace_id=NEW.workspace_id AND project_id=NEW.project_id AND task_id=NEW.task_id AND lineage_id=NEW.lineage_id AND NOT retired) THEN RAISE EXCEPTION 'foreign or retired wait task pointer'; END IF;
   PERFORM wait38_check_wait(NEW.active_wait_id); END IF;
  IF TG_OP='UPDATE' AND OLD.active_wait_id IS NOT NULL AND OLD.active_wait_id IS DISTINCT FROM NEW.active_wait_id THEN PERFORM wait38_check_wait(OLD.active_wait_id); END IF;
 ELSE
  PERFORM wait38_check_wait(NEW.wait_id);
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER wait_owner_budget_consistency AFTER INSERT OR UPDATE ON wait_owner_budgets DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wait38_consistency();
CREATE CONSTRAINT TRIGGER wait_owner_project_consistency AFTER INSERT OR UPDATE ON wait_owner_projects DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wait38_consistency();
CREATE CONSTRAINT TRIGGER wait_owner_task_consistency AFTER INSERT OR UPDATE ON wait_owner_tasks DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wait38_consistency();
CREATE CONSTRAINT TRIGGER wait_owner_record_consistency AFTER INSERT OR UPDATE ON wait_owner_records DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wait38_consistency();
CREATE CONSTRAINT TRIGGER wait_owner_member_consistency AFTER INSERT OR UPDATE ON wait_owner_members DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wait38_consistency();
CREATE CONSTRAINT TRIGGER wait_owner_edge_consistency AFTER INSERT OR UPDATE ON wait_owner_edges DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wait38_consistency();
CREATE CONSTRAINT TRIGGER wait_owner_work_consistency AFTER INSERT OR UPDATE ON wait_owner_work DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wait38_consistency();
CREATE CONSTRAINT TRIGGER wait_owner_slot_consistency AFTER INSERT OR UPDATE ON wait_owner_slots DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wait38_consistency();
UPDATE meta SET value='38' WHERE key='state_format_version';
