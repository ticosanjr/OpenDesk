-- Idempotent enqueue: identical active work is returned instead of duplicated.
ALTER TABLE execution_queue ADD COLUMN dedupe_key TEXT;

CREATE INDEX IF NOT EXISTS idx_queue_dedupe
    ON execution_queue(workspace_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_queue_active_dedupe
    ON execution_queue(workspace_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL AND status IN ('QUEUED', 'RUNNING');
