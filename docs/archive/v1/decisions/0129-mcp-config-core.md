# 0129 — MCP de configuração no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `mcp_config` só usa DB, proposals, paths e o loop MCP.

## Decisão

1. `mcp_config` migra para `opendesk-core`.
2. Shell reexporta `run_mcp_config`.
3. Core soma 224 testes; shell 359 (583 total).

## Consequências

- Propostas de configuração via MCP podem rodar no daemon/CLI.
- `mcp_bridge` continua no shell por depender de integrações Google.
