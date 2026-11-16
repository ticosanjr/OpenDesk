-- Declarative recipes, execution state and human-in-the-loop checkpoints.
-- YAML files remain the source of truth; these tables are operational state.

CREATE TABLE IF NOT EXISTS recipes_index (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    content_sha256 TEXT,
    trigger_type TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT 1,
    last_error TEXT,
    last_run_at TEXT,
    total_runs INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX IF NOT EXISTS idx_recipes_workspace ON recipes_index(workspace_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_recipes_trigger ON recipes_index(trigger_type);

CREATE TABLE IF NOT EXISTS recipe_runs (
    id TEXT PRIMARY KEY,
    recipe_id TEXT NOT NULL REFERENCES recipes_index(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK(status IN (
        'RUNNING_STEP', 'WAITING_HITL', 'RUNNING_SINKS',
        'COMPLETED', 'REJECTED', 'FAILED', 'TIMED_OUT'
    )),
    current_step_id TEXT,
    trigger_context_json TEXT NOT NULL,
    step_outputs_json TEXT NOT NULL DEFAULT '{}',
    output_dir_path TEXT NOT NULL,
    dry_run INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_recipe_runs_recipe ON recipe_runs(recipe_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_recipe_runs_workspace ON recipe_runs(workspace_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_recipe_runs_status ON recipe_runs(status);

CREATE TABLE IF NOT EXISTS hitl_requests (
    token TEXT PRIMARY KEY,
    recipe_run_id TEXT NOT NULL REFERENCES recipe_runs(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    checkpoint_id TEXT NOT NULL,
    interaction_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK(status IN ('PENDING','RESOLVED','EXPIRED','CANCELLED')),
    on_timeout TEXT NOT NULL DEFAULT 'PAUSE_AND_ALERT',
    dispatched_channels_json TEXT NOT NULL DEFAULT '{}',
    resolved_by_channel TEXT,
    resolved_by_sender TEXT,
    resolution_type TEXT,
    resolution_payload TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_hitl_status ON hitl_requests(status, expires_at);
CREATE INDEX IF NOT EXISTS idx_hitl_run ON hitl_requests(recipe_run_id);
CREATE INDEX IF NOT EXISTS idx_hitl_workspace ON hitl_requests(workspace_id, status);
