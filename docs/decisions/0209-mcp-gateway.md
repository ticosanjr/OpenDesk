# 0209 — MCP completo em `opendesk-mcp`

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** `mcp_config`, `mcp_pool` e `mcp_server` viviam no core, embora o
  crate `opendesk-mcp` já tivesse rate e RBAC.

## Decisão

1. Os três módulos mudam para `opendesk-mcp` (`config`, `pool`, `server`); o
   core reexporta os nomes antigos.
2. O pool é dividido em módulo + testes, mantendo stdio/HTTP e o contrato
   `server__tool`.
3. RBAC, rate e auditoria ficam no ponto de chamada HTTP (`api/mcp.rs`), que já
   registra 403/429/200 no `api_audit`; elicitation/sampling continuam nas
   portas de domínio e no HITL 2.0.
4. O crate depende de storage/engine, nunca do shell.

## Consequências

- Daemon e CLI podem hospedar MCPs sem carregar o core.
- Testes de negação/rate/auditoria cobrem o caminho real.
