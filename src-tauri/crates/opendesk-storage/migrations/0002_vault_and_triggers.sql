-- Fechamento OpenDesk, F0: Vault, barramento I/O unificado e estado do poller.
--
-- `0001` declarava os tipos de inbound/outbound em CHECK constraints. SQLite não
-- altera CHECK, então as duas tabelas e a tabela de bindings que as referencia
-- são reconstruídas na ordem: filhas primeiro, depois as antigas, e por fim o
-- rename das novas. Isso mantém as FKs válidas dentro da transação que o sqlx
-- usa para aplicar a migration.

-- ---------------------------------------------------------------------------
-- Credential Vault (Zero-Exposure)
-- ---------------------------------------------------------------------------

CREATE TABLE vault_entries (
    id TEXT PRIMARY KEY,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN (
        'api_key', 'login', 'bearer_token', 'totp_seed', 'ssh_key', 'header_bundle'
    )),
    label TEXT NOT NULL,
    username TEXT,
    allowed_domains_json TEXT NOT NULL DEFAULT '[]',
    secret_ciphertext BLOB NOT NULL,
    secret_nonce BLOB NOT NULL,
    requires_hitl INTEGER NOT NULL DEFAULT 1,
    expires_at TEXT,
    last_used_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE TABLE vault_acls (
    entry_id TEXT NOT NULL REFERENCES vault_entries(id) ON DELETE CASCADE,
    subject_kind TEXT NOT NULL CHECK (subject_kind IN ('agent', 'recipe', 'mcp', 'sink')),
    subject_id TEXT NOT NULL,
    can_inject INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (entry_id, subject_kind, subject_id)
) STRICT;

