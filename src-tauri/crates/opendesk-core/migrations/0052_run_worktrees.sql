-- Isolated run workspaces (Git worktree or overlay copy).
CREATE TABLE IF NOT EXISTS run_worktrees (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    run_id TEXT,
    path TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('git', 'overlay')),
    branch TEXT,
    base_commit TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('ACTIVE', 'MERGED', 'REMOVED', 'CONFLICT')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    merged_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_run_worktrees_workspace
    ON run_worktrees(workspace_id, created_at DESC);
