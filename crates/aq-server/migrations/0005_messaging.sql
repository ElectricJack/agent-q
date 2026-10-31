-- Mailbox scope is explicit; task mail survives attempts, session mail never retargets.
CREATE TABLE mailbox_heads (
 mailbox_key TEXT PRIMARY KEY,
 next_cursor BIGINT NOT NULL DEFAULT 1 CHECK(next_cursor > 0)
);
CREATE TABLE mailbox_messages (
 message_id UUID PRIMARY KEY,
 mailbox_key TEXT NOT NULL REFERENCES mailbox_heads(mailbox_key),
 cursor BIGINT NOT NULL CHECK(cursor > 0),
 thread_id UUID NOT NULL,
 sender JSONB NOT NULL,
 recipient JSONB NOT NULL,
 payload_digest TEXT NOT NULL,
 body TEXT NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 16384),
 created_at_ms BIGINT NOT NULL,
 UNIQUE(mailbox_key,cursor)
);
CREATE INDEX mailbox_thread_cursor ON mailbox_messages(mailbox_key,thread_id,cursor);
CREATE TABLE mailbox_receipts (
 message_id UUID NOT NULL REFERENCES mailbox_messages(message_id),
 attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
 disposition TEXT NOT NULL CHECK(disposition IN ('received','read')),
 acknowledged_at_ms BIGINT NOT NULL,
 PRIMARY KEY(message_id,attempt_id)
);
CREATE TABLE mailbox_operations (
 sender_key TEXT NOT NULL,
 operation_id UUID NOT NULL,
 payload_digest TEXT NOT NULL,
 response JSONB NOT NULL,
 PRIMARY KEY(sender_key,operation_id)
);
UPDATE meta SET value = '5' WHERE key = 'state_format_version';
-- Delivery intent is per exact attempt; its state does not retarget task mail.
CREATE TABLE mailbox_deliveries (
 delivery_id UUID PRIMARY KEY,
 message_id UUID NOT NULL REFERENCES mailbox_messages(message_id),
 attempt_id UUID NOT NULL REFERENCES execution_attempts(attempt_id),
 disposition TEXT NOT NULL CHECK(disposition IN ('pending','received_by_node','handed_to_harness','delivery_unknown','stale')),
 updated_at_ms BIGINT NOT NULL,
 UNIQUE(message_id,attempt_id)
);
