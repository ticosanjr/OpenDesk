# 0141 — E1: MCP Streamable HTTP/SSE

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** `/api/mcp` só respondia JSON request/response; não havia sessão,
  stream servidor→cliente nem retomada.

## Decisão

1. `McpSessionRegistry` mantém sessões com:
   - `Mcp-Session-Id` retornado no `initialize`;
   - buffer de 128 eventos por sessão;
   - IDs de evento para `Last-Event-ID`;
   - expiração por inatividade (30 min).
2. `POST /api/mcp`:
   - cria sessão no `initialize`;
   - aceita `Accept: text/event-stream` e devolve um evento `message`;
   - notificações recebem `202 Accepted`.
3. `GET /api/mcp` abre o stream servidor→cliente, com replay e keepalive de
   15s.
4. `DELETE /api/mcp` encerra a sessão.
5. A sessão valida o workspace e carrega agente/fase/role usados na política.

## Consequências

- Clientes MCP HTTP completos conseguem inicializar, listar, chamar, reconectar
  e encerrar.
- O stream também é a base para elicitation (E4) e sampling (E5).
- O buffer é em memória; reiniciar o app encerra sessões.
