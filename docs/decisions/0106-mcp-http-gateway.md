# 0106 — MCP gateway HTTP (primeira fatia)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o MCP de controle era acessível apenas por stdio. PWA, daemon
  headless e clientes externos precisavam de um endpoint único.

## Decisão

1. `POST /api/mcp` aceita JSON-RPC 2.0 com os métodos `initialize`,
   `notifications/initialized`, `tools/list` e `tools/call`.
2. O gateway reutiliza exatamente o catálogo, as tools dinâmicas de
   receitas/agentes e o dispatch do MCP stdio; não há duplicação de regras.
3. `tools/call` retorna o resultado no formato MCP (`content` textual +
   `isError`), preservando o JSON retornado pelo backend.
4. Headers opcionais:
   - `x-opendesk-agent` define o agente padrão;
   - `x-opendesk-mcp-phase` filtra tool list/call por fase.
5. A rota usa o middleware de Bearer token, rate limit e auditoria já existentes
   para `/api/*`.
6. Testes cobrem initialize e tools/list; tools/call usa o mesmo dispatch já
   coberto pelo MCP stdio.

## Consequências

- Existe um endpoint único e autenticado para ferramentas de controle do
  OpenDesk.
- Ainda não é um proxy de servidores MCP externos, não faz streaming
  (Streamable HTTP/SSE) e não tem RBAC por agente além do filtro de fase.
- Esses itens continuam em MCP-01; elicitation/sampling continuam em MCP-03.
