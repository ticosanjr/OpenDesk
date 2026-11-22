# 0074 — Bootstrap compartilhado e TUI de operação

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** desktop e `opendeskd` mantinham duas listas de inicialização
  quase iguais (fila, cron, push, IO bus, receitas, governança, pacotes e MCP).
  O CLI tinha chat/approvals/diff, mas não um modo interativo de operação.

## Decisão

1. **`control_plane::start`.** Um único bootstrap recebe `StartOptions` e liga
   daemon, integrações, watcher, fila, cron, push, manutenção, IO bus, receitas,
   heartbeat de governança, triggers, starter packs e MCP bridges.
2. **Desktop fino.** O `lib.rs` mantém apenas janela, tray, atalhos, diálogos,
   gerenciamento do estado e shutdown; o resto vem do bootstrap.
3. **Daemon sem caminho divergente.** `run_headless` usa o mesmo bootstrap com
   `daemon` ligado/desligado por `OPENDESK_SKIP_DAEMON`. O servidor Axum não usa
   mais `tauri::async_runtime::spawn`.
4. **TUI operacional.** `opendesk tui` abre um shell com `tasks`, `runs`,
   `approvals`, `queue`, `nodes`, `diff <run>`, `approve/deny <id>` e `exit`.
   É intencionalmente textual e usa a mesma API local — não duplica execução.
5. **Extração física de crate fica adiada.** O limite agora é explícito e o
   caminho do daemon não depende de código de janela; separar
   `opendesk-core`/`opendesk-daemon` em crates é uma refatoração mecânica que
   pode acontecer sem nova semântica.

## Consequências

- Inicialização e encerramento ficam previsíveis e idênticos entre os modos.
- O terminal cobre a operação diária sem GUI, incluindo HITL e diffs.
- O próximo passo de empacotamento pode mover módulos sem reescrever o
  bootstrap.
