-- Multi-node: the queue records which worker claimed a job. `workers` already
-- holds endpoint/capabilities/token; this only links execution to it.
ALTER TABLE execution_queue ADD COLUMN assigned_node_id TEXT;
CREATE INDEX IF NOT EXISTS idx_execution_queue_node
    ON execution_queue(assigned_node_id, status);
