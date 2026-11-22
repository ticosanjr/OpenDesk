# 0111 — `secrets` no core e próximo bloqueio do `session_runner`

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** o `session_runner` (ainda no shell) depende de `secrets`,
  `integrations::telegram` e `integrations::signal`. Para mover o cliente
  OpenCode para o core, o primeiro passo é tirar `secrets` do shell.

## Decisão

1. `secrets` migra para `opendesk-core` com AES-256-GCM, arquivos `0600` e
   fallback de ambiente inalterados.
2. O shell reexporta `crate::secrets`.
3. O core ganha a dependência `aes-gcm`; testes de roundtrip/remoção rodam em
   `cargo test -p opendesk-core`.
4. A extração de `opencode_client`/`adapters`/`runtime`/`sandbox` fica
   bloqueada por uma dependência de domínio: `session_runner` envia notificações
   Telegram/Signal diretamente.

## Próximo desenho necessário

Antes de mover o cliente:

- introduzir um trait `RunNotifier` no core (`run_finished`, `hitl`, `error`);
- o shell injeta uma implementação Telegram/Signal;
- `RunRequest`/`RuntimeRequest` recebem `Arc<dyn RunNotifier>` opcional;
- testes usam um notifier nulo.

Isso remove `crate::integrations` do `session_runner` sem perder os avisos.

## Consequências

- Segredos já são compartilháveis com o futuro daemon/CLI.
- O core soma 84 testes; o shell mantém 496.
- Nenhum contrato de API mudou.
