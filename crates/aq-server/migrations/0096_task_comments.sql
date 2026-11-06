-- Task comments (aq-dashboard spec 0004 §4.1). The discussion on one task,
-- keyed by (project_id, task_id); the Graph stays the record of task content.
-- A delete is a tombstone: the body is cleared and the row is kept. `seq` is
-- the insertion order that threads page by (newest first).
CREATE TABLE task_comments (
 comment_id UUID PRIMARY KEY,
 seq BIGINT GENERATED ALWAYS AS IDENTITY UNIQUE,
 project_id UUID NOT NULL,
 task_id UUID NOT NULL,
 author_kind TEXT NOT NULL CHECK(author_kind IN ('owner','agent','supervisor','system')),
 author_ref JSONB NOT NULL CHECK(jsonb_typeof(author_ref)='object' AND author_ref->>'kind'=author_kind AND octet_length(author_ref::text)<=1024),
 body TEXT CHECK(octet_length(body) BETWEEN 1 AND 16384),
 created_at TIMESTAMPTZ NOT NULL,
 edited_at TIMESTAMPTZ,
 deleted_at TIMESTAMPTZ,
 revision BIGINT NOT NULL CHECK(revision>0),
 CHECK((deleted_at IS NULL)=(body IS NOT NULL))
);
CREATE INDEX task_comments_thread ON task_comments(project_id,task_id,seq DESC);
-- One row per add/edit/delete operation: replaying an operation ID returns
-- its recorded effect, and a changed request under the same ID conflicts.
CREATE TABLE task_comment_operations (
 operation_id UUID PRIMARY KEY,
 kind TEXT NOT NULL CHECK(kind IN ('task.comment.add','task.comment.edit','task.comment.delete')),
 comment_id UUID NOT NULL,
 actor JSONB NOT NULL CHECK(octet_length(actor::text)<=1024),
 fingerprint TEXT NOT NULL CHECK(fingerprint ~ '^[0-9a-f]{64}$'),
 revision BIGINT NOT NULL CHECK(revision>=0),
 created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX task_comment_operations_comment ON task_comment_operations(comment_id);
UPDATE meta SET value='96' WHERE key='state_format_version';
