-- Path-addressed recipe nodes for DAG groups (branch/parallel/map).
-- Existing flat runs keep their index as the path.
ALTER TABLE recipe_run_nodes ADD COLUMN node_path TEXT;
ALTER TABLE recipe_run_nodes ADD COLUMN parent_path TEXT;
ALTER TABLE recipe_run_nodes ADD COLUMN item_key TEXT;
ALTER TABLE recipe_run_nodes ADD COLUMN join_policy TEXT;

UPDATE recipe_run_nodes SET node_path = CAST(node_index AS TEXT) WHERE node_path IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_recipe_run_nodes_path
    ON recipe_run_nodes(run_id, node_path);
