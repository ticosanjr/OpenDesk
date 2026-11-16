-- Persistent folder-watch deduplication. The fingerprint is mtime (ns) + size;
-- a file is only processed again when this value changes.
CREATE TABLE IF NOT EXISTS recipe_file_state (
    recipe_id TEXT NOT NULL,
    trigger_path TEXT NOT NULL,
    file_path TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'SEEN'
        CHECK(status IN ('SEEN', 'RUNNING', 'COMPLETED', 'FAILED')),
    run_id TEXT,
    processed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (recipe_id, trigger_path, file_path)
);

CREATE INDEX IF NOT EXISTS idx_recipe_file_state_recipe
    ON recipe_file_state(recipe_id, trigger_path, status);
