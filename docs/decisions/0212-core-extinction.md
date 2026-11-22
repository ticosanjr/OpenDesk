# 0212 — Extinção do `opendesk-core`

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** depois das fases 1 e 2 o core era só um conjunto de reexports e
  alguns módulos de shell/storage, mas continuava sendo uma dependência de todos
  os binários e escondia a fronteira real.

## Decisão

1. Módulos de apresentação/operação (backup, métricas, auditoria, passkey,
   desktop integration, API interna, manutenção) vão para o shell.
2. Repositórios de chat, turnos, interações e mensagens de tarefa vão para
   `opendesk-storage::repos`.
3. O shell passa a importar diretamente os crates definitivos; `crate::db` é uma
   fachada sobre `opendesk_storage::repos`.
4. O crate `opendesk-core` é removido do workspace, do default-members e de
   `src-tauri/Cargo.toml`; não há shim.
5. A baseline de `scripts/rust-limits-baseline.txt` fica vazia.

## Consequências

- `test ! -d src-tauri/crates/opendesk-core` e
  `cargo check --workspace --no-default-features` viram critérios de release.
- Um módulo novo precisa escolher um crate real; não existe mais “joga no core”.
- O shell continua dono dos handlers HTTP, mas sem reexportar domínio.
