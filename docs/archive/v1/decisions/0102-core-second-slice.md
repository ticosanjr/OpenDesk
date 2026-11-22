# 0102 — Segunda fatia do `opendesk-core`

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a fundação extraída ainda tinha módulos de domínio sem Tauri no
  crate do shell: worktrees por run e display virtual isolado.

## Decisão

1. `worktree` e `virtual_display` migram para `crates/opendesk-core`.
2. O crate raiz reexporta ambos, então `crate::worktree` e
   `crate::virtual_display` continuam válidos para API, executor e UI.
3. `opendesk-core` ganha dependências de domínio (`chrono`, `uuid`, `walkdir`) e
   `tempfile` como dev-dependency para os testes de worktree.
4. Nenhum módulo extraído depende de Tauri, SQLite, AppState ou Axum.
5. `cargo test -p opendesk-core` executa os testes de git worktree, overlay e
   Xvfb sem subir o shell.

## Consequências

- O core agora cobre contratos, paths, eventos, estimativa de custo, isolamento
  de workspace e display virtual.
- O shell continua sendo o dono de DB, fila, adapters, receitas e DSH; a
  extração segue em fatias.
- `cargo test --lib` roda shell e core via `default-members`.
