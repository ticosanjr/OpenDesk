# 0122 — Auditoria e rate limit no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `audit` só dependia de DB/error; ainda vivia no shell.

## Decisão

1. `audit` (registro, classificação e rate limiter) migra para `opendesk-core`.
2. Shell reexporta `crate::audit`; API continua usando o mesmo middleware.
3. Core ganha `urlencoding`.
4. Core soma 210 testes; shell 373 (583 total).

## Consequências

- Auditoria e rate limit podem ser usados pelo daemon/CLI.
- Restam engine de receitas, fila/api e cascas.
