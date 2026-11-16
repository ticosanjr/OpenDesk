-- Structured failure details for task runs. The queue already stores
-- error_message; runs need the same information so the UI can explain a
-- failure even after the queue job is cleaned up or retried.
ALTER TABLE task_runs ADD COLUMN error_code TEXT;
ALTER TABLE task_runs ADD COLUMN error_message TEXT;
ALTER TABLE task_runs ADD COLUMN error_hint TEXT;
