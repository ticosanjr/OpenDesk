-- Separate human work from automation executions and add a stable key for
-- duplicate prevention. Existing recipe/io-bus tasks are backfilled from tags.
ALTER TABLE tasks ADD COLUMN origin TEXT NOT NULL DEFAULT 'MANUAL';
ALTER TABLE tasks ADD COLUMN parent_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL;
ALTER TABLE tasks ADD COLUMN dedupe_key TEXT;

UPDATE tasks SET origin = 'RECIPE'
WHERE tags_json LIKE '%"recipe:%';
UPDATE tasks SET origin = 'IO_BUS'
WHERE tags_json LIKE '%"io-source:%';
UPDATE tasks SET origin = 'CAPTURE'
WHERE tags_json LIKE '%"quick-capture"%';

CREATE INDEX IF NOT EXISTS idx_tasks_origin
    ON tasks(workspace_id, origin, status);
CREATE INDEX IF NOT EXISTS idx_tasks_dedupe
    ON tasks(workspace_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL;
-- At most one active task with the same fingerprint per workspace. Terminal
-- tasks are deliberately outside the constraint so reruns remain possible.
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_active_dedupe
    ON tasks(workspace_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL
      AND status IN ('TODO', 'IN_PROGRESS', 'IN_REVIEW');
