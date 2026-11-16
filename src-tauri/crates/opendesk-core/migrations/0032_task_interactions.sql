CREATE TABLE IF NOT EXISTS task_interactions (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    question TEXT NOT NULL,
    context_markdown TEXT,
    options_json TEXT NOT NULL DEFAULT '[]',
    multiple INTEGER NOT NULL DEFAULT 0,
    allow_custom INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK(status IN ('PENDING','RESOLVED','EXPIRED','CANCELLED')),
    answer_json TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_task_interactions_task
    ON task_interactions(task_id, status);
CREATE INDEX IF NOT EXISTS idx_task_interactions_workspace
    ON task_interactions(workspace_id, status, created_at);
