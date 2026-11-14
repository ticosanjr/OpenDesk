-- Completion watchers subscribe a task to notifications on every terminal run
-- (success, failure or cancellation). The recipient is optional: when empty,
-- the channel allowlist is used.
CREATE TABLE IF NOT EXISTS task_watchers (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    channel TEXT NOT NULL CHECK(channel IN ('telegram', 'signal')),
    recipient TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_watchers_unique
    ON task_watchers(task_id, channel, IFNULL(recipient, ''));

CREATE INDEX IF NOT EXISTS idx_task_watchers_task
    ON task_watchers(task_id);
