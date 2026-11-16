-- Last cross-harness context packet built for a task.
CREATE TABLE IF NOT EXISTS session_context_packets (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    packet_json TEXT NOT NULL,
    generated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_session_context_workspace
    ON session_context_packets(workspace_id, generated_at DESC);
