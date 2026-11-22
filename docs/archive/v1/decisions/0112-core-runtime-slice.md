# 0112 — Runtime, sandbox, adapters e cliente OpenCode no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** depois do `RunNotifier`, o cliente OpenCode e a camada de
  execução não dependiam mais de AppState/integrações. Ainda viviam no shell.

## Decisão

1. Movidos para `opendesk-core`:
   - `opencode_client` (daemon manager, rest client, session runner, updater,
     binary locator);
   - `runtime` (contrato de harness);
   - `sandbox` (política e wrappers bwrap/container);
   - `adapters` (CLI/OpenCode).
2. `session_runner` usa `crate::notifier::RunNotifier`; o shell injeta
   `ShellRunNotifier` com Telegram/Signal.
3. `daemon_manager` usa `tokio::spawn`; `cleanup_orphaned_daemons` e
   `chat_turn_signals` passam a `pub` para o shell.
4. Shell reexporta os módulos; `crate::opencode_client`, `crate::runtime`,
   `crate::sandbox` e `crate::adapters` continuam válidos.
5. Core ganha `async-trait`, `futures-util`, `semver` e features de tokio para
   processo/IO/fs; reqwest com JSON/stream/rustls.

## Consequências

- Core soma **120 testes**; shell mantém 460; total 580.
- O runtime de execução já pode ser reutilizado pelo futuro daemon/CLI.
- A fila e receitas ainda dependem de integrações/browse/api; são a próxima
  fatia antes de criar os binários separados.
