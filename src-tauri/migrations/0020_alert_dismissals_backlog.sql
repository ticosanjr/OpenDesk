-- Backlog was removed from the workflow: new and existing tasks start in TODO.
-- Keeping the UPDATE is safe for legacy databases that still contain the value.
UPDATE tasks SET status = 'TODO', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
WHERE status = 'BACKLOG';

-- Alerts dismissed from the Attention Card. Dismissal is per workspace and
-- reference so a resolved warning never comes back after a refresh.
CREATE TABLE IF NOT EXISTS alert_dismissals (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    ref_id TEXT NOT NULL,
    dismissed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_alert_dismissals_unique
    ON alert_dismissals(workspace_id, kind, ref_id);
