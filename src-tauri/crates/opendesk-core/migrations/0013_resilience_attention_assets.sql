-- Queue lease/heartbeat and per-job concurrency grouping.
ALTER TABLE execution_queue ADD COLUMN lease_expires_at TEXT;
ALTER TABLE execution_queue ADD COLUMN last_heartbeat_at TEXT;

-- Model cooldown with an explicit expiry (rate-limit fallback).
ALTER TABLE model_health ADD COLUMN cooldown_until TEXT;

-- Cron misfire policy and safe background mode.
ALTER TABLE schedules ADD COLUMN misfire_policy TEXT NOT NULL DEFAULT 'SKIP_TO_NEXT';
ALTER TABLE schedules ADD COLUMN safe_readonly_mode INTEGER NOT NULL DEFAULT 1;

-- Deliverable pinning.
ALTER TABLE artifacts_provenance ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;

-- Task forking lineage.
ALTER TABLE tasks ADD COLUMN forked_from_task_id TEXT;
