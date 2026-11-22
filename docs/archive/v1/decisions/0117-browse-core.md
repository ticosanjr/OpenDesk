# 0117 — Browse no core sem AppState

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `browse` era o último bloqueio direto para mover `recipes`,
  pois recebia `&AppState` apenas para acessar DB e paths.

## Decisão

1. `browse::status/enable/disable/ensure_for_task` passam a receber
   `&SqlitePool` e `&AppPaths` em vez de `&AppState`.
2. O módulo inteiro migra para `opendesk-core`; ele usa `config_sync`, DB,
   paths e helpers de tags, todos já no core.
3. Shell reexporta `crate::browse`; API, dispatch e cron passam `&state.db` /
   `&state.paths`.
4. Testes de browse rodam no core com `test_support` local.

## Consequências

- Browse/MCP/skill podem ser provisionados por daemon/CLI.
- `recipes` ainda depende de `api`/`queue_scheduler`; esse é o próximo
  desacoplamento necessário antes de mover receitas.
- Core soma 174 testes; shell 409 (583 total).
