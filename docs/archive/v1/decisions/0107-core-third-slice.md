# 0107 — Terceira fatia do core: sync e lock de instância

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** `workspace_sync` e `server_lock` ainda viviam no shell, apesar
  de não dependerem de Tauri, DB ou AppState.

## Decisão

1. `workspace_sync` e `server_lock` migram para `opendesk-core`.
2. O shell reexporta ambos; `crate::workspace_sync` e
   `crate::server_lock` continuam válidos.
3. `paths::test_paths` passa a ser o helper de testes do core, evitando
   dependência de `test_support`/AppState.
4. `opendesk-core` ganha `sha2`, `tar` e `zstd`; testes de manifest/bundle/
   apply e do lock rodam sem o shell.

## Consequências

- O core cobre agora contratos, paths, eventos, custo, worktree, Xvfb, sync
  delta e lock de processo.
- DB, fila, adapters, receitas e DSH continuam no shell; a extração segue em
  fatias com reexports.
- Nenhum contrato de API/CLI mudou.
