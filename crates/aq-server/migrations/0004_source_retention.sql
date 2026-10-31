-- Transport receipts are independent of attempt authority and task acceptance.
CREATE TABLE source_transfers (
 transfer_id UUID PRIMARY KEY,
 attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
 node_id UUID NOT NULL REFERENCES nodes(id),
 repository_id UUID NOT NULL,
 artifact_id UUID NOT NULL UNIQUE,
 import_operation_id UUID NOT NULL UNIQUE,
 manifest JSONB NOT NULL,
 manifest_offset BIGINT NOT NULL DEFAULT 0,
 bundle_offset BIGINT NOT NULL DEFAULT 0,
 state TEXT NOT NULL DEFAULT 'uploading' CHECK(state IN ('uploading','import_pending','retained','rejected')),
 receipt JSONB,
 central_receipt JSONB,
 dispatch_intent BOOLEAN NOT NULL DEFAULT FALSE,
 staging_released BOOLEAN NOT NULL DEFAULT FALSE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX source_transfers_owner ON source_transfers(node_id,attempt_id);
CREATE TABLE source_transfer_operations (
 node_id UUID NOT NULL REFERENCES nodes(id),
 operation_id UUID NOT NULL,
 payload_digest TEXT NOT NULL,
 response JSONB NOT NULL,
 PRIMARY KEY(node_id,operation_id)
);
-- Trusted owner selects a retained base handle, never a path or remote URL.
CREATE TABLE source_bases (
 attempt_id UUID PRIMARY KEY REFERENCES execution_attempts(attempt_id),
 node_id UUID NOT NULL REFERENCES nodes(id),
 operation_id UUID NOT NULL UNIQUE,
 retention_handle UUID NOT NULL,
 reserved_bytes BIGINT NOT NULL,
 manifest JSONB,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

UPDATE meta SET value='4' WHERE key='state_format_version';
