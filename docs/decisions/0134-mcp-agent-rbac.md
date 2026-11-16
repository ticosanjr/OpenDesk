# 0134 — E3: RBAC de tools por agente no MCP HTTP

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o gateway MCP filtrava por fase/role, mas não por identidade do
  agente. Todos os agentes do workspace viam o mesmo catálogo.

## Decisão

1. Novo `opendesk_core::mcp_rbac::ToolPolicy` com `allow`/`deny`.
2. Padrões aceitam nome exato ou prefixo com `*` (`task_*`); `deny` vence.
3. Setting `mcp_agent_tools` mapeia agente → política.
4. `POST /api/mcp` lê `x-opendesk-mcp-agent`, carrega a política e:
   - filtra `tools/list`;
   - nega `tools/call` fora da allowlist/denylist.
5. Teste HTTP garante que um agente com `allow: ["task_list"]` vê só uma tool.

## Consequências

- Um cliente pode dar ao agente `audit` apenas leitura e ao `build` apenas
  criação/edição.
- A política é operacional, não identidade criptográfica; o token da API
  continua sendo a autenticação.
- SSE/proxy de servidores externos e elicitation/sampling seguem pendentes.
