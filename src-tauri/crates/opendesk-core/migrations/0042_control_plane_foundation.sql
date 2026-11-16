-- Control-plane foundations for the v1.0 program. The tables are additive and
-- intentionally generic so adapters, channels, memory and company workflows
-- can be implemented without another schema rewrite.

CREATE TABLE IF NOT EXISTS handoffs (
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
);
CREATE INDEX IF NOT EXISTS idx_handoffs_workspace
    ON handoffs(workspace_id, status, created_at);

CREATE TABLE IF NOT EXISTS tickets (
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
);
CREATE INDEX IF NOT EXISTS idx_tickets_workspace
    ON tickets(workspace_id, status, priority, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tickets_source_ref
    ON tickets(workspace_id, source, source_ref)
    WHERE source_ref IS NOT NULL;

CREATE TABLE IF NOT EXISTS channel_routes (
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
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_routes_identity
    ON channel_routes(channel, identity);

CREATE TABLE IF NOT EXISTS memory_namespaces (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_namespaces_name
    ON memory_namespaces(workspace_id, name);

CREATE TABLE IF NOT EXISTS memory_acl (
    namespace_id TEXT NOT NULL REFERENCES memory_namespaces(id) ON DELETE CASCADE,
    agent TEXT NOT NULL,
    can_read INTEGER NOT NULL DEFAULT 1,
    can_write INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (namespace_id, agent)
);

CREATE TABLE IF NOT EXISTS memory_notes (
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
);
CREATE INDEX IF NOT EXISTS idx_memory_notes_workspace
    ON memory_notes(workspace_id, namespace, updated_at);

CREATE TABLE IF NOT EXISTS memory_entities (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    namespace TEXT NOT NULL DEFAULT 'entities',
    name TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'entity',
    facts_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_memory_entities_name
    ON memory_entities(workspace_id, namespace, name);

CREATE TABLE IF NOT EXISTS workers (
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
);

CREATE TABLE IF NOT EXISTS adapter_registry (
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
);

CREATE TABLE IF NOT EXISTS runtime_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_runtime_events_run_seq
    ON runtime_events(run_id, seq);
CREATE INDEX IF NOT EXISTS idx_runtime_events_created
    ON runtime_events(created_at);
