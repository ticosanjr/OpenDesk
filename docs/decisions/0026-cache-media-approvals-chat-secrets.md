# ADR 0026 — Cache, mídia, aprovações remotas, segredos e chat

- **Status:** aceito
- **Data:** 2026-11-07
- **Contexto:** a especificação complementar foi ampliada com cinco frentes de
  integração fina: cache do OpenCode Go, ativos multimídia, aprovação
  human-in-the-loop fora do desktop, segredos fora do SQLite e conversa contínua
  com anexos.

## Decisão

### Cache do OpenCode Go
- O `context_packer` coloca primeiro o prefixo invariável (regras permanentes +
  memória do workspace) e só depois tarefa, descrição e diretório de saída.
  Isso preserva o maior prefixo estável possível para prompt caching.
- `/api/cost/mix` passou a devolver `cache_read` e `cache_ratio`; o card de
  model mix mostra o cache hit da semana.

### Mídia
- `GET /api/files/media` usa `ffprobe` para devolver formato, duração, bitrate,
  codecs, resolução e canais.
- O explorador mostra esses metadados ao abrir vídeo ou áudio.
- O caminho continua compatível com MCPs de pós-produção; nenhum binário de
  edição é embutido.

### Aprovação remota
- O pedido de permissão é enviado pelo bridge Signal já existente para a
  allowlist, com um prefixo curto do request id.
- Respostas `APROVAR <prefixo>` e `NEGAR <prefixo>` resolvem o pedido sem abrir
  o desktop; o bridge continua exigindo allowlist.
- E-mail/Telegram/WhatsApp podem usar o mesmo contrato de reply.

### Segredos
- Os segredos continuam em AES-256-GCM com arquivos 0600 e nunca entram no
  SQLite.
- Quando não há arquivo, o SecretStore lê `OPENDESK_<NAMESPACE>_<KEY>` do
  ambiente e nunca persiste esse valor.

### Chat pair-programming
- O composer aceita arquivos e imagens por colagem ou arraste.
- Anexos são gravados em `input/chat/YYYY-MM/` e o caminho entra na mensagem.
- O usuário continua alternando agente e modelo por thread.

## Consequências

- O consumo de cache fica visível e o prompt é mais cache-friendly.
- Vídeo e áudio têm metadados técnicos no viewer e nos artefatos.
- Uma aprovação urgente pode ser resolvida pelo celular via Signal.
- Credenciais não aparecem em banco, logs ou prompts.
- Conversas aceitam contexto multimodal sem sair do OpenDesk.
