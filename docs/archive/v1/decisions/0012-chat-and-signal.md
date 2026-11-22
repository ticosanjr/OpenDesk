# ADR 0012 — Chat interativo e bridge Signal

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** a página “Chat & Criador” apenas criava uma tarefa e a enviava
  para a fila. Não era possível conversar com um agente, escolher agente/modelo
  nem continuar uma conversa. O usuário também quer falar com o OpenDesk pelo
  celular via Signal.

## Decisão

### Chat

- Threads e mensagens ficam no SQLite (`chat_threads`, `chat_messages`), com
  agente, modelo, sessão do OpenCode, origem (`desktop`/`signal`) e id externo.
- Cada thread usa uma sessão OpenCode persistente; o primeiro envio cria a
  sessão e os seguintes reutilizam.
- `POST /api/chat/threads/{id}/messages` envia o prompt, aguarda a sessão e
  devolve a resposta + uso (tokens/custo).
- Antes de cada mensagem o budget guard é consultado.
- A UI permite escolher qualquer agente, modelo opcional, histórico, apagar e
  continuar conversas.

### Signal

- O OpenDesk usa `signal-cli` como dispositivo vinculado.
- Configuração: conta, caminho do CLI, allowlist obrigatória, workspace e agente
  padrão.
- `signal-cli link -n OpenDesk` gera um URI `sgnl://` exibido como QR code.
- O bridge inicia `signal-cli -a <conta> --output json daemon`, lê envelopes,
  valida a allowlist, encontra/cria uma thread `source=signal` e responde pelo
  mesmo número.
- Desabilitado por padrão; allowlist vazia impede o start.

## Consequências

- Chat não passa pela fila, mas consome budget e é registrado localmente.
- O histórico local é um espelho; a fonte de verdade da execução é a sessão do
  OpenCode.
- Signal exige vínculo manual pelo celular e o `signal-cli` instalado.
- Mensagens de números fora da allowlist são descartadas e registradas no log.
