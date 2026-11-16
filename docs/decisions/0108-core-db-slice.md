# 0108 — DB e domínio de nós no `opendesk-core`

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** após as fatias de foundation, worktree, Xvfb, sync e lock, o
  maior bloco ainda no shell era o banco: DB, migrações, fila, nós, passkey e
  dedupe. Sem ele, nem daemon nem CLI podiam ser cascas finas.

## Decisão

1. Movidos para `crates/opendesk-core`:
   - `db` inteiro (models, migrações, repositórios, maintenance, write_buffer,
     worktrees, session packets);
   - `dedupe`;
   - `nodes`;
   - `passkey`.
2. `migrations/` migra para a raiz do crate core; `sqlx::migrate!("./migrations")`
   aponta para lá.
3. O shell reexporta `db`, `dedupe`, `nodes` e `passkey`; todas as chamadas
   `crate::db::...` continuam funcionando sem mudanças.
4. `write_buffer` troca `tauri::async_runtime::spawn` por `tokio::spawn`.
5. O core ganha `webauthn-rs`, `base64`, `rand`, `tokio-util` e
   `iana-time-zone`, além das features de tokio necessárias aos testes.
6. Um `test_support` do core cria SQLite + workspace + write-buffer sem
   `AppState`; os testes de DB deixam de depender do shell.
7. `AppPaths` ganha `allow_temp_workspaces`, propagado do ambiente e ligado
   nos helpers de teste; isso mantém a validação de workspace correta quando o
   DB é compilado como dependência (sem `cfg!(test)` no crate do chamador).
8. O display virtual reclama locks/sockets X11 obsoletos de processos mortos,
   evitando esgotar `:90–:129` após crashes.
9. O core soma 83 testes; o shell mantém 495. `cargo tree -p opendesk-core`
   não contém Tauri.

## Consequências

- DB, nós, passkey e dedupe agora rodam em `cargo test -p opendesk-core`.
- O shell ainda é dono de fila, adapters, receitas, DSH, I/O bus e comandos.
- `AppState` continua no shell, mas os repositórios e migrações já são
  compartilháveis com o futuro `opendesk-daemon`.
- Próxima fatia: fila/adapters/runtime e depois os binários daemon/CLI/node.
