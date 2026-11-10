---
name: operação do OpenDesk
description: Opera tarefas, rotinas, receitas, fila e atenção pelo MCP opendesk-control sem violar os gates humanos.
---
# Operação do OpenDesk

## Quando usar

Quando o usuário pedir para criar, editar, consultar ou executar algo dentro do
próprio OpenDesk: tarefas do Kanban, rotinas, receitas, fila, execuções,
artefatos ou itens de atenção.

Não use para implementar a lógica de execução. O OpenDesk delega ao OpenCode;
o MCP apenas opera o estado e a configuração.

## Dois MCPs, duas fronteiras

- **`opendesk-control`**: operação direta. Tarefas, execuções, rotinas,
  receitas, fila, artefatos e leitura da atenção.
- **`opendesk-config`**: propostas com aprovação humana. Agentes, skills,
  comandos, MCPs, `opencode.jsonc`, receitas e rotinas que precisam de revisão.

Prefira `opendesk-control` para trabalho normal. Use `opendesk-config` quando a
mudança for estrutural ou ambígua.

## Regras invioláveis

1. **Rotina e receita nascem desligadas.** Não chame `routine_set_active` nem
   `recipe_set_enabled` sem um pedido explícito do usuário. Explique o que vai
   ligar e por quê.
2. **Nunca aprove HITL, permissões ou propostas.** `attention_list` é leitura.
   Mostre o item e peça a decisão.
3. **Não apague destrutivamente.** Não há ferramenta de exclusão; proponha.
4. **Não crie loops.** Não crie tarefa para um passo que você já está
   executando, nem uma tarefa que chame o mesmo agente em recursão.
5. **Uma tarefa, um resultado.** Título e descrição devem ter critério de
   conclusão observável.
6. **Use o workspace atual.** Todas as ferramentas já são escopadas pelo
   `OPENDESK_WORKSPACE_ID`.
7. **Saída determinística.** Entregas vão para `output/YYYY-MM/<slug>/`.

## Fluxo para tarefas

1. Confirme o resultado esperado e o agente responsável.
2. `task_create` com `title`, `description`, `assigned_agent` e tags.
3. Se o usuário pedir execução imediata e o motor estiver online,
   `task_dispatch`.
4. Acompanhe com `run_list` (id, status, timeline).
5. Mova para `IN_REVIEW`/`DONE` quando houver evidência.

## Fluxo para rotinas

1. Prefira `recurrence` com `frequency`, `time` e, se preciso, `weekdays`.
2. `routine_create` sempre cria pausada.
3. Valide com `routine_run` e `dry_run=true`.
4. Só ative com pedido explícito: `routine_set_active`.
5. Para desligar, use a mesma ferramenta com `active=false`.

## Fluxo para receitas

1. Leia a receita com `recipe_list` e `id`.
2. Para criar, envie YAML completo em `recipe_save`. Receita nova fica
   desligada automaticamente.
3. Valide com `recipe_run` e `dry_run=true`.
4. Para editar, preserve os HITL existentes e não remova gates sem pedido.
5. Só ligue com `recipe_set_enabled` após revisão humana.

## Consultas úteis

- `opendesk_status`: visão geral de workspace, motor, fila, budget e atenção.
- `run_list`: execuções recentes ou detalhe/timeline de uma execução.
- `artifact_list`: entregas por run, tarefa, agente ou ação.
- `queue_control`: status/pause/resume/cancel/retry quando o usuário pedir.
- `attention_list`: o que está esperando uma pessoa.

## Anti-padrões

- Criar rotina já ativa “para testar”.
- Ligar receita para “ver se funciona”.
- Aprovar permissão para destravar o próprio run.
- Criar tarefas em cascata sem necessidade.
- Editar receita e remover HITL para simplificar.
- Usar `opendesk-config` para operação rotineira.
