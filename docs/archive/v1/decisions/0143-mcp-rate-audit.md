# 0143 — E3: rate limit e auditoria por agente/tool

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o RBAC por agente já existia, mas não havia limite de chamadas
  nem registro específico de decisão por tool.

## Decisão

1. `opendesk_core::mcp_rate::McpRateLimiter` usa janela deslizante de 60s por
   `agente:tool`.
2. Setting `mcp_tool_rates`:
   - `default`: limite global;
   - `agents.<agente>.<tool>`: override.
   - `0` significa ilimitado.
3. `/api/mcp` aplica o limite antes de executar `tools/call` e devolve erro
   JSON-RPC `-32002` com o retry em segundos.
4. Toda decisão (`allow`, `deny`, `rate_limited`) grava uma linha em
   `api_audit` com ação `mcp:<tool>`, ator e status 200/403/429.
5. `audit::record_mcp_tool` centraliza a gravação.

## Consequências

- Um agente de leitura pode ter limite menor que o `build`.
- O rate limit é por processo; reiniciar zera as janelas.
- A auditoria usa a tabela existente, sem migração nova.
