# 0127 — Dispatcher injetado no engine/DAG

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o `execute_step` construía `ShellRecipeDispatcher` internamente,
  mantendo o engine preso ao shell mesmo após o trait.

## Decisão

1. `execute` e `dag::execute` recebem `&dyn RecipeStepDispatcher`.
2. `start`/`resume` criam `ShellRecipeDispatcher` uma vez e o passam adiante.
3. `execute_step` usa o dispatcher recebido; não instancia mais shell.
4. O DAG carrega o dispatcher no `Ctx`.
5. Testes usam `NoopRecipeDispatcher`.

## Consequências

- Basta trocar a injeção para mover o engine ao core.
- Próximo passo: `RecipeContext` com pool/paths/events/secrets para remover
  `AppState` do engine/HITL/sinks.
