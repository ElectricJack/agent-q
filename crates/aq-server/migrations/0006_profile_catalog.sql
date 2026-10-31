-- Definitions remain in the configuration vault. These are immutable validated
-- artifacts and the one central activation pointer, never a second editable catalog.
CREATE TABLE profile_catalog_artifacts (
 digest TEXT PRIMARY KEY CHECK(digest ~ '^[0-9a-f]{64}$'),
 artifact BYTEA NOT NULL CHECK(octet_length(artifact) BETWEEN 1 AND 2097152),
 created_by UUID NOT NULL REFERENCES users(id),
 created_at_ms BIGINT NOT NULL
);
CREATE TABLE profile_catalog_registry (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(singleton),
 revision BIGINT NOT NULL CHECK(revision >= 0),
 active_digest TEXT REFERENCES profile_catalog_artifacts(digest),
 state JSONB NOT NULL
);
INSERT INTO profile_catalog_registry(singleton,revision,active_digest,state)
VALUES(TRUE,0,NULL,'{"revision":0,"active_digest":null,"known_identities":{},"retained":{}}');
CREATE TABLE profile_catalog_operations (
 actor_id UUID NOT NULL REFERENCES users(id),
 operation_id UUID NOT NULL,
 payload_digest TEXT NOT NULL,
 response JSONB NOT NULL,
 PRIMARY KEY(actor_id,operation_id)
);
UPDATE meta SET value = '6' WHERE key = 'state_format_version';
