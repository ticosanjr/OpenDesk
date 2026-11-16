-- Curated skill evaluation cases (G1).
CREATE TABLE IF NOT EXISTS skill_eval_cases (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    skill_id TEXT NOT NULL,
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    expected_contains TEXT,
    expected_schema_json TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_skill_eval_cases_skill
    ON skill_eval_cases(workspace_id, skill_id, created_at);
