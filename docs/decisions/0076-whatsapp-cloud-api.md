# 0076 — WhatsApp Cloud API real (onda 3, fechamento)

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** a onda 3 entregou rotas/trust/tickets e uma configuração
  estrutural de WhatsApp. O conector real ficou pendente.

## Decisão

1. **Cloud API como conector oficial.** `provider=cloud-api` usa
   `phone_number_id`, `api_version` e um access token da Meta. Baileys continua
   apenas nominal (não é dependência do projeto).
2. **Credenciais no SecretStore.** `access_token` e `app_secret` ficam em
   `channel_whatsapp` (0600); a API devolve apenas flags `*_set`. O token de
   webhook continua em `channel_webhook:whatsapp` e serve de `verify_token`.
3. **Verificação do webhook.** `GET /api/webhooks/channels/whatsapp` responde ao
   `hub.challenge` somente com `hub.mode=subscribe` e token correto.
4. **Assinatura obrigatória.** POSTs da Meta exigem
   `X-Hub-Signature-256` HMAC-SHA256 com o app secret. Sem app secret
   configurado, o payload é recusado.
5. **Normalização de mensagens.** Texto, botão, respostas interativas e
   mídia com legenda viram o mesmo `process_inbound` de Telegram/Signal:
   rota por identidade, ticket idempotente, trust e tarefa.
6. **Workspace opcional.** A configuração pode fixar o workspace das mensagens;
   sem isso, usa o workspace ativo.
7. **Envio explícito.** `POST /api/channels/whatsapp/send` envia texto pela
   Graph API e é usado pelo teste da UI. Não há envio automático de respostas.
8. **Mídia.** Imagens/documentos/áudio entram como marcador textual (ou legenda);
   download/armazenamento de mídia fica para uma próxima etapa.

## Consequências

- WhatsApp deixa de ser só estrutura e pode receber e enviar mensagens reais.
- A segurança do webhook não depende de IP allowlist: depende de assinatura.
- O operador precisa configurar app secret, access token, phone number id e
  assinar o webhook no painel da Meta.
