# 0091 — Receitas e agentes como tools MCP dinâmicas

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o MCP `opendesk-control` tinha tools fixas. Clientes externos
  precisavam criar tarefas manualmente para usar uma receita ou um agente.

## Decisão

1. Na inicialização, o MCP consulta `/api/recipes` e `/api/studio/agents` do
   workspace e registra cada receita habilitada como `recipe_<id>` e cada agente
   habilitado como `agent_<id>`.
2. Nomes são sanitizados para `[A-Za-z0-9_-]`; ids repetidos após sanitização
   não sobrescrevem tools estáticas (prefixos distintos).
3. `recipe_<id>` recebe `trigger` (objeto) e `dry_run`, e chama
   `POST /api/recipes/{id}/run` com `trigger_context`.
4. `agent_<id>` recebe `prompt`, `title`, `model` e `mode`: cria a tarefa com o
   agente atribuído e tenta `POST /api/tasks/{id}/dispatch`. Se o autostart já
   enfileirou, a resposta mantém a tarefa e registra `dispatch_error`.
5. `mcp_discover_tools` passa a incluir as tools dinâmicas na busca.
6. Em fase `AUDIT`, tools dinâmicas não são expostas. A descoberta é
   best-effort: se a API local não responder, o MCP continua com as tools fixas.
7. Limite de 50 receitas e 50 agentes por sessão para manter o schema MCP
   utilizável.

## Consequências

- Um cliente MCP externo consegue disparar automações e agentes sem conhecer
  rotas internas.
- Receitas desabilitadas e agentes desabilitados não viram tools.
- `agent_*` respeita o roteador/budget/fila porque passa pelo mesmo dispatch da
  UI; ele não executa nada por conta própria.
- Mudanças de configuração durante uma sessão MCP exigem reiniciar o processo
  do MCP para refletir a nova lista (mesmo comportamento de tools fixas).
