PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- 1. Workspaces (companies / areas / projects)
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    description TEXT,
    color_hex TEXT NOT NULL DEFAULT '#3b82f6',
    default_router_policy TEXT NOT NULL DEFAULT 'COST_BENEFIT'
        CHECK (default_router_policy IN ('COST_BENEFIT', 'CHEAPEST', 'BEST_QUALITY', 'MANUAL', 'CUSTOM_RULES')),
    default_email_recipient TEXT,
    google_drive_folder_id TEXT,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_opened_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 2. Recurring schedules (created before execution_queue because it references this table)
CREATE TABLE IF NOT EXISTS schedules (
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
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 3. Kanban tasks
CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    column_stage TEXT NOT NULL DEFAULT 'BACKLOG'
        CHECK (column_stage IN ('BACKLOG', 'PLAN', 'BUILD', 'REVIEW', 'DONE')),
    phase_status TEXT,
    priority INTEGER NOT NULL DEFAULT 2,
    execution_mode TEXT NOT NULL DEFAULT 'AUTO_DSH'
        CHECK (execution_mode IN ('AUTO_DSH', 'PLAN_ONLY', 'BUILD_DIRECT', 'PLAN_THEN_BUILD')),
    assigned_agent TEXT,
    preferred_model TEXT,
    effort_level TEXT NOT NULL DEFAULT 'medium'
        CHECK (effort_level IN ('low', 'medium', 'high')),
    plan_markdown TEXT,
    output_dir_path TEXT,
    notify_email TEXT,
    export_to_gdocs INTEGER NOT NULL DEFAULT 0,
    tags_json TEXT NOT NULL DEFAULT '[]',
    position_index INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 4. Execution queue
CREATE TABLE IF NOT EXISTS execution_queue (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    schedule_id TEXT REFERENCES schedules(id) ON DELETE SET NULL,
    prompt_payload TEXT NOT NULL,
    resolved_mode TEXT NOT NULL,
    resolved_agent TEXT NOT NULL,
    resolved_model TEXT NOT NULL,
    resolved_effort TEXT NOT NULL,
    job_kind TEXT NOT NULL DEFAULT 'task',
    scheduled_for TEXT,
    priority INTEGER NOT NULL DEFAULT 2,
    status TEXT NOT NULL DEFAULT 'QUEUED'
        CHECK (status IN ('QUEUED', 'RUNNING', 'PAUSED', 'COMPLETED', 'FAILED', 'CANCELLED')),
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 2,
    error_message TEXT,
    enqueued_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    finished_at TEXT
);

-- 5. Run history and financial telemetry
CREATE TABLE IF NOT EXISTS task_runs (
    id TEXT PRIMARY KEY,
    queue_job_id TEXT REFERENCES execution_queue(id) ON DELETE SET NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    parent_run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    phase TEXT,
    agent_name TEXT NOT NULL,
    execution_mode TEXT NOT NULL,
    provider_id TEXT,
    model_id TEXT NOT NULL,
    effort_level TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('RUNNING', 'SUCCESS', 'FAILED', 'CANCELLED')),
    tokens_input INTEGER NOT NULL DEFAULT 0,
    tokens_output INTEGER NOT NULL DEFAULT 0,
    tokens_reasoning INTEGER NOT NULL DEFAULT 0,
    tokens_cache_read INTEGER NOT NULL DEFAULT 0,
    tokens_cache_write INTEGER NOT NULL DEFAULT 0,
    estimated_cost_usd REAL NOT NULL DEFAULT 0.0,
    pricing_snapshot_json TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    log_path TEXT,
    raw_stdout_log TEXT,
    summary_response TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT
);

