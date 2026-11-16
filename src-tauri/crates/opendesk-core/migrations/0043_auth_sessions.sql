-- Persistent remote sessions for the PWA/CLI. The raw token is never stored;
-- only its SHA-256 hash.
CREATE TABLE IF NOT EXISTS auth_sessions (
    id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT 'remote',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT,
    expires_at TEXT NOT NULL,
    revoked_at TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_sessions_hash
    ON auth_sessions(token_hash);
CREATE INDEX IF NOT EXISTS idx_auth_sessions_expiry
    ON auth_sessions(expires_at);
