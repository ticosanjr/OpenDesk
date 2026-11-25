-- Fase 9: Ferramentas (scripts como MCP local).
CREATE TABLE script_tools (
    id TEXT PRIMARY KEY,
    workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE,
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    agent_prompt TEXT NOT NULL DEFAULT '',
    runtime TEXT NOT NULL,
    custom_command TEXT,
    script_body TEXT NOT NULL,
    input_schema_json TEXT NOT NULL DEFAULT '{"type":"object","properties":{}}',
    env_secret_refs_json TEXT NOT NULL DEFAULT '{}',
    timeout_secs INTEGER NOT NULL DEFAULT 30,
    require_approval INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE UNIQUE INDEX script_tools_global_slug ON script_tools(slug)
    WHERE workspace_id IS NULL;
CREATE UNIQUE INDEX script_tools_workspace_slug ON script_tools(workspace_id, slug)
    WHERE workspace_id IS NOT NULL;
CREATE INDEX script_tools_workspace ON script_tools(workspace_id, enabled);