-- 6. Artifact provenance (separate from the file explorer)
CREATE TABLE IF NOT EXISTS artifacts_provenance (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    run_id TEXT NOT NULL REFERENCES task_runs(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    absolute_path TEXT NOT NULL,
    file_type TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL DEFAULT 0,
    content_sha256 TEXT,
    previous_sha256 TEXT,
    action_type TEXT NOT NULL CHECK (action_type IN ('CREATED', 'MODIFIED', 'DELETED')),
    created_by_agent TEXT NOT NULL,
    creation_reason TEXT NOT NULL,
    diff_patch TEXT,
    google_doc_url TEXT,
    google_drive_url TEXT,
    emailed_to TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 7. DSH router rules and budget limits
CREATE TABLE IF NOT EXISTS router_rules (
    id TEXT PRIMARY KEY,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    priority_order INTEGER NOT NULL,
    condition_json TEXT NOT NULL,
    target_mode TEXT NOT NULL CHECK (target_mode IN ('plan', 'build', 'plan_then_build')),
    target_agent TEXT,
    target_model TEXT NOT NULL,
    target_effort TEXT NOT NULL CHECK (target_effort IN ('low', 'medium', 'high')),
    is_enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS budget_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    daily_limit_usd REAL NOT NULL DEFAULT 5.0,
    monthly_limit_usd REAL NOT NULL DEFAULT 50.0,
    soft_alert_threshold_pct REAL NOT NULL DEFAULT 80.0,
    hard_stop_enabled INTEGER NOT NULL DEFAULT 1,
    opencode_go_5h_token_limit INTEGER NOT NULL DEFAULT 5000000,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 8. SMTP and Google Workspace settings per workspace
CREATE TABLE IF NOT EXISTS workspace_integrations (
    workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    smtp_host TEXT,
    smtp_port INTEGER NOT NULL DEFAULT 587,
    smtp_username TEXT,
    smtp_from_email TEXT,
    smtp_use_tls INTEGER NOT NULL DEFAULT 1,
    smtp_secret_ref TEXT,
    google_account_email TEXT,
    google_calendar_id TEXT NOT NULL DEFAULT 'primary',
    google_scopes TEXT,
    google_keep_sync_enabled INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 9. Operational settings (not OpenCode configuration; that remains on disk)
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 10. Post-run delivery tracking (email, Drive, Docs, Calendar)
CREATE TABLE IF NOT EXISTS integration_deliveries (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    artifact_id TEXT REFERENCES artifacts_provenance(id) ON DELETE SET NULL,
    channel TEXT NOT NULL CHECK (channel IN ('SMTP', 'GMAIL', 'DRIVE', 'GDOCS', 'GCALENDAR', 'GTASKS')),
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'RUNNING', 'SUCCESS', 'FAILED', 'SKIPPED')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    external_id TEXT,
    external_url TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 11. Audited disk changes made by Meta-Builder / Studio
CREATE TABLE IF NOT EXISTS config_change_log (
    id TEXT PRIMARY KEY,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
    entity TEXT NOT NULL,
    path TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('CREATED', 'UPDATED', 'DELETED')),
    diff_patch TEXT,
    actor TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- 12. Permission decisions for unattended runs
CREATE TABLE IF NOT EXISTS permission_audit (
    id TEXT PRIMARY KEY,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    session_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    permission_action TEXT NOT NULL,
    resource TEXT,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny', 'ask')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Indexes for the queries used by the dashboard, queue and provenance explorer.
CREATE INDEX IF NOT EXISTS idx_tasks_workspace_stage_position
    ON tasks (workspace_id, column_stage, position_index);
CREATE INDEX IF NOT EXISTS idx_tasks_workspace_updated
    ON tasks (workspace_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_queue_status_priority
    ON execution_queue (status, priority DESC, enqueued_at ASC);
CREATE INDEX IF NOT EXISTS idx_queue_workspace_status
    ON execution_queue (workspace_id, status);
CREATE INDEX IF NOT EXISTS idx_task_runs_workspace_started
    ON task_runs (workspace_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_task_runs_task
    ON task_runs (task_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_artifacts_run
    ON artifacts_provenance (run_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_workspace_created
    ON artifacts_provenance (workspace_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_schedules_active_next
    ON schedules (is_active, next_run_at);
CREATE INDEX IF NOT EXISTS idx_router_rules_workspace_priority
    ON router_rules (workspace_id, priority_order ASC);
CREATE INDEX IF NOT EXISTS idx_deliveries_status
    ON integration_deliveries (status, channel);

-- Single-row budget settings.
INSERT OR IGNORE INTO budget_settings (id) VALUES (1);
