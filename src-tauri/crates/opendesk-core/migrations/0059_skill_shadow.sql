-- Shadow replay runs for skill evaluation (G2) and promotion (G3).
CREATE TABLE IF NOT EXISTS skill_shadow_runs (
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
);

CREATE INDEX IF NOT EXISTS idx_skill_shadow_runs_skill
    ON skill_shadow_runs(workspace_id, skill_id, created_at DESC);

CREATE TABLE IF NOT EXISTS skill_shadow_results (
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
);

CREATE INDEX IF NOT EXISTS idx_skill_shadow_results_run
    ON skill_shadow_results(run_id);
