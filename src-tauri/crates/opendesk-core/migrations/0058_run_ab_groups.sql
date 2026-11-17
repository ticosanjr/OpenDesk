-- A/B experiment groups (C2).
CREATE TABLE IF NOT EXISTS run_ab_groups (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    prompt TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'OPEN'
        CHECK (status IN ('OPEN', 'DECIDED', 'CANCELLED')),
    winner_slot TEXT CHECK (winner_slot IN ('a', 'b')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    decided_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_run_ab_groups_workspace
    ON run_ab_groups(workspace_id, created_at DESC);
