---
description: Cria uma tarefa durável no Kanban do OpenDesk.
agent: build
---
Transforme o pedido abaixo em uma tarefa do OpenDesk.

1. Defina título curto e descrição com resultado observável.
2. Escolha o agente responsável e tags úteis.
3. Chame `task_create` no workspace atual.
4. Devolva o id, o status e o que falta para concluir.
5. Só chame `task_dispatch` se o usuário pedir execução imediata.

Nunca crie tarefas em cascata nem tarefas para passos que você já está
executando.

Pedido: $ARGUMENTS
