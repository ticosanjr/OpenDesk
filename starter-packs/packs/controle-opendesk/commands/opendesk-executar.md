---
description: Executa uma tarefa, rotina ou receita agora.
agent: build
---
Execute o item pedido abaixo usando o MCP `opendesk-control`.

1. Identifique o tipo: tarefa (`task_dispatch`), rotina (`routine_run`) ou
   receita (`recipe_run`).
2. Prefira `dry_run=true` quando for rotina/receita e ainda não estiver validada.
3. Confirme que o motor OpenCode está online; se não estiver, explique e pare.
4. Devolva o id da execução/job e onde acompanhar (`run_list`).
5. Não aprove HITL nem permissões em nome do usuário.

Pedido: $ARGUMENTS
