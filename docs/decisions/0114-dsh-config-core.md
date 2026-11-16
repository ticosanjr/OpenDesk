# 0114 — DSH router e config sync no core

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** após `browse_tags`, o `dsh_router` só precisava de
  `config_sync`, DB e cliente OpenCode. `config_sync` já não dependia de Tauri
  nem de AppState.

## Decisão

1. Movidos para `opendesk-core`:
   - `dsh_router` (classificador, meta-builder, context packer, templates DSH);
   - `config_sync` (watcher, JSONC, agents/skills/commands, paths).
2. `dsh_router` usa `crate::browse_tags` em vez de `crate::browse`.
3. Core ganha `jsonc-parser`, `notify`, `notify-debouncer-full`,
   `unicode-normalization` e `serde_yaml`.
4. Shell reexporta ambos; chamadas `crate::dsh_router`/`crate::config_sync`
   continuam válidas.
5. `ConfigWatcher` ganha `Default` para satisfazer clippy.
6. Core soma 162 testes; shell 420 (582 total).

## Consequências

- Prompt packing, templates e watcher de configuração já são compartilháveis
  com daemon/CLI.
- `browse` ainda depende de AppState para instalar MCP/skill; fica no shell.
- Próxima fatia: abstrair browse/api e mover `recipes` + `queue_scheduler`.
