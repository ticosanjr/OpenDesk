# 0110 — RBAC por role no gateway MCP HTTP

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o gateway `/api/mcp` filtrava por fase, mas qualquer portador do
  token via o catálogo inteiro. Não havia separação entre leitura, build,
  operação e auditoria.

## Decisão

1. Novo header `x-opendesk-mcp-role` aceita `read`, `audit`, `plan`, `build`,
   `ops`/`operator` e `chat`.
2. Quando o header de fase não é explícito (ou é `ALL`), o role escolhe a fase:
   `read/audit → AUDIT`, `plan → PLAN`, `build → BUILD`, `ops → OPS`,
   `chat → CHAT`.
3. Um `x-opendesk-mcp-phase` explícito continua vencendo o role.
4. O role vale para `tools/list`, `tools/call` e tools dinâmicas; a mesma
   filtragem do MCP stdio é aplicada.
5. Teste HTTP garante que `role=read` vê `task_list` mas não `task_create`.

## Consequências

- Clientes externos podem receber um token/sessão limitada por finalidade.
- A autorização continua baseada no token da API; o role é uma restrição
  operacional, não identidade criptográfica.
- Rate limit por tool e proxy de servidores externos seguem pendentes em MCP-01.
