-- Local calendar cache. Phase 3 stores local events; Phase 4 imports Google
-- calendars into the same tables and adds incremental sync tokens.
CREATE TABLE IF NOT EXISTS calendar_sources (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    provider TEXT NOT NULL DEFAULT 'LOCAL'
        CHECK (provider IN ('LOCAL', 'GOOGLE')),
    external_id TEXT,
    name TEXT NOT NULL,
    color_hex TEXT NOT NULL DEFAULT '#3b82f6',
    timezone TEXT NOT NULL DEFAULT 'UTC',
    is_visible INTEGER NOT NULL DEFAULT 1,
    is_read_only INTEGER NOT NULL DEFAULT 0,
    sync_enabled INTEGER NOT NULL DEFAULT 0,
    sync_token TEXT,
    last_synced_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_calendar_sources_external
    ON calendar_sources(workspace_id, provider, external_id)
    WHERE external_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS calendar_events (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    source_id TEXT REFERENCES calendar_sources(id) ON DELETE CASCADE,
    external_id TEXT,
    title TEXT NOT NULL,
    description TEXT,
    location TEXT,
    start_at TEXT NOT NULL,
    end_at TEXT NOT NULL,
    all_day INTEGER NOT NULL DEFAULT 0,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    status TEXT NOT NULL DEFAULT 'CONFIRMED'
        CHECK (status IN ('CONFIRMED', 'TENTATIVE', 'CANCELLED')),
    color_hex TEXT,
    recurrence_rule TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_calendar_events_range
    ON calendar_events(workspace_id, start_at, end_at);
CREATE INDEX IF NOT EXISTS idx_calendar_events_source
    ON calendar_events(source_id, start_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_calendar_events_external
    ON calendar_events(source_id, external_id)
    WHERE external_id IS NOT NULL;

-- One row per recipe/event occurrence already fired. The unique key makes the
-- future calendar trigger idempotent across restarts.
CREATE TABLE IF NOT EXISTS calendar_trigger_fires (
    recipe_id TEXT NOT NULL,
    event_id TEXT NOT NULL,
    occurrence_start TEXT NOT NULL,
    fired_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (recipe_id, event_id, occurrence_start)
);
