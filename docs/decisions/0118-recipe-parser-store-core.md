# 0118 — HITL kind, parser e store de receitas no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o `recipes` ainda dependia do shell para o vocabulário HITL, o
  parser YAML e o store durável. O engine continua no shell por depender de
  dispatch/queue.

## Decisão

1. `hitl.rs` (HitlKind) migra para `opendesk-core` como `hitl`.
2. `recipes::parser` e `recipes::store` migram para `opendesk-core/recipes`.
3. O shell reexporta `hitl` e `recipes::{parser, store}`; o engine, HITL de
   canais, sinks e cron continuam no shell.
4. Core ganha `jsonschema` e `similar`.
5. Testes de parser, versionamento, DAG e store rodam no core; shell 386,
   core 197 (583 total).

## Consequências

- Contratos de receita, validação YAML, nós duráveis e versões já são
  compartilháveis com daemon/CLI.
- Restam engine, hitl de canais, sinks e cron; eles dependem de
  dispatch/queue/AppState.
- Próximo passo: extrair o executor de receitas com um trait de dispatch.
