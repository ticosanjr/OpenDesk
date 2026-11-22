# 0082 — Filtragem dinâmica de tools MCP e descoberta sob demanda

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `opendesk-control` expunha todas as 34 ferramentas em qualquer
  fase. Somadas às tools externas, isso consome contexto e degrada o raciocínio.

## Decisão

1. **Fase/papel no MCP.** O servidor lê `OPENDESK_MCP_PHASE`
   (`PLAN`, `BUILD`, `AUDIT`, `CHAT`, `OPS`, `ALL`) e `OPENDESK_MCP_ROLE`.
   `ALL` preserva o comportamento anterior.
2. **Visibilidade por categoria.** Tools de leitura ficam sempre disponíveis
   (`opendesk_status`, `task_list`, `ask_user`, `memory_search`,
   `mcp_discover_tools`); escrita/planejamento aparece em PLAN/BUILD/CHAT; OPS
   fica restrito a operação; AUDIT é somente leitura.
3. **Descoberta sob demanda.** `mcp_discover_tools({ query, phase })` devolve
   nome, descrição e schema das ferramentas que correspondem à busca e são
   visíveis na fase. O dispatcher continua aceitando qualquer tool do catálogo,
   então a descoberta não bloqueia chamadas legítimas.
4. **Novas tools de governança.** `recipe_resume`, `recipe_cancel` e
   `voice_speak` entram no catálogo; `worktree_*` ficam para a fase de staging.
5. **Compatibilidade.** Clientes que não definem env continuam vendo tudo.

## Consequências

- Um agente PLAN não recebe `queue_control` nem `recipe_run`, mas descobre sob
  demanda.
- O gateway/proxy ativo (MCP-01) e o roteamento de elicitation/sampling
  continuam pendentes; esta decisão cobre a camada nativa.
