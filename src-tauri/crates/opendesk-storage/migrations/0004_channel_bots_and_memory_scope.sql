-- Plano v3 F5/F7: bots de chat por workspace, contextos isolados e escopo de memória.
CREATE TABLE IF NOT EXISTS channel_bots (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  channel TEXT NOT NULL CHECK(channel IN ('telegram', 'signal', 'whatsapp', 'matrix')),
  name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 0,
  secret_ref TEXT,
  config_json TEXT NOT NULL DEFAULT '{}',
  allowlist_json TEXT NOT NULL DEFAULT '[]',
  default_agent TEXT NOT NULL DEFAULT 'build',
  status TEXT NOT NULL DEFAULT 'stopped' CHECK(status IN ('stopped', 'running', 'error')),
  last_error TEXT,
  last_checked_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(workspace_id, channel, name)
) STRICT;
CREATE INDEX IF NOT EXISTS idx_channel_bots_ws ON channel_bots(workspace_id, channel);

CREATE TABLE IF NOT EXISTS channel_chat_contexts (
  id TEXT PRIMARY KEY,
  bot_id TEXT NOT NULL REFERENCES channel_bots(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  channel TEXT NOT NULL,
  external_chat_id TEXT NOT NULL,
  chat_type TEXT NOT NULL DEFAULT 'group',
  title TEXT NOT NULL,
  system_context TEXT NOT NULL DEFAULT '',
  agent_override TEXT,
  memory_scope TEXT NOT NULL DEFAULT 'chat' CHECK(memory_scope IN ('chat', 'normal', 'both', 'none')),
  opencode_session_id TEXT,
  enabled INTEGER NOT NULL DEFAULT 1,
  last_message_at TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  UNIQUE(bot_id, external_chat_id)
) STRICT;
CREATE INDEX IF NOT EXISTS idx_channel_contexts_bot ON channel_chat_contexts(bot_id, enabled);

ALTER TABLE personal_context_groups ADD COLUMN usage_scope TEXT NOT NULL DEFAULT 'both'
  CHECK(usage_scope IN ('normal', 'chat', 'both'));

ALTER TABLE memory_notes ADD COLUMN usage_scope TEXT NOT NULL DEFAULT 'both'
  CHECK(usage_scope IN ('normal', 'chat', 'both'));
