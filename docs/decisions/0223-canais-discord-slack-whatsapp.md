# 0223 — Canais Discord, Slack e WhatsApp QR

- **Status:** aceito
- **Data:** 2026-11-23
- **Contexto:** `opendesk-channels` cobre Signal, Telegram, WhatsApp Cloud API,
  e-mail e WebPush. Faltam canais de equipe (Discord/Slack) e WhatsApp Web
  pessoal. Tokens de bot precisam viver no Vault e todo canal deve respeitar o
  mesmo roteador de confiança e HITL.

## Decisão

1. Discord usa Gateway WebSocket com heartbeat/resume, allowlist por
   `guild_id`/`channel_id`/`user_id`, threads e botões de HITL/stop. O bot token
   é lido do Vault no momento da conexão.
2. Slack usa Socket Mode (`apps.connections.open` + WebSocket), sem porta
   pública, com app token + bot token no Vault, allowlist por canal/usuário e
   Block Kit para HITL.
3. WhatsApp Web pessoal usa um sidecar externo opcional, com protocolo
   JSON-RPC sobre stdio/HTTP, no mesmo modelo do `signal-cli`. O OpenDesk não
   embute Baileys nem instala o sidecar automaticamente; o doctor detecta e
   orienta.
4. Todos os canais passam pelo `router.rs` de trust levels
   (`trusted`/`internal`/`untrusted`). Mensagem fora da allowlist é descartada
   antes de atingir o LLM, ou vira ticket `untrusted` quando a política do
   workspace permitir.
5. Reconexão usa backoff com teto de 60 s e circuit breaker após 5 falhas;
   desconexão publica `channel.disconnected` para o Attention Card.
6. O envio de HITL, stop e anexos é extraído para um `ChannelTransport` comum,
   sem duplicar lógica entre Telegram, Signal, WhatsApp, Discord e Slack.

## Consequências

- Discord/Slack ficam atrás das features `ch-discord`/`ch-slack`; WhatsApp QR
  atrás de `ch-whatsapp-qr`.
- Nenhum token de canal é persistido em `config_json`.
- O WhatsApp QR depende de um sidecar instalado pelo usuário; a UI mostra o QR
  e o estado da sessão, mas não presume que o binário exista.
