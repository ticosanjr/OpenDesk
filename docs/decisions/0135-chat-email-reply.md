# 0135 — F5: resposta de thread de e-mail a partir do Chat

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** os headers SMTP/IMAP já existiam, mas o Chat não tinha um
  caminho para responder uma thread.

## Decisão

1. Novo `POST /api/chat/threads/{id}/email-reply` com `to`, `subject` e
   `body_html` opcional.
2. O `external_id` da thread é usado como `In-Reply-To`/`References`.
3. Sem `body_html`, envia a última mensagem do assistente como `<pre>` escapado.
4. Thread sem `external_id` ou sem SMTP configurado retorna erro acionável.
5. Teste cobre thread de e-mail e ausência de SMTP.

## Consequências

- O backend já preserva a continuidade de thread a partir do Chat.
- A UI do Chat ainda precisa expor o botão/campo de resposta.
- Anexos e criação de thread a partir do IMAP continuam pendentes em F5.
