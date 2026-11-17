# 0160 — A6/A7/A8: daemon, CLI e node sem o desktop

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** `opendeskd` chamava a lib do shell, que sempre linkava Tauri.

## Decisão

1. O shell ganhou a feature `desktop` (default):
   - Tauri e plugins são dependências opcionais;
   - `run()`, tray, atalhos, comandos Tauri e `open_paths` são `cfg(desktop)`.
2. `opendesk_lib::run_headless`, `run_cli` e os MCPs continuam disponíveis sem
   a feature.
3. Novos crates:
   - `opendesk-daemon` → bin `opendeskd`;
   - `opendesk-cli` → bin `opendesk-cli`;
   - `opendesk-node` → bin `opendesk-node` (força `node run`).
4. `src/bin/opendeskd.rs` do shell foi removido.
5. O bin desktop declara `required-features = ["desktop"]`.

## Verificação

```bash
cargo build -p opendesk-daemon -p opendesk-cli -p opendesk-node
cargo tree -p opendesk-daemon -e normal | rg tauri   # vazio
```

`tauri-build` ainda aparece como build-dependency do pacote shell, mas não há
Tauri em runtime nem nos edges normais do daemon.

## Consequências

- O daemon, a CLI e o node compilam sem a UI.
- O desktop continua sendo o mesmo binário Tauri.
- A separação de build-dependency de Tauri fica para uma futura extração de
  `opendesk-desktop`.
