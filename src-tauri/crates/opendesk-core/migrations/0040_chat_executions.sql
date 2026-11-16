-- Chat turns become first-class executions without pretending every message is
-- a task. A task_run is created only when the turn uses tools or changes files;
-- chat_turns always records tokens and status for observability.
ALTER TABLE task_runs ADD COLUMN source TEXT;
ALTER TABLE task_runs ADD COLUMN channel TEXT;
ALTER TABLE task_runs ADD COLUMN origin_ref TEXT;

UPDATE task_runs
SET source = COALESCE(
    (SELECT CASE
        WHEN tasks.origin = 'RECIPE' THEN 'recipe'
        WHEN tasks.origin = 'ROUTINE' THEN 'routine'
        WHEN tasks.origin IN ('IO_BUS', 'DELEGATION') THEN 'automation'
        ELSE 'task'
     END
     FROM tasks WHERE tasks.id = task_runs.task_id),
    CASE WHEN schedule_id IS NOT NULL THEN 'routine' ELSE 'task' END
);

CREATE INDEX IF NOT EXISTS idx_task_runs_source
    ON task_runs(source, started_at);

CREATE TABLE IF NOT EXISTS chat_turns (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    thread_id TEXT NOT NULL REFERENCES chat_threads(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    source TEXT NOT NULL,
    status TEXT NOT NULL,
    agent TEXT NOT NULL,
    model TEXT,
    tokens_input INTEGER NOT NULL DEFAULT 0,
    tokens_output INTEGER NOT NULL DEFAULT 0,
    tokens_reasoning INTEGER NOT NULL DEFAULT 0,
    tokens_cache_read INTEGER NOT NULL DEFAULT 0,
    tokens_cache_write INTEGER NOT NULL DEFAULT 0,
    estimated_cost_usd REAL NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    user_content TEXT NOT NULL,
    assistant_content TEXT,
    error_code TEXT,
    error_message TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_chat_turns_thread
    ON chat_turns(thread_id, started_at);
CREATE INDEX IF NOT EXISTS idx_chat_turns_source
    ON chat_turns(source, started_at);
