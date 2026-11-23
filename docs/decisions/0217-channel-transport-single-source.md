# 0217 — Transporte de canais em uma única fonte

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** o `io_bus` ainda tinha seu próprio `send_telegram` e chamava
  `getUpdates`/`answerCallbackQuery` direto na Bot API, enquanto
  `opendesk-channels` já possuía o cliente Telegram. Delivery/HITL usavam as
  funções do crate por reexport, mas o I/O bus era uma terceira cópia.

## Decisão

1. `opendesk-channels::telegram` ganhou `send_text_with_parse_mode` (HTML usado
   pelo sink de artefatos) e `get_updates_value` (formato bruto consumido pelo
   bridge que mantém seu próprio handler).
2. `io_bus` não fala mais com `api.telegram.org`:
   - envio de mensagens e documentos usa `opendesk_channels::telegram`;
   - long polling usa `get_updates_value`;
   - callback usa `answer_callback_query`.
3. `io_bus/chat.rs` ficou só com Matrix (não há adaptador Matrix no crate).
4. Delivery e HITL continuam delegando ao crate; a remoção de
   `reqwest`/`lettre`/CLI foi confirmada por `rg` (nenhum transporte direto
   nesses módulos).

## Consequências

- Um único cliente Telegram, com o mesmo timeout, tratamento de erro e
  chunking.
- O sink HTML preserva `parse_mode`.
- Matrix permanece no shell até existir um adaptador próprio.
- Testes de `opendesk-channels --all-features`, `delivery`, `hitl` e `io_bus`
  seguem verdes.
