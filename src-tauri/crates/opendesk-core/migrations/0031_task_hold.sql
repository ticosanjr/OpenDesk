-- Explicit hold: a task in the queue that must not auto-start until resumed.
ALTER TABLE tasks ADD COLUMN held INTEGER NOT NULL DEFAULT 0;
