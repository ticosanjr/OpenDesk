-- Computer-use sessions. Actions are counted and summarized; typed text is
-- never stored.
CREATE TABLE IF NOT EXISTS computer_sessions (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    run_id TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('PENDING', 'ACTIVE', 'ENDED')),
    require_approval INTEGER NOT NULL DEFAULT 0,
    actions INTEGER NOT NULL DEFAULT 0,
    last_action_json TEXT,
    screenshot_path TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ended_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_computer_sessions_workspace
    ON computer_sessions(workspace_id, created_at);
