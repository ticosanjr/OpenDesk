-- Recipes are workspace-scoped, but the index used a global id primary key.
-- Installing the same pack in two workspaces moved rows between workspaces and
-- made list/get depend on sync order. Rebuild related tables with a composite
-- business key. `-- no-transaction` lets us disable foreign keys while swapping
-- tables; the app takes a database backup before running migrations.
-- no-transaction
PRAGMA foreign_keys = OFF;

CREATE TABLE recipes_index_new (
    id TEXT NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    content_sha256 TEXT,
    trigger_type TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT 1,
    last_error TEXT,
    last_run_at TEXT,
    total_runs INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (workspace_id, id)
);

CREATE TABLE recipe_runs_new (
    id TEXT PRIMARY KEY,
    recipe_id TEXT NOT NULL,
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
    completed_at TEXT,
    FOREIGN KEY (workspace_id, recipe_id)
        REFERENCES recipes_index_new(workspace_id, id) ON DELETE CASCADE
);

CREATE TABLE hitl_requests_new (
    token TEXT PRIMARY KEY,
    recipe_run_id TEXT NOT NULL REFERENCES recipe_runs_new(id) ON DELETE CASCADE,
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
    resolved_at TEXT,
    options_json TEXT NOT NULL DEFAULT '[]'
);

-- `recipe_id` no longer has a single-column foreign key: recipes are keyed by
-- (workspace_id, id). The routine already carries workspace_id.
CREATE TABLE schedules_new (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    cron_expression TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    agent_name TEXT,
    command_or_prompt TEXT NOT NULL,
    router_policy TEXT NOT NULL DEFAULT 'COST_BENEFIT'
        CHECK (router_policy IN ('COST_BENEFIT', 'CHEAPEST', 'BEST_QUALITY', 'MANUAL', 'CUSTOM_RULES')),
    output_folder_template TEXT NOT NULL DEFAULT 'output/{YYYY}-{MM}/{schedule_slug}',
    notify_email_recipients TEXT,
    sync_to_google_docs INTEGER NOT NULL DEFAULT 0,
    sync_to_google_calendar INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    last_run_at TEXT,
    last_run_status TEXT,
    next_run_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    recurrence_json TEXT,
    one_time INTEGER NOT NULL DEFAULT 0,
    notify_signal INTEGER NOT NULL DEFAULT 0,
    notify_signal_recipients TEXT,
    notify_only_on_change INTEGER NOT NULL DEFAULT 0,
    last_output_hash TEXT,
    misfire_policy TEXT NOT NULL DEFAULT 'SKIP_TO_NEXT',
    safe_readonly_mode INTEGER NOT NULL DEFAULT 1,
    trigger_type TEXT NOT NULL DEFAULT 'CRON',
    trigger_config_json TEXT NOT NULL DEFAULT '{}',
    target_kind TEXT NOT NULL DEFAULT 'PROMPT',
    recipe_id TEXT
);

INSERT INTO recipes_index_new (
    id, workspace_id, name, file_path, content_sha256, trigger_type,
    is_enabled, last_error, last_run_at, total_runs, updated_at
)
SELECT
    id, workspace_id, name, file_path, content_sha256, trigger_type,
    is_enabled, last_error, last_run_at, total_runs, updated_at
FROM recipes_index;

INSERT INTO recipe_runs_new (
    id, recipe_id, workspace_id, task_id, status, current_step_id,
    trigger_context_json, step_outputs_json, output_dir_path, dry_run,
    error_message, started_at, completed_at
)
SELECT
    id, recipe_id, workspace_id, task_id, status, current_step_id,
    trigger_context_json, step_outputs_json, output_dir_path, dry_run,
    error_message, started_at, completed_at
FROM recipe_runs;

INSERT INTO hitl_requests_new (
    token, recipe_run_id, workspace_id, checkpoint_id, interaction_type,
    status, on_timeout, dispatched_channels_json, resolved_by_channel,
    resolved_by_sender, resolution_type, resolution_payload, expires_at,
    created_at, resolved_at, options_json
)
SELECT
    token, recipe_run_id, workspace_id, checkpoint_id, interaction_type,
    status, on_timeout, dispatched_channels_json, resolved_by_channel,
    resolved_by_sender, resolution_type, resolution_payload, expires_at,
    created_at, resolved_at, options_json
FROM hitl_requests;

INSERT INTO schedules_new (
    id, workspace_id, name, description, cron_expression, timezone, agent_name,
    command_or_prompt, router_policy, output_folder_template,
    notify_email_recipients, sync_to_google_docs, sync_to_google_calendar,
    is_active, last_run_at, last_run_status, next_run_at, created_at,
    recurrence_json, one_time, notify_signal, notify_signal_recipients,
    notify_only_on_change, last_output_hash, misfire_policy, safe_readonly_mode,
    trigger_type, trigger_config_json, target_kind, recipe_id
)
SELECT
    id, workspace_id, name, description, cron_expression, timezone, agent_name,
    command_or_prompt, router_policy, output_folder_template,
    notify_email_recipients, sync_to_google_docs, sync_to_google_calendar,
    is_active, last_run_at, last_run_status, next_run_at, created_at,
    recurrence_json, one_time, notify_signal, notify_signal_recipients,
    notify_only_on_change, last_output_hash, misfire_policy, safe_readonly_mode,
    trigger_type, trigger_config_json, target_kind, recipe_id
FROM schedules;

DROP TABLE hitl_requests;
DROP TABLE recipe_runs;
DROP TABLE recipes_index;
DROP TABLE schedules;

ALTER TABLE recipes_index_new RENAME TO recipes_index;
ALTER TABLE recipe_runs_new RENAME TO recipe_runs;
ALTER TABLE hitl_requests_new RENAME TO hitl_requests;
ALTER TABLE schedules_new RENAME TO schedules;

CREATE INDEX IF NOT EXISTS idx_recipes_workspace ON recipes_index(workspace_id, is_enabled);
CREATE INDEX IF NOT EXISTS idx_recipes_trigger ON recipes_index(trigger_type);
CREATE INDEX IF NOT EXISTS idx_recipe_runs_recipe ON recipe_runs(workspace_id, recipe_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_recipe_runs_workspace ON recipe_runs(workspace_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_recipe_runs_status ON recipe_runs(status);
CREATE INDEX IF NOT EXISTS idx_hitl_status ON hitl_requests(status, expires_at);
CREATE INDEX IF NOT EXISTS idx_hitl_run ON hitl_requests(recipe_run_id);
CREATE INDEX IF NOT EXISTS idx_hitl_workspace ON hitl_requests(workspace_id, status);
CREATE INDEX IF NOT EXISTS idx_schedules_active_next ON schedules(is_active, next_run_at);
CREATE INDEX IF NOT EXISTS idx_schedules_due ON schedules(is_active, next_run_at);
CREATE INDEX IF NOT EXISTS idx_schedules_trigger ON schedules(workspace_id, trigger_type, is_active);

PRAGMA foreign_keys = ON;
