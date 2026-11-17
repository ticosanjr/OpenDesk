# 0152 — F5: e-mail no Chat, anexos e inbound

- **Status:** aceito
- **Data:** 2026-11-17
- **Contexto:** o endpoint de reply existia, mas não havia UI, anexos nem
  criação de thread a partir do IMAP.

## Decisão

1. `POST /api/chat/threads/{id}/email-reply` aceita `attachments` (caminhos
   relativos ao workspace).
   - valida path, canonicaliza e exige que o arquivo esteja dentro do workspace;
   - respeita o limite de 10 MB por anexo;
   - infere MIME básico por extensão.
2. O Chat mostra o botão **Responder por e-mail** quando `thread.source` é
   `email`.
   - modal com destinatário, assunto, mensagem e caminhos de anexos;
   - mensagem vazia envia a última resposta do assistente;
   - usa `In-Reply-To`/`References` do `external_id`.
3. O coletor IMAP agora cria uma thread de Chat (`source=email`) além de
   ingerir a tarefa.
   - usa `in_reply_to`/`Message-ID` para manter respostas na mesma thread;
   - grava remetente, assunto, Message-ID e anexos no metadata da mensagem;
   - publica `ChatUpdated`.
4. Testes:
   - Rust cria a thread, adiciona duas mensagens e mantém uma única thread;
   - E2E abre o modal, preenche os campos e verifica o erro acionável sem SMTP.

## Consequências

- O histórico de e-mail fica visível e respondível no Chat.
- Anexos usam caminhos do workspace; upload direto pelo navegador ainda não é
  suportado nesse modal.
- O inbound continua criando a tarefa normal para rotinas/automações.
