# 0123 — Backup de workspace no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `backup` só usava `AppState` para DB/paths.

## Decisão

1. `backup` migra para `opendesk-core`.
2. `create`/`restore_as_new` recebem `&SqlitePool` + `&AppPaths`; `list`/`get`/`delete` recebem `&AppPaths`.
3. Shell reexporta; API passa pool/paths explicitamente.
4. Core ganha `zip`.
5. Core soma 212 testes; shell 371 (583 total).

## Consequências

- Backup/restore podem ser usados por daemon/CLI.
- Restam engine de receitas, fila/api e cascas.
