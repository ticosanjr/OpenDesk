-- Routines can be triggered by any input source and can execute either a
-- prompt/command or a declarative recipe.
ALTER TABLE schedules ADD COLUMN trigger_type TEXT NOT NULL DEFAULT 'CRON';
ALTER TABLE schedules ADD COLUMN trigger_config_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE schedules ADD COLUMN target_kind TEXT NOT NULL DEFAULT 'PROMPT';
ALTER TABLE schedules ADD COLUMN recipe_id TEXT REFERENCES recipes_index(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_schedules_trigger
    ON schedules(workspace_id, trigger_type, is_active);
