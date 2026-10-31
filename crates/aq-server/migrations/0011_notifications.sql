-- AQ24 bounded central notification delivery.
-- Notification content, routing and transport attempts have distinct owners.
CREATE TABLE notification_budget (
 singleton BOOLEAN PRIMARY KEY CHECK(singleton),
 reserved_bytes BIGINT NOT NULL DEFAULT 0 CHECK(reserved_bytes BETWEEN 0 AND 134217728),
 message_count BIGINT NOT NULL DEFAULT 0 CHECK(message_count BETWEEN 0 AND 4096),
 delivery_count BIGINT NOT NULL DEFAULT 0 CHECK(delivery_count BETWEEN 0 AND 4096),
 route_count BIGINT NOT NULL DEFAULT 0 CHECK(route_count BETWEEN 0 AND 128),
 operation_count BIGINT NOT NULL DEFAULT 0 CHECK(operation_count BETWEEN 0 AND 8192),
 dispatcher_id UUID,
 scan_after BIGINT NOT NULL DEFAULT 0 CHECK(scan_after>=0),
 configuration_revision BIGINT NOT NULL DEFAULT 0 CHECK(configuration_revision>=0),
 configuration_digest TEXT,
 provider_not_before_ms BIGINT NOT NULL DEFAULT 0 CHECK(provider_not_before_ms>=0)
);
INSERT INTO notification_budget(singleton) VALUES(TRUE);
CREATE TABLE notification_routes (
 route_id UUID PRIMARY KEY,
 project_id UUID NOT NULL REFERENCES project_execution_bindings(project_id),
 workspace_id UUID NOT NULL,
 recipient_user_id UUID NOT NULL REFERENCES users(id),
 revision BIGINT NOT NULL CHECK(revision>0),
 definition JSONB NOT NULL CHECK(octet_length(definition::text)<=16384),
 digest TEXT NOT NULL CHECK(digest ~ '^[0-9a-f]{64}$'),
 endpoint_key TEXT NOT NULL CHECK(octet_length(endpoint_key) BETWEEN 1 AND 128),
 endpoint_revision BIGINT NOT NULL CHECK(endpoint_revision>0),
 enabled BOOLEAN NOT NULL
);
CREATE INDEX notification_routes_recipient ON notification_routes(project_id,recipient_user_id,enabled);
CREATE TABLE notification_destinations (
 endpoint_key TEXT PRIMARY KEY,
 endpoint_revision BIGINT NOT NULL CHECK(endpoint_revision>0),
 configuration_revision BIGINT NOT NULL CHECK(configuration_revision>0),
 available BOOLEAN NOT NULL,
 fingerprint TEXT CHECK(fingerprint ~ '^[0-9a-f]{64}$'),
 not_before_ms BIGINT NOT NULL DEFAULT 0 CHECK(not_before_ms>=0),
 disabled BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE TABLE notification_messages (
 sequence BIGSERIAL UNIQUE NOT NULL,
 message_id UUID PRIMARY KEY,
 revision BIGINT NOT NULL CHECK(revision>0),
 project_id UUID NOT NULL REFERENCES project_execution_bindings(project_id),
 workspace_id UUID NOT NULL,
 recipient_user_id UUID NOT NULL REFERENCES users(id),
 definition JSONB NOT NULL CHECK(octet_length(definition::text)<=16384),
 digest TEXT NOT NULL CHECK(digest ~ '^[0-9a-f]{64}$'),
 logical_digest TEXT NOT NULL CHECK(logical_digest ~ '^[0-9a-f]{64}$'),
 external_state TEXT NOT NULL CHECK(external_state IN ('no_external_route','routed')),
 created_at_ms BIGINT NOT NULL CHECK(created_at_ms>0),
 expires_at_ms BIGINT NOT NULL CHECK(expires_at_ms>created_at_ms)
);
CREATE INDEX notification_messages_inbox ON notification_messages(recipient_user_id,sequence);
CREATE TABLE notification_deliveries (
 sequence BIGSERIAL UNIQUE NOT NULL,
 delivery_id UUID PRIMARY KEY,
 message_id UUID NOT NULL REFERENCES notification_messages(message_id),
 route_id UUID NOT NULL REFERENCES notification_routes(route_id),
 route_revision BIGINT NOT NULL CHECK(route_revision>0),
 endpoint_key TEXT NOT NULL,
 endpoint_revision BIGINT NOT NULL CHECK(endpoint_revision>0),
 record_revision BIGINT NOT NULL DEFAULT 1 CHECK(record_revision>0),
 state JSONB NOT NULL CHECK(octet_length(state::text)<=32768),
 status TEXT NOT NULL CHECK(status IN ('queued','dispatching','accepted_by_transport','delivery_unknown','retry_scheduled','permanently_failed','suppressed')),
 due_at_ms BIGINT NOT NULL CHECK(due_at_ms>=0),
 attempt_id UUID,
 lease_until_ms BIGINT,
 UNIQUE(message_id,route_id,route_revision),
 CHECK((status='dispatching')=(attempt_id IS NOT NULL AND lease_until_ms IS NOT NULL))
);
CREATE INDEX notification_due ON notification_deliveries(status,due_at_ms,sequence);
CREATE INDEX notification_inflight ON notification_deliveries(endpoint_key,status,lease_until_ms);
CREATE TABLE notification_operations (
 actor_id UUID NOT NULL REFERENCES users(id),
 operation_id UUID NOT NULL,
 request_digest TEXT NOT NULL CHECK(request_digest ~ '^[0-9a-f]{64}$'),
 response JSONB NOT NULL CHECK(octet_length(response::text)<=16384),
 PRIMARY KEY(actor_id,operation_id)
);
UPDATE meta SET value='11' WHERE key='state_format_version';
