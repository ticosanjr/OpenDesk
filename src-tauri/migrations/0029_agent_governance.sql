-- Agent governance: hierarchy, goals, budgets, heartbeats and delegations.
CREATE TABLE IF NOT EXISTS agent_profiles (
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
);

CREATE TABLE IF NOT EXISTS agent_goals (
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
);

CREATE INDEX IF NOT EXISTS idx_agent_goals_agent
    ON agent_goals(workspace_id, agent_id, status);

CREATE TABLE IF NOT EXISTS agent_delegations (
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
);

CREATE INDEX IF NOT EXISTS idx_agent_delegations_status
    ON agent_delegations(workspace_id, status, requested_at);
