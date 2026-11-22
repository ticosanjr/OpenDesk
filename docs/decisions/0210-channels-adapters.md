# 0210 — Canais no crate, host no shell e roteador no caminho real

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** Telegram, Signal, WhatsApp, e-mail e Web Push viviam no shell;
  o crate de canais só tinha registry e políticas.

## Decisão

1. Transporte/protocolo vão para `opendesk-channels` atrás de features:
   `ch-telegram`, `ch-signal`, `ch-whatsapp`, `ch-email`, `ch-webpush`.
2. O que precisa de `AppState` (chat, HITL, tarefas, daemon) fica no shell como
   orquestração fina; o shell reexporta as funções movidas para não quebrar
   chamadas existentes.
3. `NotificationRouter` passa a filtrar a entrega de runs quando a setting
   `channels.notification_router` existe (incluindo quiet hours); sem setting,
   o comportamento anterior é mantido.
4. Segredos continuam no `SecretStore` (nunca em texto plano no banco).

## Consequências

- `cargo check -p opendesk-channels --no-default-features` compila sem I/O.
- Engine e recipes não importam canais.
- Testes de cada feature rodam isolados.
