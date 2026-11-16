-- Web Push subscriptions for the PWA. Only the browser-provided endpoint and
-- encryption keys are stored; VAPID signing keys live in a 0600 file.
CREATE TABLE IF NOT EXISTS push_subscriptions (
    id TEXT PRIMARY KEY,
    endpoint TEXT NOT NULL,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT 'device',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_success_at TEXT,
    last_failure_at TEXT,
    failure_count INTEGER NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_push_subscriptions_endpoint
    ON push_subscriptions(endpoint);
