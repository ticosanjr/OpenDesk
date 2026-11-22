# 0201 — Onda 2 (início): engine, recipes, recorrência, worktree e parser

- **Status:** aceito como entrega parcial
- **Data:** 2026-11-22
- **Contexto:** a Onda 1 deixou os repositórios de `db` no core. O `engine` não
  pode depender do core (regra da Onda 2), então a fila, o runner, a telemetria
  e o DAG continuam bloqueados até esses repositórios migrarem para o storage.
  Para não parar a onda, os módulos já desacoplados foram extraídos agora.

## Decisão

1. Criados `opendesk-engine` e `opendesk-recipes`, ambos dependendo apenas de
   `opendesk-domain` e `opendesk-storage`.
2. **Recorrência única:** `recurrence.rs` foi para
   `engine/src/scheduler/recurrence.rs`; `recipes/cron.rs` consome essa
   implementação. O core reexporta o módulo, então `crate::recurrence` continua
   válido no shell.
3. **Worktree:** `worktree.rs` (831 linhas) foi dividido em
   `engine/src/worktree/{mod,manifest,git_ops,overlay,sweeper}.rs` + `tests.rs`.
   O core reexporta `opendesk_engine::worktree`.
4. **Parser:** `recipes/parser.rs` (1278) virou
   `recipes/src/parser/{mod,schema,expressions}.rs` + `tests.rs`. O core
   reexporta `opendesk_recipes::parser`.
5. Nenhum arquivo novo passa de 500 linhas; nenhum `unwrap/expect` fora de
   testes; clippy limpo.

## Consequências

- O shell não mudou de API: `crate::worktree`, `crate::recurrence` e
  `crate::recipes::parser` agora atravessam reexports.
- Testes: 340 shell + 267 core + 19 domain + 6 engine + 13 recipes + 15 storage
  (**660**).
- **Pendente da Onda 2:** fila, scheduler, runner, telemetria/export, router,
  adapters, DAG, store, fachada do motor, event-driven e cobertura de 75%.
  Todos dependem de mover os 28 repositórios restantes de `db/` para o storage
  (a parte não concluída da Onda 1). Sem isso, movê-los exigiria o engine
  depender do core, violando a regra da onda.
- A cobertura do gate segue no escopo antigo (54,61% total / 64,10% core); os
  novos crates ainda não entram no `coverage-core.sh`, então a meta de 75% não
  foi atingida.
