-- Harness selection (wave 2): tasks and queue jobs remember which runtime
-- adapter executes them. OpenCode stays the default, so existing rows keep the
-- previous behavior.
ALTER TABLE tasks ADD COLUMN preferred_harness TEXT;
ALTER TABLE execution_queue ADD COLUMN resolved_harness TEXT NOT NULL DEFAULT 'opencode';

ALTER TABLE adapter_registry ADD COLUMN args_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE adapter_registry ADD COLUMN health_args_json TEXT NOT NULL DEFAULT '["--version"]';

-- Built-in harnesses. command/args can be edited through /api/adapters.
INSERT OR IGNORE INTO adapter_registry
    (id, name, kind, command, capabilities_json, status, args_json, health_args_json)
VALUES
    ('opencode', 'OpenCode V2', 'opencode', NULL,
     '{"streaming":true,"tools":true,"permissions":true,"reasoning":true,"usage":true,"diffs":true,"sessions":true,"sandbox":false}',
     'UNKNOWN', '[]', '[]'),
    ('claude-code', 'Claude Code', 'cli', 'claude',
     '{"streaming":true,"tools":true,"permissions":true,"reasoning":true,"usage":true,"diffs":false,"sessions":false,"sandbox":false}',
     'UNKNOWN',
     '["-p","{prompt}","--output-format","stream-json","--verbose"]',
     '["--version"]'),
    ('codex', 'Codex CLI', 'cli', 'codex',
     '{"streaming":true,"tools":true,"permissions":false,"reasoning":true,"usage":true,"diffs":false,"sessions":false,"sandbox":false}',
     'UNKNOWN', '["exec","--json","{prompt}"]', '["--version"]'),
    ('pi', 'Pi', 'cli', 'pi',
     '{"streaming":true,"tools":true,"permissions":false,"reasoning":false,"usage":false,"diffs":false,"sessions":false,"sandbox":false}',
     'UNKNOWN', '["run","--json","{prompt}"]', '["--version"]'),
    ('hermes', 'Hermes', 'cli', 'hermes',
     '{"streaming":true,"tools":true,"permissions":false,"reasoning":false,"usage":false,"diffs":false,"sessions":false,"sandbox":false}',
     'UNKNOWN', '["run","--json","{prompt}"]', '["--version"]');
