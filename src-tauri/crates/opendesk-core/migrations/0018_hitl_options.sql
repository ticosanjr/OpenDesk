-- Options for MULTI_CHOICE HITL checkpoints. Kept as a separate migration so
-- databases created by v0.22.0 do not see a modified checksum.
ALTER TABLE hitl_requests ADD COLUMN options_json TEXT NOT NULL DEFAULT '[]';
