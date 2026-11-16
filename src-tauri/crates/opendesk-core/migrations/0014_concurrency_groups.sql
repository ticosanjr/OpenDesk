-- Limit resource-heavy job classes independently.
ALTER TABLE execution_queue ADD COLUMN concurrency_group TEXT NOT NULL DEFAULT 'default';
