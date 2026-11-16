-- Memory lifecycle: archived notes stay auditable instead of being deleted;
-- access counters support future consolidation heuristics.
ALTER TABLE memory_notes ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE memory_notes ADD COLUMN last_accessed_at TEXT;
ALTER TABLE memory_notes ADD COLUMN archived_at TEXT;
CREATE INDEX IF NOT EXISTS idx_memory_notes_archived
    ON memory_notes(workspace_id, archived_at);
