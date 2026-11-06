-- Phase 4: deterministic output directories for scheduled jobs and
-- idempotency for a schedule slot.

ALTER TABLE execution_queue ADD COLUMN output_dir TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_queue_schedule_slot
    ON execution_queue (schedule_id, scheduled_for)
    WHERE schedule_id IS NOT NULL AND scheduled_for IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_schedules_due
    ON schedules (is_active, next_run_at);
