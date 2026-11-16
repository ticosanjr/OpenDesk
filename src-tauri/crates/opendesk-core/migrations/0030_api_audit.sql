-- Sensitive local API audit trail. Bodies are intentionally never stored.
CREATE TABLE IF NOT EXISTS api_audit (
    id TEXT PRIMARY KEY,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    status INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    actor TEXT NOT NULL DEFAULT 'local',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_api_audit_created
    ON api_audit(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_api_audit_action
    ON api_audit(action, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_api_audit_workspace
    ON api_audit(workspace_id, created_at DESC);
