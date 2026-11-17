# 0161 — A9: reexports do shell (parcial)

- **Status:** aceito como pendência consciente
- **Data:** 2026-11-17
- **Contexto:** o shell ainda faz `pub use opendesk_core::{db, api_endpoint, …}`
  para manter milhares de imports `crate::db::…` funcionando.

## Decisão

1. Os reexports permanecem por enquanto; o core é a única implementação.
2. Não há duplicação de lógica: o shell apenas reexporta.
3. A remoção definitiva exige migrar os imports do shell para
   `opendesk_core::…` em uma alteração mecânica separada, com o gate verde.

## Consequências

- A9 não foi marcado como concluído.
- O objetivo arquitetural (core sem Tauri, daemon/CLI/node sem UI) foi atingido;
  falta apenas a limpeza cosmética dos caminhos de import.
