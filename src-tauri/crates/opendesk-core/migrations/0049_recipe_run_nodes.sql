-- Durable recipe nodes and cooperative cancellation. `recipe_runs` keeps its
-- original status CHECK; cancellation is requested via a flag and the driver
-- finishes the run as REJECTED with a clear message.
ALTER TABLE recipe_runs ADD COLUMN cancel_requested INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS recipe_run_nodes (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES recipe_runs(id) ON DELETE CASCADE,
    node_index INTEGER NOT NULL,
    node_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'RUNNING', 'WAITING_HITL', 'DONE', 'FAILED', 'SKIPPED')),
    attempts INTEGER NOT NULL DEFAULT 0,
    session_id TEXT,
    output_json TEXT,
    error_message TEXT,
    started_at TEXT,
    finished_at TEXT,
    UNIQUE (run_id, node_index)
);
CREATE INDEX IF NOT EXISTS idx_recipe_run_nodes_run
    ON recipe_run_nodes(run_id, node_index);
CREATE INDEX IF NOT EXISTS idx_recipe_run_nodes_status
    ON recipe_run_nodes(status);
