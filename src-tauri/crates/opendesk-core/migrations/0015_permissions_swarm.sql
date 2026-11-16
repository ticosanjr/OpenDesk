-- Interactive permission requests raised by the OpenCode engine while a run
-- is waiting for a human decision.
CREATE TABLE IF NOT EXISTS permission_requests (
    id TEXT PRIMARY KEY,
    run_id TEXT REFERENCES task_runs(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    request_id TEXT NOT NULL UNIQUE,
    permission_action TEXT NOT NULL,
    resources TEXT,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'DECIDED', 'EXPIRED')),
    decision TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    decided_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_permission_requests_pending
    ON permission_requests(status, workspace_id, created_at);

-- Tags are already stored as JSON; this helps the swarm comparison query.
CREATE INDEX IF NOT EXISTS idx_tasks_tags ON tasks(tags_json);
