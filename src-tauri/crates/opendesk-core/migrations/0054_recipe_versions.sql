-- Recipe version history for diff and rollback.
CREATE TABLE IF NOT EXISTS recipe_versions (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    recipe_id TEXT NOT NULL,
    content TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_recipe_versions_recipe
    ON recipe_versions(workspace_id, recipe_id, created_at DESC);
