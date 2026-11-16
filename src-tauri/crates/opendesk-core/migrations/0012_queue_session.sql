-- Reuse the OpenCode session for conversational task follow-ups.
ALTER TABLE execution_queue ADD COLUMN session_id TEXT;
