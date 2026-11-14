# 0057 — Turno do chat, envio de arquivos e artefatos de shell

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** pedidos de arquivo pelo Telegram retornavam “O agente concluiu a
  execução sem retornar texto” mesmo quando o agente produzia uma resposta
  final. A causa era a leitura de uma mensagem intermediária de ferramenta. Além
  disso, o chat não tinha como anexar um arquivo, e arquivos criados por shell
  não entravam na proveniência.

## Decisão

1. **Fim do turno exige texto e idle.** `wait_for_turn` só retorna quando existe
   pelo menos uma parte `text` não vazia do turno e o OpenCode sinaliza `idle`.
   A rede de segurança por assinatura estável passou a exigir texto e 30 s de
   silêncio; mensagens com apenas chamadas de ferramenta nunca encerram o turno.
   Se o turno terminar sem texto, a mensagem vira um aviso explícito com erro
   `empty assistant response`, em vez de fingir sucesso.
2. **Envio de arquivo pelo chat.** Novo endpoint
   `POST /api/chat/threads/{id}/files` e ferramenta MCP `chat_send_file`
   (`thread_id`, `path`, `caption` opcional).
   - O caminho é resolvido dentro do workspace; symlinks que escapam são
     rejeitados e o limite é 50 MB.
   - Telegram usa `sendDocument`; Signal usa `send_with_attachments`.
   - No desktop o endpoint devolve `sent: false` com o caminho para o agente
     citar na resposta.
   - O contexto de Telegram, Signal e desktop passa a informar o `thread_id` e,
     nos canais externos, a instrução de usar `chat_send_file`.
3. **Varredura do diretório de saída.** Ao registrar artefatos de um run, o
   OpenDesk percorre o diretório de saída e registra arquivos que o diff do
   OpenCode não reportou (por exemplo, criados por `cp` ou scripts).
   - Compara tamanho e SHA-256 com o último artefato do mesmo caminho.
   - Arquivo novo vira `CREATED`; alterado vira `MODIFIED` com
     `previous_sha256`; inalterado é ignorado.
   - Limite de 200 arquivos por run e profundidade 8.
4. **Contexto único de canal.** `chat::channel_context` centraliza workspace,
   thread e ferramentas para Telegram, Signal e desktop.

## Consequências

- Pedidos de arquivo deixam de cair no fallback e o agente consegue entregar o
  documento no Telegram/Signal.
- Arquivos criados por shell passam a alimentar proveniência, watchers e sinks.
- O desktop ainda não renderiza o anexo; o caminho continua disponível na
  resposta e o envio externo é o foco desta entrega.