CREATE TABLE vault_audit (
    id TEXT PRIMARY KEY,
    entry_id TEXT NOT NULL,
    actor_kind TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    run_id TEXT,
    target_domain TEXT,
    outcome TEXT NOT NULL CHECK (outcome IN (
        'INJECTED', 'DENIED_ACL', 'DENIED_DOMAIN', 'DENIED_HITL', 'EXPIRED'
    )),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX idx_vault_entries_workspace
    ON vault_entries(workspace_id, label);
CREATE INDEX idx_vault_audit_entry
    ON vault_audit(entry_id, created_at DESC);

-- ---------------------------------------------------------------------------
-- I/O bus: novos tipos e destino explícito
-- ---------------------------------------------------------------------------

CREATE TABLE io_inbound_sources_new (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    source_type TEXT NOT NULL CHECK(source_type IN (
        'FOLDER_WATCH', 'OCR_WATCH', 'AUDIO_STT_WATCH',
        'IMAP_IDLE', 'WEBHOOK_HTTP', 'SQL_POLLER', 'HTTP_POLLER',
        'TELEGRAM_BOT', 'MATRIX_BOT', 'MATTERMOST_BOT'
    )),
    config_json TEXT NOT NULL,
    target_agent TEXT,
    target_column TEXT NOT NULL DEFAULT 'TODO'
        CHECK(target_column IN ('BACKLOG','TODO','IN_PROGRESS','IN_REVIEW','DONE','CANCELLED','BLOCKED')),
    target_kind TEXT NOT NULL DEFAULT 'TASK'
        CHECK(target_kind IN ('TASK', 'RECIPE', 'TICKET', 'SCHEDULE')),
    target_id TEXT,
    auto_execute INTEGER NOT NULL DEFAULT 1,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_triggered_at TEXT,
    last_status TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

INSERT INTO io_inbound_sources_new (
    id, workspace_id, name, source_type, config_json, target_agent, target_column,
    target_kind, target_id, auto_execute, is_enabled, last_triggered_at,
    last_status, last_error, created_at
)
SELECT
    id, workspace_id, name, source_type, config_json, target_agent, target_column,
    CASE
        WHEN json_valid(config_json)
             AND json_extract(config_json, '$.schedule_id') IS NOT NULL THEN 'SCHEDULE'
        WHEN json_valid(config_json)
             AND json_extract(config_json, '$.recipe_id') IS NOT NULL THEN 'RECIPE'
        ELSE 'TASK'
    END,
    CASE
        WHEN json_valid(config_json) THEN CAST(COALESCE(
            json_extract(config_json, '$.recipe_id'),
            json_extract(config_json, '$.schedule_id')
        ) AS TEXT)
        ELSE NULL
    END,
    auto_execute, is_enabled, last_triggered_at, last_status, last_error, created_at
FROM io_inbound_sources;

CREATE TABLE io_outbound_sinks_new (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sink_type TEXT NOT NULL CHECK(sink_type IN (
        'ATOMIC_FS', 'MINIO_S3', 'SMTP_EMAIL',
        'SIGNAL_MSG', 'TELEGRAM_MSG', 'MATRIX_MSG', 'MATTERMOST_MSG',
        'DOC_COMPILER', 'GIT_COMMIT', 'SQL_INSERT', 'RSYNC_FS',
        'HTTP_WEBHOOK', 'WHATSAPP_MSG', 'WEBPUSH'
    )),
    config_json TEXT NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    last_run_at TEXT,
    last_status TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

INSERT INTO io_outbound_sinks_new (
    id, workspace_id, name, sink_type, config_json, is_enabled,
    last_run_at, last_status, last_error, created_at
)
SELECT
    id, workspace_id, name, sink_type, config_json, is_enabled,
    last_run_at, last_status, last_error, created_at
FROM io_outbound_sinks;

CREATE TABLE task_sink_bindings_new (
    id TEXT PRIMARY KEY,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    schedule_id TEXT REFERENCES schedules(id) ON DELETE CASCADE,
    inbound_source_id TEXT REFERENCES io_inbound_sources_new(id) ON DELETE CASCADE,
    sink_id TEXT NOT NULL REFERENCES io_outbound_sinks_new(id) ON DELETE CASCADE,
    execution_order INTEGER NOT NULL DEFAULT 1,
    CHECK ((task_id IS NOT NULL) != (schedule_id IS NOT NULL))
) STRICT;

INSERT INTO task_sink_bindings_new (
    id, task_id, schedule_id, inbound_source_id, sink_id, execution_order
)
SELECT id, task_id, schedule_id, inbound_source_id, sink_id, execution_order
FROM task_sink_bindings;

DROP TABLE task_sink_bindings;
DROP TABLE io_inbound_sources;
DROP TABLE io_outbound_sinks;

ALTER TABLE io_inbound_sources_new RENAME TO io_inbound_sources;
ALTER TABLE io_outbound_sinks_new RENAME TO io_outbound_sinks;
ALTER TABLE task_sink_bindings_new RENAME TO task_sink_bindings;

CREATE INDEX idx_io_inbound_enabled
    ON io_inbound_sources(workspace_id, source_type, is_enabled);
CREATE UNIQUE INDEX idx_io_inbound_workspace_name
    ON io_inbound_sources(workspace_id, name);
CREATE INDEX idx_io_outbound_enabled
    ON io_outbound_sinks(workspace_id, sink_type, is_enabled);
CREATE UNIQUE INDEX idx_io_outbound_workspace_name
    ON io_outbound_sinks(workspace_id, name);
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

-- ---------------------------------------------------------------------------
-- Estado do HTTP/RSS poller (ETag, Last-Modified e deduplicação)
-- ---------------------------------------------------------------------------

CREATE TABLE io_poller_state (
    source_id TEXT PRIMARY KEY REFERENCES io_inbound_sources(id) ON DELETE CASCADE,
    etag TEXT,
    last_modified TEXT,
    last_polled_at TEXT,
    last_error TEXT
) STRICT;

CREATE TABLE io_poller_seen (
    source_id TEXT NOT NULL REFERENCES io_inbound_sources(id) ON DELETE CASCADE,
    item_key TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    seen_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (source_id, item_key)
) STRICT;

CREATE INDEX idx_io_poller_seen_hash
    ON io_poller_seen(source_id, content_hash);
