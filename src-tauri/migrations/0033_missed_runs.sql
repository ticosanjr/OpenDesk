CREATE TABLE IF NOT EXISTS missed_runs (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    schedule_id TEXT NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,
    scheduled_for TEXT NOT NULL,
    detected_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK(status IN ('PENDING','QUEUED','IGNORED','FAILED','CANCELLED')),
    run_id TEXT,
    error_message TEXT,
    UNIQUE(schedule_id, scheduled_for)
);

CREATE INDEX IF NOT EXISTS idx_missed_runs_workspace
    ON missed_runs(workspace_id, status, scheduled_for);
