-- Continuous learning without a heavyweight pipeline: lessons are derived from
-- run outcomes, deduplicated by a deterministic fingerprint, and promoted to
-- native Studio proposals only after human approval.

CREATE TABLE IF NOT EXISTS learning_lessons (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    task_id TEXT,
    agent_name TEXT,
    model_id TEXT,
    category TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'info',
    title TEXT NOT NULL,
    detail TEXT NOT NULL,
    evidence_json TEXT NOT NULL DEFAULT '{}',
    fingerprint TEXT NOT NULL,
    occurrences INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'OPEN'
        CHECK (status IN ('OPEN', 'DISMISSED', 'PROMOTED')),
    proposal_id TEXT,
    remembered_at TEXT,
    last_seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_learning_lessons_fingerprint
    ON learning_lessons(workspace_id, fingerprint);
CREATE INDEX IF NOT EXISTS idx_learning_lessons_status
    ON learning_lessons(workspace_id, status, last_seen_at DESC);

-- A run is observed at most once, so manual scans are idempotent and do not
-- inflate occurrence counts.
CREATE TABLE IF NOT EXISTS learning_run_captures (
    run_id TEXT PRIMARY KEY REFERENCES task_runs(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    lesson_id TEXT,
    captured_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
