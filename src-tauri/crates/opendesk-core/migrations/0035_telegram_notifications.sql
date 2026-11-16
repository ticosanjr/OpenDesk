-- Telegram notifications for routines and the delivery ledger.
ALTER TABLE schedules ADD COLUMN notify_telegram INTEGER NOT NULL DEFAULT 0;
ALTER TABLE schedules ADD COLUMN notify_telegram_recipients TEXT;

-- SQLite cannot alter the channel CHECK in place.
CREATE TABLE integration_deliveries_new (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    artifact_id TEXT REFERENCES artifacts_provenance(id) ON DELETE SET NULL,
    channel TEXT NOT NULL CHECK (channel IN ('SMTP', 'GMAIL', 'DRIVE', 'GDOCS', 'GCALENDAR', 'GTASKS', 'SIGNAL', 'TELEGRAM')),
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'RUNNING', 'SUCCESS', 'FAILED', 'SKIPPED')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    external_id TEXT,
    external_url TEXT,
    next_retry_at TEXT,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    payload_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
INSERT INTO integration_deliveries_new (
    id, workspace_id, artifact_id, channel, idempotency_key, status,
    attempts, last_error, external_id, external_url, next_retry_at,
    max_attempts, payload_json, created_at, updated_at
)
SELECT
    id, workspace_id, artifact_id, channel, idempotency_key, status,
    attempts, last_error, external_id, external_url, next_retry_at,
    max_attempts, payload_json, created_at, updated_at
FROM integration_deliveries;
DROP TABLE integration_deliveries;
ALTER TABLE integration_deliveries_new RENAME TO integration_deliveries;
CREATE INDEX IF NOT EXISTS idx_deliveries_status
    ON integration_deliveries (status, channel);
