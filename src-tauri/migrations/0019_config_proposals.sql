-- Changes proposed by agents. Nothing is applied until an explicit human
-- approval, using the same HITL approval surface shown in the Attention Card.
CREATE TABLE IF NOT EXISTS config_proposals (
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
);

CREATE INDEX IF NOT EXISTS idx_config_proposals_workspace
    ON config_proposals(workspace_id, status, created_at);
CREATE INDEX IF NOT EXISTS idx_config_proposals_status
    ON config_proposals(status, created_at);
