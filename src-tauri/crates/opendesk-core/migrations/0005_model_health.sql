-- Models rejected by the provider (region, auth, missing model) are cooled down
-- so the router stops selecting them until the user fixes the provider.
CREATE TABLE IF NOT EXISTS model_health (
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    code TEXT NOT NULL,
    failures INTEGER NOT NULL DEFAULT 1,
    failed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (provider_id, model_id)
);
