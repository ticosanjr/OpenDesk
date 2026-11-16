# 0128 — Web Push no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `push` só usava `AppState` para DB/paths/events.

## Decisão

1. `push::send_to_all` recebe `&SqlitePool` + `&AppPaths`.
2. `push::start` recebe pool/paths/event bus.
3. Módulo migra para `opendesk-core`; shell reexporta.
4. Control plane/API passam DB/paths/events explicitamente.
5. Core ganha `web-push`, `p256` e tokio `io-std`.

## Consequências

- Notificações PWA podem ser usadas pelo daemon/CLI.
- Core soma 222 testes; shell 361 (583 total).
