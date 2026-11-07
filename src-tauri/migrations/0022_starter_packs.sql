-- Starter packs and versioned alert dismissals.
-- A dismissal is valid for one revision of the underlying task/run; when the
-- task changes (for example, it is blocked again) the alert reappears.
ALTER TABLE alert_dismissals ADD COLUMN version TEXT NOT NULL DEFAULT '';

CREATE TABLE IF NOT EXISTS starter_pack_installs (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    pack_id TEXT NOT NULL,
    pack_version TEXT NOT NULL,
    files_json TEXT NOT NULL,
    routines_json TEXT NOT NULL DEFAULT '[]',
    installed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_starter_pack_install_unique
    ON starter_pack_installs(workspace_id, pack_id);
