# 0120 — Exportação de runs no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `run_export` só usava `AppState` para acessar o pool SQLite.

## Decisão

1. `run_export::load_timeline` e `load_thread_timeline` recebem
   `&SqlitePool` em vez de `&AppState`.
2. `build_timeline`/`load_run_inputs` também usam pool.
3. O módulo migra para `opendesk-core`; shell reexporta e a API passa
   `&state.db`.
4. Core ganha `html-escape`.
5. Core soma 205 testes; shell 378 (583 total).

## Consequências

- Export Markdown/HTML/JSON e timeline podem ser usados por daemon/CLI.
- Restam engine de receitas, fila/api e cascas para fechar REF-01.
