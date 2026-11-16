CREATE TABLE IF NOT EXISTS personal_context_groups (
    id TEXT PRIMARY KEY,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK(scope IN ('workspace', 'global')),
    name TEXT NOT NULL,
    instructions TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_personal_context_scope
    ON personal_context_groups(scope, workspace_id, position);
