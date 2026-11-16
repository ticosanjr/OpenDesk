-- Persist what the human is being asked. Channels already rendered the
-- question, but the desktop UI only had checkpoint ids and generic buttons.
ALTER TABLE hitl_requests ADD COLUMN title TEXT;
ALTER TABLE hitl_requests ADD COLUMN question_markdown TEXT;
ALTER TABLE hitl_requests ADD COLUMN context_json TEXT NOT NULL DEFAULT '{}';
