-- Universal I/O bus: inbound triggers, outbound sinks and their bindings.
-- Credentials are never stored here; config_json only contains references resolved
-- through SecretStore.

CREATE TABLE IF NOT EXISTS io_inbound_sources (
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
    auto_execute BOOLEAN NOT NULL DEFAULT 1,
    is_enabled BOOLEAN NOT NULL DEFAULT 1,
    last_triggered_at TEXT,
    last_status TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_io_inbound_workspace_name
    ON io_inbound_sources(workspace_id, name);
CREATE INDEX IF NOT EXISTS idx_io_inbound_enabled
    ON io_inbound_sources(workspace_id, source_type, is_enabled);

CREATE TABLE IF NOT EXISTS io_outbound_sinks (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sink_type TEXT NOT NULL CHECK(sink_type IN (
        'ATOMIC_FS', 'MINIO_S3', 'SMTP_EMAIL',
        'TELEGRAM_MSG', 'MATRIX_MSG', 'MATTERMOST_MSG',
        'DOC_COMPILER', 'GIT_COMMIT', 'SQL_INSERT'
    )),
    config_json TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT 1,
    last_run_at TEXT,
    last_status TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_io_outbound_workspace_name
    ON io_outbound_sinks(workspace_id, name);
CREATE INDEX IF NOT EXISTS idx_io_outbound_enabled
    ON io_outbound_sinks(workspace_id, sink_type, is_enabled);

CREATE TABLE IF NOT EXISTS task_sink_bindings (
    id TEXT PRIMARY KEY,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    schedule_id TEXT REFERENCES schedules(id) ON DELETE CASCADE,
    inbound_source_id TEXT REFERENCES io_inbound_sources(id) ON DELETE CASCADE,
    sink_id TEXT NOT NULL REFERENCES io_outbound_sinks(id) ON DELETE CASCADE,
    execution_order INTEGER NOT NULL DEFAULT 1,
    CHECK ((task_id IS NOT NULL) != (schedule_id IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_sink_unique
    ON task_sink_bindings(
        sink_id,
        IFNULL(task_id, ''),
        IFNULL(schedule_id, ''),
        IFNULL(inbound_source_id, '')
    );
CREATE INDEX IF NOT EXISTS idx_task_sink_task
    ON task_sink_bindings(task_id, execution_order);
CREATE INDEX IF NOT EXISTS idx_task_sink_schedule
    ON task_sink_bindings(schedule_id, execution_order);
CREATE INDEX IF NOT EXISTS idx_task_sink_inbound
    ON task_sink_bindings(inbound_source_id, execution_order);
