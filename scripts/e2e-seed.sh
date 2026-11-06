#!/usr/bin/env bash
# Seeds a realistic global OpenCode layout for Playwright.
set -euo pipefail

ROOT="/tmp/opendesk-e2e"
GLOBAL="${XDG_CONFIG_HOME:-$ROOT/xdg-config}/opencode"
rm -rf "$ROOT"
mkdir -p \
  "$ROOT/home" \
  "$GLOBAL/agents" \
  "$GLOBAL/skills/global-skill" \
  "$GLOBAL/commands" \
  "$ROOT/xdg-data" \
  "$ROOT/xdg-state" \
  "$ROOT/cache"

cat > "$GLOBAL/opencode.jsonc" <<'JSON'
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "rea": {
      "type": "local",
      "command": ["/usr/bin/true", "mcp"],
      "enabled": true
    }
  }
}
JSON

cat > "$GLOBAL/agents/global-agent.md" <<'MD'
---
name: Global Agent
description: Agente global do fixture
mode: subagent
avatar: "🌍"
---
Você é um agente global.
MD

cat > "$GLOBAL/skills/global-skill/SKILL.md" <<'MD'
---
name: Global Skill
description: Skill global do fixture
---
Use esta skill global.
MD

cat > "$GLOBAL/commands/global-command.md" <<'MD'
---
description: Comando global do fixture
agent: global-agent
---
Execute $ARGUMENTS
MD
