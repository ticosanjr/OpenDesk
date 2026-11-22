# 0125 — Dispatch de passos de receita por trait

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `recipes::engine` chamava diretamente
  `api::tasks::dispatch_task_with_session`, prendendo o engine ao shell.

## Decisão

1. Novo `RecipeStepDispatcher` no core com `RecipeStepRequest`.
2. O shell implementa `ShellRecipeDispatcher` e converte para `DispatchBody`.
3. `execute_step` usa o trait; não importa mais `api::tasks`.
4. O engine continua no shell por enquanto, mas o único acoplamento de dispatch
   está isolado atrás do trait.
5. Nenhum comportamento muda: o dispatcher usa o mesmo caminho de fila/task.

## Consequências

- O próximo passo pode mover o engine e injetar o dispatcher explicitamente.
- AppState ainda é necessário para DB/events/task updates/HITL; a próxima fatia
  deve introduzir um contexto de receita.
