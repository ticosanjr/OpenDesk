-- Paperclip-style task status, separate workflow phase, attempt metadata and
-- persistent user messages for the execution conversation.
ALTER TABLE tasks ADD COLUMN status TEXT NOT NULL DEFAULT 'BACKLOG';
ALTER TABLE tasks ADD COLUMN phase TEXT;
ALTER TABLE tasks ADD COLUMN last_run_id TEXT;
ALTER TABLE tasks ADD COLUMN last_run_status TEXT;
ALTER TABLE tasks ADD COLUMN last_run_at TEXT;
ALTER TABLE tasks ADD COLUMN blocked_reason TEXT;
ALTER TABLE tasks ADD COLUMN cancelled_at TEXT;

UPDATE tasks SET status = CASE column_stage
    WHEN 'PLAN' THEN 'TODO'
    WHEN 'BUILD' THEN 'IN_PROGRESS'
    WHEN 'REVIEW' THEN 'IN_REVIEW'
    WHEN 'DONE' THEN 'DONE'
    ELSE 'BACKLOG'
END;

UPDATE tasks SET phase = column_stage
WHERE column_stage IN ('PLAN', 'BUILD', 'REVIEW');

CREATE INDEX IF NOT EXISTS idx_tasks_status
    ON tasks (workspace_id, status, position_index);

ALTER TABLE task_runs ADD COLUMN attempt_number INTEGER NOT NULL DEFAULT 1;
ALTER TABLE task_runs ADD COLUMN session_id TEXT;

CREATE TABLE IF NOT EXISTS task_messages (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    kind TEXT NOT NULL DEFAULT 'comment',
    content TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_task_messages_task
    ON task_messages (task_id, created_at ASC);
