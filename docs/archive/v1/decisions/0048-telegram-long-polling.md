# 0048 — Integração Telegram por long polling

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** o Signal exige `signal-cli`, Java e vínculo por QR, o que limita o
  uso remoto em celulares e máquinas sem esse ambiente. O Telegram oferece um
  bot gratuito criado pelo `@BotFather` e uma Bot API HTTPS estável.

## Decisão

1. Usar a Bot API com **long polling** (`getUpdates`, `timeout=25`). Não há
   webhook, porta pública, túnel ou processo externo.
2. Guardar o token do bot no `SecretStore` criptografado (AES-256-GCM, `0600`).
   O token nunca é retornado pela API, escrito em log ou incluído em mensagens
   de erro.
3. Exigir allowlist para iniciar a bridge. A allowlist aceita chat IDs
   numéricos (inclusive IDs negativos de grupo) e `@username`. Mensagens de
   chats fora dela são descartadas.
4. Persistir o offset de `getUpdates` em `app_settings`; mudar ou remover o
   token zera o offset. Assim o bot não repete mensagens antigas do bot anterior.
5. Reutilizar o pipeline do Signal: HITL, `aprovar`/`negar` permissões, thread de
   chat `source="telegram"` e resposta do agente no mesmo chat.
6. Espelhar o modelo de notificações do Signal nas rotinas
   (`notify_telegram`, `notify_telegram_recipients`) e no ledger de entregas
   (`channel = TELEGRAM`).
7. Guardar chats já vistos (`telegram_chats`) para resolver `@username` em chat
   IDs no envio. O Telegram não permite enviar mensagem privada para um
   username antes de o usuário falar com o bot.
8. Limitar a primeira entrega a texto e comandos. Áudio, fotos e documentos
   ficam para uma fase posterior.

## Consequências

- O OpenDesk precisa estar rodando (desktop ou `--headless`) para o bot
  responder.
- O usuário cria o bot e cola o token; não há como criar bot programaticamente.
- Em grupos, o modo de privacidade do BotFather pode exigir menção ao bot.
- O polling sobrevive a reinícios sem duplicar mensagens confirmadas, mas uma
  queda no meio de um lote pode reenviar uma resposta (at-least-once).
- Notificações para `@username` só funcionam depois que aquele chat falou com o
  bot ao menos uma vez; para avisos automáticos, prefira um chat ID numérico.
