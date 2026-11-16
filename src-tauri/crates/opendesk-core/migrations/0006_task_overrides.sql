-- Distinguish an explicit user-selected agent/model from a value the router
-- wrote for display. Only explicit overrides should win over the policy.
ALTER TABLE tasks ADD COLUMN agent_override INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tasks ADD COLUMN model_override INTEGER NOT NULL DEFAULT 0;
