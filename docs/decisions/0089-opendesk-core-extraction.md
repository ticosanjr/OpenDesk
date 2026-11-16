# 0089 — Extração incremental do `opendesk-core`

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o binário `opendesk` acumula shell Tauri, daemon, CLI, protocolo de
  nós, domínio, banco, fila, adapters e receitas no mesmo crate. Isso impede
  compilar ou testar o domínio sem Tauri e mistura tempos de build.

## Decisão

1. `src-tauri` passa a ser um workspace Cargo com:
   - o pacote `opendesk` (shell Tauri, `opendeskd`, CLI e node runner);
   - `crates/opendesk-core`, que não pode depender de Tauri nem de tipos de GUI.
2. A extração segue o padrão *strangler*: cada fatia move módulos folha,
   reexporta-os no crate antigo (`pub use opendesk_core::{...}`) e mantém todo o
   app verde entre as fatias.
3. Primeira fatia: `error`, `events`, `paths`, `text` e `token_estimate`. Todos
   já eram independentes de Tauri e concentram contratos usados por todas as
   cascas.
4. `default-members` inclui o crate raiz e o core, então `cargo test --lib`,
   `cargo clippy --all-targets` e o gate cobrem os dois sem comandos extras.
5. As próximas fatias movem `db`, fila, adapters, receitas e DSH; somente depois
   disso os binários `opendesk-daemon`, `opendesk-cli` e `opendesk-node` viram
   pacotes próprios.

## Consequências

- `cargo tree -p opendesk-core` não contém Tauri.
- Mudanças em contratos-base deixam de exigir recompilar o shell inteiro para
  testes pequenos.
- O binário do daemon ainda carrega Tauri nesta fatia; a separação física das
  cascas é o objetivo final do REF-01, não deste incremento.
