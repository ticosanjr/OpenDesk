# 0208 — Engine e receitas completos; cron atrás de porta

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** depois do primeiro corte (ADR 0207), fila, scheduler, receitas e
  export ainda viviam no core. `cron_engine` chamava `recipes::engine::start`
  diretamente, o que impedia mover a fila para o engine sem criar ciclo.

## Decisão

1. `recipes` (`dag`, `store`, `engine`, `hitl`, `parser`), `RecipeContext` e
   `run_export` mudam para `opendesk-recipes`, que depende de domain, storage e
   engine. O core reexporta os módulos para manter a API do shell.
2. O engine define `scheduler::ScheduledRecipeRunner`. O cron recebe um
   `Arc<dyn ScheduledRecipeRunner>` e não conhece receitas.
3. `opendesk-recipes::runner::ContextRecipeRunner` implementa a porta usando o
   `RecipeContext` construído pelo shell.
4. `queue_scheduler` inteiro (worker, cron, execução, settlement, leases e
   recorrência) muda para `opendesk-engine`.
5. O repositório de worktrees muda para `engine::worktree::store`; o core
   reexporta como `db::worktrees`.
6. God-files de engine/recipes são divididos por concern, com `mod.rs`
   reexportando a API pública e testes em módulos próprios.

## Consequências

- Core com ~6,2 mil linhas e sem dependência de receitas/fila.
- O engine pode ser usado pelo daemon/CLI sem carregar o core.
- O cron é testável com um runner falso, sem banco de receitas.
- Próximo passo: mover MCP/canais/enterprise e depois extinguir o core.
