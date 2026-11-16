-- Distinguish rules added in the UI from rules the agent saved after an
-- explicit user request. The quote makes agent writes auditable.
ALTER TABLE personal_context_groups ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE personal_context_groups ADD COLUMN source_quote TEXT;
