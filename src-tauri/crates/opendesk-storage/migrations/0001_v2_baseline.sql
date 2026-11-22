-- OpenDesk V2 baseline: the only schema a V2 database needs.
-- PRAGMAs are applied per connection in `pool.rs` (WAL, synchronous=NORMAL,
-- busy_timeout=5000, cache_size=-64000, foreign_keys=ON). They cannot take
-- effect here because sqlx runs each migration inside a transaction.

CREATE TABLE adapter_registry (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    command TEXT,
    capabilities_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'UNKNOWN'
        CHECK (status IN ('UNKNOWN', 'READY', 'MISSING', 'ERROR')),
    last_health_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
, args_json TEXT NOT NULL DEFAULT '[]', health_args_json TEXT NOT NULL DEFAULT '["--version"]') STRICT;
CREATE TABLE agent_delegations (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    from_agent TEXT NOT NULL,
    to_agent TEXT NOT NULL,
    title TEXT NOT NULL,
    prompt TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'APPROVED', 'REJECTED', 'FAILED')),
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    requested_by TEXT,
    requested_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    resolved_at TEXT,
    resolved_by TEXT,
    error TEXT
) STRICT;
CREATE TABLE agent_goals (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    metric TEXT,
    target_value REAL,
    current_value REAL NOT NULL DEFAULT 0,
    unit TEXT,
    due_at TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('ACTIVE', 'DONE', 'PAUSED', 'CANCELLED')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE agent_profiles (
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL,
    role TEXT,
    team TEXT,
    manager_id TEXT,
    monthly_budget_usd REAL,
    concurrency_quota INTEGER NOT NULL DEFAULT 5,
    heartbeat_enabled INTEGER NOT NULL DEFAULT 0,
    heartbeat_minutes INTEGER NOT NULL DEFAULT 60,
    last_heartbeat_at TEXT,
    next_heartbeat_at TEXT,
    notes TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (workspace_id, agent_id)
) STRICT;
CREATE TABLE alert_dismissals (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    ref_id TEXT NOT NULL,
    dismissed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
, version TEXT NOT NULL DEFAULT '') STRICT;
CREATE TABLE api_audit (
    id TEXT PRIMARY KEY,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    status INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    actor TEXT NOT NULL DEFAULT 'local',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE artifacts_provenance (
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
, pinned INTEGER NOT NULL DEFAULT 0) STRICT;
CREATE TABLE auth_sessions (
    id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT 'remote',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT,
    expires_at TEXT NOT NULL,
    revoked_at TEXT
) STRICT;
CREATE TABLE budget_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    daily_limit_usd REAL NOT NULL DEFAULT 5.0,
    monthly_limit_usd REAL NOT NULL DEFAULT 50.0,
    soft_alert_threshold_pct REAL NOT NULL DEFAULT 80.0,
    hard_stop_enabled INTEGER NOT NULL DEFAULT 1,
    opencode_go_5h_token_limit INTEGER NOT NULL DEFAULT 5000000,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE calendar_events (
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
) STRICT;
CREATE TABLE calendar_sources (
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
) STRICT;
CREATE TABLE calendar_trigger_fires (
    recipe_id TEXT NOT NULL,
    event_id TEXT NOT NULL,
    occurrence_start TEXT NOT NULL,
    fired_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (recipe_id, event_id, occurrence_start)
) STRICT;
CREATE TABLE channel_routes (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    channel TEXT NOT NULL,
    identity TEXT NOT NULL,
    agent TEXT NOT NULL,
    permission_profile TEXT NOT NULL DEFAULT 'default',
    trust_level TEXT NOT NULL DEFAULT 'untrusted'
        CHECK (trust_level IN ('trusted', 'internal', 'untrusted')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE computer_sessions (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    run_id TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('PENDING', 'ACTIVE', 'ENDED')),
    require_approval INTEGER NOT NULL DEFAULT 0,
    actions INTEGER NOT NULL DEFAULT 0,
    last_action_json TEXT,
    screenshot_path TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ended_at TEXT
, display TEXT, isolated INTEGER NOT NULL DEFAULT 0) STRICT;
CREATE TABLE config_proposals (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK(kind IN (
        'AGENT', 'SKILL', 'COMMAND', 'RECIPE', 'ROUTINE', 'MCP', 'CONFIG'
    )),
    action TEXT NOT NULL CHECK(action IN ('CREATE', 'UPDATE', 'DELETE')),
    title TEXT NOT NULL,
    description TEXT,
    payload_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK(status IN ('PENDING', 'APPROVED', 'REJECTED', 'APPLIED', 'FAILED')),
    requested_by TEXT,
    source_run_id TEXT,
    source_task_id TEXT,
    apply_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    resolved_at TEXT,
    resolved_by TEXT
) STRICT;
CREATE TABLE execution_queue (
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
, output_dir TEXT, session_id TEXT, lease_expires_at TEXT, last_heartbeat_at TEXT, concurrency_group TEXT NOT NULL DEFAULT 'default', dedupe_key TEXT, resolved_harness TEXT NOT NULL DEFAULT 'opencode', assigned_node_id TEXT) STRICT;
CREATE TABLE handoffs (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    parent_handoff_id TEXT REFERENCES handoffs(id) ON DELETE SET NULL,
    source_agent TEXT NOT NULL,
    target_agent TEXT NOT NULL,
    objective TEXT NOT NULL,
    input_artifacts_json TEXT NOT NULL DEFAULT '[]',
    output_schema_json TEXT NOT NULL DEFAULT '{}',
    acceptance_json TEXT NOT NULL DEFAULT '[]',
    sub_budget_usd REAL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'ACCEPTED', 'REJECTED', 'RUNNING', 'DONE', 'FAILED', 'CANCELLED')),
    result_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE "hitl_requests" (
    token TEXT PRIMARY KEY,
    recipe_run_id TEXT NOT NULL REFERENCES "recipe_runs"(id) ON DELETE CASCADE,
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
, title TEXT, question_markdown TEXT, context_json TEXT NOT NULL DEFAULT '{}') STRICT;
CREATE TABLE "integration_deliveries" (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    artifact_id TEXT REFERENCES artifacts_provenance(id) ON DELETE SET NULL,
    channel TEXT NOT NULL CHECK (channel IN ('SMTP', 'GMAIL', 'DRIVE', 'GDOCS', 'GCALENDAR', 'GTASKS', 'SIGNAL', 'TELEGRAM')),
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'RUNNING', 'SUCCESS', 'FAILED', 'SKIPPED')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    external_id TEXT,
    external_url TEXT,
    next_retry_at TEXT,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    payload_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE io_inbound_sources (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL CHECK(source_type IN (
        'FOLDER_WATCH', 'OCR_WATCH', 'AUDIO_STT_WATCH',
        'IMAP_IDLE', 'WEBHOOK_HTTP', 'SQL_POLLER',
        'TELEGRAM_BOT', 'MATRIX_BOT', 'MATTERMOST_BOT'
    )),
    config_json TEXT NOT NULL,
    target_agent TEXT,
    target_column TEXT NOT NULL DEFAULT 'TODO'
        CHECK(target_column IN ('BACKLOG','TODO','IN_PROGRESS','IN_REVIEW','DONE','CANCELLED','BLOCKED')),
    auto_execute INTEGER NOT NULL DEFAULT 1,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_triggered_at TEXT,
    last_status TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE io_outbound_sinks (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sink_type TEXT NOT NULL CHECK(sink_type IN (
        'ATOMIC_FS', 'MINIO_S3', 'SMTP_EMAIL',
        'TELEGRAM_MSG', 'MATRIX_MSG', 'MATTERMOST_MSG',
        'DOC_COMPILER', 'GIT_COMMIT', 'SQL_INSERT'
    )),
    config_json TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_run_at TEXT,
    last_status TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE learning_lessons (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    task_id TEXT,
    agent_name TEXT,
    model_id TEXT,
    category TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'info',
    title TEXT NOT NULL,
    detail TEXT NOT NULL,
    evidence_json TEXT NOT NULL DEFAULT '{}',
    fingerprint TEXT NOT NULL,
    occurrences INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'OPEN'
        CHECK (status IN ('OPEN', 'DISMISSED', 'PROMOTED')),
    proposal_id TEXT,
    remembered_at TEXT,
    last_seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE learning_run_captures (
    run_id TEXT PRIMARY KEY REFERENCES task_runs(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    lesson_id TEXT
) STRICT;
CREATE TABLE memory_acl (
    namespace_id TEXT NOT NULL REFERENCES memory_namespaces(id) ON DELETE CASCADE,
    agent TEXT NOT NULL,
    can_read INTEGER NOT NULL DEFAULT 1,
    can_write INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (namespace_id, agent)
) STRICT;
CREATE TABLE memory_namespaces (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE memory_notes (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    namespace TEXT NOT NULL DEFAULT 'core',
    kind TEXT NOT NULL DEFAULT 'semantic',
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    tags_json TEXT NOT NULL DEFAULT '[]',
    confidence REAL NOT NULL DEFAULT 0.5,
    trust_level TEXT NOT NULL DEFAULT 'observed'
        CHECK (trust_level IN ('observed', 'verified', 'inferred')),
    expires_at TEXT,
    source TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
, access_count INTEGER NOT NULL DEFAULT 0, last_accessed_at TEXT, archived_at TEXT) STRICT;
CREATE TABLE missed_runs (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    schedule_id TEXT NOT NULL REFERENCES schedules(id) ON DELETE CASCADE,
    scheduled_for TEXT NOT NULL,
    detected_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK(status IN ('PENDING','QUEUED','IGNORED','FAILED','CANCELLED')),
    run_id TEXT,
    error_message TEXT,
    UNIQUE(schedule_id, scheduled_for)
) STRICT;
CREATE TABLE model_health (
    provider_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    code TEXT NOT NULL,
    failures INTEGER NOT NULL DEFAULT 1,
    failed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), cooldown_until TEXT,
    PRIMARY KEY (provider_id, model_id)
) STRICT;
CREATE TABLE passkey_ceremonies (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('register', 'login')),
    state_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    expires_at TEXT NOT NULL
) STRICT;
CREATE TABLE passkey_credentials (
    id TEXT PRIMARY KEY,
    credential_id TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL DEFAULT 'passkey',
    passkey_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT
) STRICT;
CREATE TABLE permission_audit (
    id TEXT PRIMARY KEY,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    session_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    permission_action TEXT NOT NULL,
    resource TEXT,
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny', 'ask')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE permission_requests (
    id TEXT PRIMARY KEY,
    run_id TEXT REFERENCES task_runs(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    request_id TEXT NOT NULL UNIQUE,
    permission_action TEXT NOT NULL,
    resources TEXT,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'DECIDED', 'EXPIRED')),
    decision TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    decided_at TEXT
) STRICT;
CREATE TABLE personal_context_groups (
    id TEXT PRIMARY KEY,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK(scope IN ('workspace', 'global')),
    name TEXT NOT NULL,
    instructions TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
, source TEXT NOT NULL DEFAULT 'manual', source_quote TEXT) STRICT;
CREATE TABLE push_subscriptions (
    id TEXT PRIMARY KEY,
    endpoint TEXT NOT NULL,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT 'device',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_success_at TEXT,
    last_failure_at TEXT,
    failure_count INTEGER NOT NULL DEFAULT 0
) STRICT;
CREATE TABLE recipe_file_state (
    recipe_id TEXT NOT NULL,
    trigger_path TEXT NOT NULL,
    file_path TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'SEEN'
        CHECK(status IN ('SEEN', 'RUNNING', 'COMPLETED', 'FAILED')),
    run_id TEXT,
    processed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (recipe_id, trigger_path, file_path)
) STRICT;
CREATE TABLE recipe_run_nodes (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES recipe_runs(id) ON DELETE CASCADE,
    node_index INTEGER NOT NULL,
    node_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'RUNNING', 'WAITING_HITL', 'DONE', 'FAILED', 'SKIPPED')),
    attempts INTEGER NOT NULL DEFAULT 0,
    session_id TEXT,
    output_json TEXT,
    error_message TEXT,
    started_at TEXT,
    finished_at TEXT, node_path TEXT, parent_path TEXT, item_key TEXT, join_policy TEXT,
    UNIQUE (run_id, node_index)
) STRICT;
CREATE TABLE "recipe_runs" (
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
    completed_at TEXT, cancel_requested INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (workspace_id, recipe_id)
        REFERENCES "recipes_index"(workspace_id, id) ON DELETE CASCADE
) STRICT;
CREATE TABLE recipe_versions (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    recipe_id TEXT NOT NULL,
    content TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE "recipes_index" (
    id TEXT NOT NULL,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    content_sha256 TEXT,
    trigger_type TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_error TEXT,
    last_run_at TEXT,
    total_runs INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    PRIMARY KEY (workspace_id, id)
) STRICT;
CREATE TABLE router_rules (
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
) STRICT;
CREATE TABLE run_ab_groups (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    prompt TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'OPEN'
        CHECK (status IN ('OPEN', 'DECIDED', 'CANCELLED')),
    winner_slot TEXT CHECK (winner_slot IN ('a', 'b')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    decided_at TEXT
) STRICT;
CREATE TABLE run_worktrees (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    run_id TEXT,
    path TEXT NOT NULL,
    mode TEXT NOT NULL CHECK (mode IN ('git', 'overlay')),
    branch TEXT,
    base_commit TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('ACTIVE', 'MERGED', 'REMOVED', 'CONFLICT')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    merged_at TEXT
) STRICT;
CREATE TABLE "schedules" (
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
, notify_telegram INTEGER NOT NULL DEFAULT 0, notify_telegram_recipients TEXT) STRICT;
CREATE TABLE session_context_packets (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    packet_json TEXT NOT NULL,
    generated_at TEXT NOT NULL
) STRICT;
CREATE TABLE skill_eval_cases (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL,
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    expected_contains TEXT,
    expected_schema_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE skill_eval_gates (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('PASS', 'BLOCKED', 'INSUFFICIENT')),
    mentions INTEGER NOT NULL DEFAULT 0,
    successes INTEGER NOT NULL DEFAULT 0,
    failures INTEGER NOT NULL DEFAULT 0,
    success_rate REAL,
    min_samples INTEGER NOT NULL,
    min_success_rate REAL NOT NULL,
    days INTEGER NOT NULL,
    details_json TEXT NOT NULL DEFAULT '{}',
    decision TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (decision IN ('PENDING', 'APPROVED', 'REJECTED')),
    decision_note TEXT,
    decided_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE skill_shadow_results (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES skill_shadow_runs(id) ON DELETE CASCADE,
    case_id TEXT,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('PASS', 'FAILED', 'ERROR')),
    output TEXT,
    contains_ok INTEGER NOT NULL DEFAULT 0,
    schema_ok INTEGER NOT NULL DEFAULT 0,
    tokens INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE skill_shadow_runs (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('PASS', 'BLOCKED', 'ERROR')),
    total INTEGER NOT NULL DEFAULT 0,
    passed INTEGER NOT NULL DEFAULT 0,
    failed INTEGER NOT NULL DEFAULT 0,
    success_rate REAL,
    tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd REAL NOT NULL DEFAULT 0.0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    baseline_run_id TEXT,
    regression REAL,
    min_success_rate REAL NOT NULL,
    max_regression REAL NOT NULL,
    details_json TEXT NOT NULL DEFAULT '{}',
    promoted_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE starter_pack_installs (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    pack_id TEXT NOT NULL,
    pack_version TEXT NOT NULL,
    files_json TEXT NOT NULL,
    routines_json TEXT NOT NULL DEFAULT '[]',
    installed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
) STRICT;
CREATE TABLE task_runs (
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
, error_code TEXT, error_message TEXT, error_hint TEXT, schedule_id TEXT, attempt_number INTEGER NOT NULL DEFAULT 1, session_id TEXT, source TEXT, channel TEXT, origin_ref TEXT) STRICT;
CREATE TABLE task_sink_bindings (
    id TEXT PRIMARY KEY,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    schedule_id TEXT REFERENCES schedules(id) ON DELETE CASCADE,
    inbound_source_id TEXT REFERENCES io_inbound_sources(id) ON DELETE CASCADE,
    sink_id TEXT NOT NULL REFERENCES io_outbound_sinks(id) ON DELETE CASCADE,
    execution_order INTEGER NOT NULL DEFAULT 1,
    CHECK ((task_id IS NOT NULL) != (schedule_id IS NOT NULL))
) STRICT;
CREATE TABLE task_watchers (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    channel TEXT NOT NULL CHECK(channel IN ('telegram', 'signal')),
    recipient TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE tasks (
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
, agent_override INTEGER NOT NULL DEFAULT 0, model_override INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'BACKLOG', phase TEXT, last_run_id TEXT, last_run_status TEXT, last_run_at TEXT, blocked_reason TEXT, cancelled_at TEXT, forked_from_task_id TEXT, origin TEXT NOT NULL DEFAULT 'MANUAL', parent_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL, dedupe_key TEXT, held INTEGER NOT NULL DEFAULT 0, care_level TEXT NOT NULL DEFAULT 'standard', preferred_harness TEXT) STRICT;
CREATE TABLE tickets (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    source TEXT NOT NULL,
    source_ref TEXT,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 2,
    status TEXT NOT NULL DEFAULT 'OPEN'
        CHECK (status IN ('OPEN', 'TRIAGED', 'IN_PROGRESS', 'RESOLVED', 'DISMISSED')),
    assigned_agent TEXT,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE workers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    capabilities_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'OFFLINE'
        CHECK (status IN ('ONLINE', 'BUSY', 'OFFLINE', 'DRAINING')),
    token_hash TEXT,
    last_seen_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
CREATE TABLE workspace_integrations (
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
) STRICT;
CREATE TABLE workspaces (
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
) STRICT;
CREATE INDEX idx_agent_delegations_status
    ON agent_delegations(workspace_id, status, requested_at);
CREATE INDEX idx_agent_goals_agent
    ON agent_goals(workspace_id, agent_id, status);
CREATE UNIQUE INDEX idx_alert_dismissals_unique
    ON alert_dismissals(workspace_id, kind, ref_id);
CREATE INDEX idx_api_audit_action
    ON api_audit(action, created_at DESC);
CREATE INDEX idx_api_audit_created
    ON api_audit(created_at DESC);
CREATE INDEX idx_api_audit_workspace
    ON api_audit(workspace_id, created_at DESC);
CREATE INDEX idx_artifacts_run
    ON artifacts_provenance (run_id);
CREATE INDEX idx_artifacts_workspace_created
    ON artifacts_provenance (workspace_id, created_at DESC);
CREATE INDEX idx_auth_sessions_expiry
    ON auth_sessions(expires_at);
CREATE UNIQUE INDEX idx_auth_sessions_hash
    ON auth_sessions(token_hash);
CREATE UNIQUE INDEX idx_calendar_events_external
    ON calendar_events(source_id, external_id)
    WHERE external_id IS NOT NULL;
CREATE INDEX idx_calendar_events_range
    ON calendar_events(workspace_id, start_at, end_at);
CREATE INDEX idx_calendar_events_source
    ON calendar_events(source_id, start_at);
CREATE UNIQUE INDEX idx_calendar_sources_external
    ON calendar_sources(workspace_id, provider, external_id)
    WHERE external_id IS NOT NULL;
CREATE UNIQUE INDEX idx_channel_routes_identity
    ON channel_routes(channel, identity);
CREATE INDEX idx_computer_sessions_workspace
    ON computer_sessions(workspace_id, created_at);
CREATE INDEX idx_config_proposals_status
    ON config_proposals(status, created_at);
CREATE INDEX idx_config_proposals_workspace
    ON config_proposals(workspace_id, status, created_at);
CREATE INDEX idx_execution_queue_node
    ON execution_queue(assigned_node_id, status);
CREATE UNIQUE INDEX idx_queue_active_dedupe
    ON execution_queue(workspace_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL AND status IN ('QUEUED', 'RUNNING');
CREATE INDEX idx_queue_dedupe
    ON execution_queue(workspace_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL;
CREATE UNIQUE INDEX idx_queue_schedule_slot
    ON execution_queue (schedule_id, scheduled_for)
    WHERE schedule_id IS NOT NULL AND scheduled_for IS NOT NULL;
CREATE INDEX idx_queue_status_priority
    ON execution_queue (status, priority DESC, enqueued_at ASC);
CREATE INDEX idx_queue_workspace_status
    ON execution_queue (workspace_id, status);
CREATE INDEX idx_handoffs_workspace
    ON handoffs(workspace_id, status, created_at);
CREATE INDEX idx_hitl_run ON hitl_requests(recipe_run_id);
CREATE INDEX idx_hitl_status ON hitl_requests(status, expires_at);
CREATE INDEX idx_hitl_workspace ON hitl_requests(workspace_id, status);
CREATE INDEX idx_deliveries_status
    ON integration_deliveries (status, channel);
CREATE INDEX idx_io_inbound_enabled
    ON io_inbound_sources(workspace_id, source_type, is_enabled);
CREATE UNIQUE INDEX idx_io_inbound_workspace_name
    ON io_inbound_sources(workspace_id, name);
CREATE INDEX idx_io_outbound_enabled
    ON io_outbound_sinks(workspace_id, sink_type, is_enabled);
CREATE UNIQUE INDEX idx_io_outbound_workspace_name
    ON io_outbound_sinks(workspace_id, name);
CREATE UNIQUE INDEX idx_learning_lessons_fingerprint
    ON learning_lessons(workspace_id, fingerprint);
CREATE INDEX idx_learning_lessons_status
    ON learning_lessons(workspace_id, status, last_seen_at DESC);
CREATE UNIQUE INDEX idx_memory_namespaces_name
    ON memory_namespaces(workspace_id, name);
CREATE INDEX idx_memory_notes_archived
    ON memory_notes(workspace_id, archived_at);
CREATE INDEX idx_memory_notes_workspace
    ON memory_notes(workspace_id, namespace, updated_at);
CREATE INDEX idx_missed_runs_workspace
    ON missed_runs(workspace_id, status, scheduled_for);
CREATE INDEX idx_passkey_ceremonies_expiry
    ON passkey_ceremonies(expires_at);
CREATE INDEX idx_permission_requests_pending
    ON permission_requests(status, workspace_id, created_at);
CREATE INDEX idx_personal_context_scope
    ON personal_context_groups(scope, workspace_id, position);
CREATE UNIQUE INDEX idx_push_subscriptions_endpoint
    ON push_subscriptions(endpoint);
CREATE INDEX idx_recipe_file_state_recipe
    ON recipe_file_state(recipe_id, trigger_path, status);
CREATE UNIQUE INDEX idx_recipe_run_nodes_path
    ON recipe_run_nodes(run_id, node_path);
CREATE INDEX idx_recipe_run_nodes_run
    ON recipe_run_nodes(run_id, node_index);
CREATE INDEX idx_recipe_run_nodes_status
    ON recipe_run_nodes(status);
CREATE INDEX idx_recipe_runs_recipe ON recipe_runs(workspace_id, recipe_id, started_at DESC);
CREATE INDEX idx_recipe_runs_status ON recipe_runs(status);
CREATE INDEX idx_recipe_runs_workspace ON recipe_runs(workspace_id, started_at DESC);
CREATE INDEX idx_recipe_versions_recipe
    ON recipe_versions(workspace_id, recipe_id, created_at DESC);
CREATE INDEX idx_recipes_trigger ON recipes_index(trigger_type);
CREATE INDEX idx_recipes_workspace ON recipes_index(workspace_id, is_enabled);
CREATE INDEX idx_router_rules_workspace_priority
    ON router_rules (workspace_id, priority_order ASC);
CREATE INDEX idx_run_ab_groups_workspace
    ON run_ab_groups(workspace_id, created_at DESC);
CREATE INDEX idx_run_worktrees_workspace
    ON run_worktrees(workspace_id, created_at DESC);
CREATE INDEX idx_schedules_active_next ON schedules(is_active, next_run_at);
CREATE INDEX idx_schedules_due ON schedules(is_active, next_run_at);
CREATE INDEX idx_schedules_trigger ON schedules(workspace_id, trigger_type, is_active);
CREATE INDEX idx_session_context_workspace
    ON session_context_packets(workspace_id, generated_at DESC);
CREATE INDEX idx_skill_eval_cases_skill
    ON skill_eval_cases(workspace_id, skill_id, created_at);
CREATE INDEX idx_skill_eval_gates_skill
    ON skill_eval_gates(workspace_id, skill_id, created_at DESC);
CREATE INDEX idx_skill_shadow_results_run
    ON skill_shadow_results(run_id);
CREATE INDEX idx_skill_shadow_runs_skill
    ON skill_shadow_runs(workspace_id, skill_id, created_at DESC);
CREATE UNIQUE INDEX idx_starter_pack_install_unique
    ON starter_pack_installs(workspace_id, pack_id);
CREATE INDEX idx_task_runs_schedule
    ON task_runs (schedule_id, started_at DESC);
CREATE INDEX idx_task_runs_source
    ON task_runs(source, started_at);
CREATE INDEX idx_task_runs_task
    ON task_runs (task_id, started_at DESC);
CREATE INDEX idx_task_runs_workspace_started
    ON task_runs (workspace_id, started_at DESC);
CREATE INDEX idx_task_sink_inbound
    ON task_sink_bindings(inbound_source_id, execution_order);
CREATE INDEX idx_task_sink_schedule
    ON task_sink_bindings(schedule_id, execution_order);
CREATE INDEX idx_task_sink_task
    ON task_sink_bindings(task_id, execution_order);
CREATE UNIQUE INDEX idx_task_sink_unique
    ON task_sink_bindings(
        sink_id,
        IFNULL(task_id, ''),
        IFNULL(schedule_id, ''),
        IFNULL(inbound_source_id, '')
    );
CREATE INDEX idx_task_watchers_task
    ON task_watchers(task_id);
CREATE UNIQUE INDEX idx_task_watchers_unique
    ON task_watchers(task_id, channel, IFNULL(recipient, ''));
CREATE UNIQUE INDEX idx_tasks_active_dedupe
    ON tasks(workspace_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL
      AND status IN ('TODO', 'IN_PROGRESS', 'IN_REVIEW');
CREATE INDEX idx_tasks_dedupe
    ON tasks(workspace_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL;
CREATE INDEX idx_tasks_origin
    ON tasks(workspace_id, origin, status);
CREATE INDEX idx_tasks_status
    ON tasks (workspace_id, status, position_index);
CREATE INDEX idx_tasks_tags ON tasks(tags_json);
CREATE INDEX idx_tasks_workspace_stage_position
    ON tasks (workspace_id, column_stage, position_index);
CREATE INDEX idx_tasks_workspace_updated
    ON tasks (workspace_id, updated_at DESC);
CREATE UNIQUE INDEX idx_tickets_source_ref
    ON tickets(workspace_id, source, source_ref)
    WHERE source_ref IS NOT NULL;
CREATE INDEX idx_tickets_workspace
    ON tickets(workspace_id, status, priority, created_at);

-- Unified conversation stream: chat threads, task comments, HITL requests and
-- run turns share one time-ordered table pair.
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    channel TEXT NOT NULL DEFAULT 'chat',
    thread_key TEXT,
    title TEXT NOT NULL DEFAULT '',
    agent TEXT,
    model TEXT,
    opencode_session_id TEXT,
    source TEXT NOT NULL DEFAULT 'desktop',
    external_id TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE',
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    last_message_at TEXT
) STRICT;

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    run_id TEXT REFERENCES task_runs(id) ON DELETE SET NULL,
    role TEXT NOT NULL CHECK (role IN ('user','assistant','system')),
    kind TEXT NOT NULL DEFAULT 'chat',
    body TEXT NOT NULL DEFAULT '',
    payload_json TEXT,
    metadata_json TEXT,
    status TEXT NOT NULL DEFAULT 'COMPLETED',
    external_id TEXT,
    dedupe_key TEXT,
    expires_at TEXT,
    resolved_at TEXT,
    resolved_by TEXT,
    choice_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at TEXT
) STRICT;

CREATE UNIQUE INDEX idx_conversations_thread
    ON conversations(workspace_id, channel, thread_key)
    WHERE thread_key IS NOT NULL;
CREATE UNIQUE INDEX idx_conversations_external
    ON conversations(workspace_id, source, external_id)
    WHERE external_id IS NOT NULL;
CREATE INDEX idx_conversations_cursor
    ON conversations(workspace_id, updated_at DESC, id DESC);
CREATE INDEX idx_messages_cursor
    ON messages(conversation_id, created_at ASC, id ASC);
CREATE INDEX idx_messages_task
    ON messages(task_id, created_at ASC)
    WHERE task_id IS NOT NULL;
CREATE INDEX idx_messages_hitl_pending
    ON messages(kind, status)
    WHERE kind = 'hitl_request' AND status = 'PENDING';
CREATE UNIQUE INDEX idx_messages_dedupe
    ON messages(conversation_id, dedupe_key)
    WHERE dedupe_key IS NOT NULL;

-- V2 marker: a database without it is treated as a V1 database.
CREATE TABLE v2_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
INSERT INTO v2_meta (key, value) VALUES
    ('schema_version', '2'),
    ('baseline', '0001_v2_baseline');

-- Seed rows created by the V1 migrations: the budget guard expects row 1 and
-- the adapter registry ships the built-in harnesses.
INSERT OR IGNORE INTO budget_settings (id) VALUES (1);
INSERT OR IGNORE INTO adapter_registry
    (id, name, kind, command, capabilities_json, status, args_json, health_args_json)
VALUES
    ('opencode', 'OpenCode V2', 'opencode', NULL,
     '{"streaming":true,"tools":true,"permissions":true,"reasoning":true,"usage":true,"diffs":true,"sessions":true,"sandbox":false}',
     'UNKNOWN', '[]', '[]'),
    ('claude-code', 'Claude Code', 'cli', 'claude',
     '{"streaming":true,"tools":true,"permissions":true,"reasoning":true,"usage":true,"diffs":false,"sessions":false,"sandbox":false}',
     'UNKNOWN',
     '["-p","{prompt}","--output-format","stream-json","--verbose"]',
     '["--version"]'),
    ('codex', 'Codex CLI', 'cli', 'codex',
     '{"streaming":true,"tools":true,"permissions":false,"reasoning":true,"usage":true,"diffs":false,"sessions":false,"sandbox":false}',
     'UNKNOWN', '["exec","--json","{prompt}"]', '["--version"]'),
    ('pi', 'Pi', 'cli', 'pi',
     '{"streaming":true,"tools":true,"permissions":false,"reasoning":false,"usage":false,"diffs":false,"sessions":false,"sandbox":false}',
     'UNKNOWN', '["run","--json","{prompt}"]', '["--version"]'),
    ('hermes', 'Hermes', 'cli', 'hermes',
     '{"streaming":true,"tools":true,"permissions":false,"reasoning":false,"usage":false,"diffs":false,"sessions":false,"sandbox":false}',
     'UNKNOWN', '["run","--json","{prompt}"]', '["--version"]');
