-- Staged until migrations015/016 are integrated. Existing human Decision JSON,
-- payload digests and operation namespaces remain unchanged.
ALTER TABLE intelligence_decisions ALTER COLUMN actor_id DROP NOT NULL;
ALTER TABLE intelligence_decisions
 ADD COLUMN authority_kind TEXT NOT NULL DEFAULT 'human_owner',
 ADD COLUMN authority_project_id UUID REFERENCES project_execution_bindings(project_id),
 ADD COLUMN authority_policy_ref JSONB,
 ADD COLUMN authority_policy_revision BIGINT,
 ADD COLUMN authority_policy_digest TEXT,
 ADD CONSTRAINT intelligence_decision_principal_shape CHECK (
  (authority_kind='human_owner' AND actor_id IS NOT NULL
   AND authority_project_id IS NULL AND authority_policy_ref IS NULL
   AND authority_policy_revision IS NULL AND authority_policy_digest IS NULL)
  OR
  (authority_kind='project_policy' AND actor_id IS NULL
   AND authority_project_id IS NOT NULL AND authority_policy_ref IS NOT NULL
   AND octet_length(authority_policy_ref::text)<=2048
   AND authority_policy_revision IS NOT NULL AND authority_policy_revision>0
   AND authority_policy_digest IS NOT NULL AND authority_policy_digest ~ '^[0-9a-f]{64}$'
   AND authority_policy_ref->>'digest' IS NOT DISTINCT FROM authority_policy_digest
   AND authority_policy_ref->>'version' IS NOT DISTINCT FROM authority_policy_revision::text
   AND authority_policy_ref->>'id' IS NOT DISTINCT FROM ('aq.project-source-policy/'||authority_project_id::text))
 );
CREATE UNIQUE INDEX intelligence_policy_operation ON intelligence_decisions(authority_project_id,authority_policy_digest,operation_id)
 WHERE authority_kind='project_policy';
-- Existing immutable decision trigger also protects the principal columns.
-- Ordered registration advances state_format_version after015/016 are present.
