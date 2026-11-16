-- Passkeys (WebAuthn) and short-lived ceremonies. Ceremony state is stored
-- server-side and is single-use; credentials are JSON from webauthn-rs.
CREATE TABLE IF NOT EXISTS passkey_credentials (
    id TEXT PRIMARY KEY,
    credential_id TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL DEFAULT 'passkey',
    passkey_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT
);
CREATE TABLE IF NOT EXISTS passkey_ceremonies (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('register', 'login')),
    state_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_passkey_ceremonies_expiry
    ON passkey_ceremonies(expires_at);
