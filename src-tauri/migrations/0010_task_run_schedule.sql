-- Associate runs with the routine that produced them.
ALTER TABLE task_runs ADD COLUMN schedule_id TEXT;
CREATE INDEX IF NOT EXISTS idx_task_runs_schedule
    ON task_runs (schedule_id, started_at DESC);
