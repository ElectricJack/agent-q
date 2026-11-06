-- Spec 0006 phase 2: tasks an agent created, so a child's grants never
-- exceed its creator's (inheritance), the per-attempt creation quota holds,
-- and task:subtree reaches the tasks below the attempt's own task. A row is
-- reserved when the create is admitted, before the Graph write.
CREATE TABLE agent_created_tasks (
 task_id UUID PRIMARY KEY,
 attempt_id UUID NOT NULL REFERENCES agent_launch_bindings(attempt_id),
 project_id UUID NOT NULL,
 parent_id UUID,
 -- The creating attempt's own task; depth counts levels below it (1 = a
 -- child). NULL for a task outside that subtree (a project-level create).
 root_task_id UUID NOT NULL,
 depth INTEGER CHECK (depth BETWEEN 1 AND 64),
 operation_id UUID NOT NULL,
 created_at_ms BIGINT NOT NULL CHECK (created_at_ms>=0)
);
CREATE INDEX agent_created_tasks_attempt ON agent_created_tasks(attempt_id);
CREATE INDEX agent_created_tasks_root ON agent_created_tasks(root_task_id, task_id);
UPDATE meta SET value='98' WHERE key='state_format_version';
