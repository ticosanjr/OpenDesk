# 0121 — Métricas no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `metrics` só dependia de DB/error; ainda vivia no shell.

## Decisão

1. `metrics` migra para `opendesk-core`.
2. Shell reexporta `crate::metrics`.
3. Testes usam o `test_support` do core.
4. Core soma 207 testes; shell 376 (583 total).

## Consequências

- Observabilidade operacional pode ser usada por daemon/CLI.
- Restam engine de receitas, fila/api e cascas para concluir REF-01.
