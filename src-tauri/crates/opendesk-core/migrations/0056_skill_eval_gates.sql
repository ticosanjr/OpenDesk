-- Historical skill evaluation gate and human decision.
CREATE TABLE IF NOT EXISTS skill_eval_gates (
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
);

CREATE INDEX IF NOT EXISTS idx_skill_eval_gates_skill
    ON skill_eval_gates(workspace_id, skill_id, created_at DESC);
