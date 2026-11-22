# 0115 — Threads de e-mail: headers SMTP e metadados IMAP

- **Status:** aceito
- **Data:** 2026-11-16
- **Contexto:** respostas SMTP perdiam a thread porque não enviavam
  `In-Reply-To`/`References`; o inbound IMAP descartava esses headers.

## Decisão

1. `smtp_mailer::build_message` aceita um `EmailThread`
   (`in_reply_to`, `references`) e injeta os headers.
2. `send_email_threaded` usa o builder; `send_email` continua como atalho sem
   thread.
3. Novo endpoint `POST /api/integrations/smtp/reply` com `to`, `subject`,
   `body_html`, `in_reply_to` e `references`, usando a config SMTP do workspace.
4. IMAP grava `message_id`, `in_reply_to`, `references`, `subject` e `from` no
   metadata do evento normalizado.
5. Teste verifica os headers no `Message::formatted()`.

## Consequências

- Um cliente pode responder mantendo a thread sem reimplementar MIME.
- A UI do Chat ainda precisa usar esse endpoint; o backend já preserva a
  continuidade.
- Anexos no reply ficam para a próxima fatia (o builder já suporta).
