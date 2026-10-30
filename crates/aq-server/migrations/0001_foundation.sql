CREATE TABLE meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO meta (key, value) VALUES ('state_format_version', '1');

CREATE TABLE users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('owner', 'viewer')),
    issuer TEXT,
    subject TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    bound_at TIMESTAMPTZ,
    UNIQUE (issuer, subject)
);
CREATE UNIQUE INDEX users_email_lower ON users (lower(email));

CREATE TABLE enrollment_codes (
    id UUID PRIMARY KEY,
    code_hash TEXT NOT NULL UNIQUE,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    node_id UUID
);

CREATE TABLE nodes (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    credential_hash TEXT NOT NULL UNIQUE,
    worker_version TEXT NOT NULL,
    protocol_version INTEGER NOT NULL,
    enrolled_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);
