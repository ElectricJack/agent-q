-- AQ owns bridge admission/authorization/receipts. The embedded engine owns its
-- separate aq_playbooks schema. Registration waits for ordered native0022.
CREATE TABLE playbook_namespaces (
 namespace_id uuid PRIMARY KEY,
 generation bigint NOT NULL CHECK (generation > 0),
 name text NOT NULL UNIQUE CHECK (octet_length(name) BETWEEN 1 AND 128),
 workspace_id uuid NOT NULL,
 project_id uuid NOT NULL,
 owner_user_id uuid NOT NULL REFERENCES users(id),
 catalog_digest text NOT NULL CHECK (octet_length(catalog_digest) BETWEEN 64 AND 128),
 source_digest text NOT NULL CHECK (octet_length(source_digest) BETWEEN 64 AND 128),
 runtime_revision bigint NOT NULL DEFAULT 0 CHECK (runtime_revision >= 0),
 runtime_incarnation uuid,
 grant_count bigint NOT NULL DEFAULT 0 CHECK (grant_count BETWEEN 0 AND 1024),
 admission_count bigint NOT NULL DEFAULT 0 CHECK (admission_count BETWEEN 0 AND 4096),
 effect_count bigint NOT NULL DEFAULT 0 CHECK (effect_count BETWEEN 0 AND 8192),
 cancellation_count bigint NOT NULL DEFAULT 0 CHECK (cancellation_count BETWEEN 0 AND 8192),
 outbox_count bigint NOT NULL DEFAULT 0 CHECK (outbox_count BETWEEN 0 AND 16384),
 retained_bytes bigint NOT NULL DEFAULT 0 CHECK (retained_bytes BETWEEN 0 AND 67108864),
 UNIQUE(namespace_id,generation)
);
CREATE TABLE playbook_grants (
 grant_id uuid PRIMARY KEY,
 namespace_id uuid NOT NULL,
 generation bigint NOT NULL,
 revision bigint NOT NULL CHECK (revision > 0),
 owner_user_id uuid NOT NULL REFERENCES users(id),
 scope jsonb NOT NULL CHECK (octet_length(scope::text) <= 32768),
 expires_at_ms bigint NOT NULL CHECK (expires_at_ms > 0),
 revoked_at_ms bigint CHECK (revoked_at_ms >= 0),
 payload_digest text NOT NULL CHECK (octet_length(payload_digest) BETWEEN 64 AND 128),
 FOREIGN KEY(namespace_id,generation) REFERENCES playbook_namespaces(namespace_id,generation)
);
CREATE TABLE playbook_admissions (
 namespace_id uuid NOT NULL,
 generation bigint NOT NULL,
 request_id uuid NOT NULL,
 request_digest text NOT NULL CHECK (octet_length(request_digest) BETWEEN 64 AND 128),
 engine_key text NOT NULL CHECK (octet_length(engine_key) BETWEEN 1 AND 256),
 grant_id uuid NOT NULL REFERENCES playbook_grants(grant_id),
 grant_revision bigint NOT NULL CHECK (grant_revision > 0),
 expected_activation_revision bigint NOT NULL CHECK (expected_activation_revision > 0),
 sealed_version integer NOT NULL CHECK (sealed_version=1),
 key_id text NOT NULL CHECK (octet_length(key_id) BETWEEN 1 AND 64),
 nonce bytea NOT NULL CHECK (octet_length(nonce)=12),
 ciphertext bytea NOT NULL CHECK (octet_length(ciphertext) BETWEEN 16 AND 65552),
 created_at_ms bigint NOT NULL,
 status text NOT NULL CHECK (status IN ('pending','engine_accepted','refused','cancelled','unknown')),
 receipt jsonb CHECK (octet_length(receipt::text) <= 16384),
 PRIMARY KEY(namespace_id,generation,request_id),
 UNIQUE(namespace_id,generation,engine_key),
 FOREIGN KEY(namespace_id,generation) REFERENCES playbook_namespaces(namespace_id,generation)
);
CREATE TABLE playbook_effects (
 namespace_id uuid NOT NULL,
 generation bigint NOT NULL,
 intent_id text NOT NULL CHECK (octet_length(intent_id) BETWEEN 1 AND 512),
 run_id text NOT NULL CHECK (octet_length(run_id) BETWEEN 1 AND 512),
 command_name text NOT NULL CHECK (command_name = 'message.send'),
 request_digest text NOT NULL CHECK (octet_length(request_digest) BETWEEN 64 AND 128),
 command_fingerprint text NOT NULL CHECK (octet_length(command_fingerprint) BETWEEN 64 AND 128),
 input_digest text NOT NULL CHECK (octet_length(input_digest) BETWEEN 64 AND 128),
 grant_id uuid NOT NULL REFERENCES playbook_grants(grant_id),
 grant_revision bigint NOT NULL CHECK (grant_revision > 0),
 operation_id uuid NOT NULL UNIQUE,
 status text NOT NULL CHECK (status IN ('queued','refused','cancelled')),
 receipt jsonb NOT NULL CHECK (octet_length(receipt::text) <= 16384),
 created_at_ms bigint NOT NULL,
 PRIMARY KEY(namespace_id,generation,intent_id),
 FOREIGN KEY(namespace_id,generation) REFERENCES playbook_namespaces(namespace_id,generation)
);
CREATE TABLE playbook_cancellations (
 namespace_id uuid NOT NULL,
 generation bigint NOT NULL,
 cancellation_key text NOT NULL CHECK (octet_length(cancellation_key) BETWEEN 1 AND 512),
 intent_id text NOT NULL CHECK (octet_length(intent_id) BETWEEN 1 AND 512),
 request_digest text NOT NULL CHECK (octet_length(request_digest) BETWEEN 64 AND 128),
 disposition text NOT NULL CHECK (disposition IN ('accepted','refused','too_late','confirmed','unknown')),
 created_at_ms bigint NOT NULL,
 PRIMARY KEY(namespace_id,generation,cancellation_key),
 UNIQUE(namespace_id,generation,intent_id),
 FOREIGN KEY(namespace_id,generation) REFERENCES playbook_namespaces(namespace_id,generation)
);
CREATE TABLE playbook_outbox (
 namespace_id uuid NOT NULL,
 generation bigint NOT NULL,
 producer text NOT NULL CHECK (producer = 'aq.host.bridge@1'),
 epoch bigint NOT NULL CHECK (epoch=0),
 event_id uuid NOT NULL,
 event_digest text NOT NULL CHECK (octet_length(event_digest) BETWEEN 64 AND 128),
 sequence bigint GENERATED ALWAYS AS IDENTITY,
 sealed_version integer NOT NULL CHECK (sealed_version=1),
 key_id text NOT NULL CHECK (octet_length(key_id) BETWEEN 1 AND 64),
 nonce bytea NOT NULL CHECK (octet_length(nonce)=12),
 ciphertext bytea NOT NULL CHECK (octet_length(ciphertext) BETWEEN 16 AND 65552),
 created_at_ms bigint NOT NULL,
 status text NOT NULL CHECK (status IN ('pending','engine_accepted','unknown')),
 fanout_complete boolean NOT NULL DEFAULT false,
 engine_receipt jsonb CHECK (octet_length(engine_receipt::text) <= 16384),
 PRIMARY KEY(namespace_id,generation,event_id),
 UNIQUE(namespace_id,generation,sequence),
 FOREIGN KEY(namespace_id,generation) REFERENCES playbook_namespaces(namespace_id,generation)
);
CREATE INDEX playbook_admissions_pending ON playbook_admissions(namespace_id,generation,status,created_at_ms,request_id);
CREATE INDEX playbook_outbox_pending ON playbook_outbox(namespace_id,generation,status,sequence);
