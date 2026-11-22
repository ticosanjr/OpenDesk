# 0054 — Arquivos no Telegram, anexos em sinks e watchers de conclusão

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** o Telegram já operava o chat, HITL e permissões, mas só aceitava
  texto. Sinks de mensageria enviavam apenas o resumo, e tarefas só avisavam por
  e-mail. Faltava levar o resultado real (arquivos) para onde o usuário está.

## Decisão

1. **Entrada de arquivos pelo Telegram.** Mensagens com `document`, `photo`,
   `audio`, `voice` ou `video` são baixadas via `getFile` e salvas em
   `<workspace>/input/telegram/YYYY-MM-DD/`. O caminho relativo entra no prompt
   do chat, de modo que o agente pode analisar o arquivo imediatamente.
   - Limite de 20 MB imposto pelo Bot API; nomes são sanitizados e não escapam
     do workspace.
   - A legenda vira o pedido; sem legenda, o agente confirma o recebimento.
2. **Saída de arquivos.** `TELEGRAM_MSG` ganha `attach_artifacts` (padrão
   ligado) e envia até cinco artefatos com `sendDocument`. A entrega de rotinas
   no Telegram também anexa o artefato principal. O Signal já recebia anexos.
3. **Watchers de conclusão.** Uma tarefa pode assinar Telegram e/ou Signal.
   Cada execução terminal (sucesso, falha ou cancelamento) envia status,
   resumo/erro e o artefato principal. O estado fica em `task_watchers`
   (migração 0038) e o envio é deduplicado por execução na tabela
   `integration_deliveries`.
4. **API e UI.** `GET/PUT /api/tasks/{id}/watchers` substitui o conjunto de
   canais de forma idempotente. O formulário de tarefa mostra “Avisar quando
   concluir”; o editor de receitas mostra chat, segredo e anexos do Telegram.

## Consequências

- O usuário pode mandar um PDF pelo Telegram e pedir a análise no mesmo fio.
- Entregas deixam de ser apenas texto: o artefato chega no canal escolhido.
- Avisos de conclusão são explícitos por tarefa, não globais, e sobrevivem a
  reinícios porque vivem no SQLite.
- O limite de 20 MB é uma restrição do Bot API, não do OpenDesk; arquivos
  maiores continuam disponíveis no workspace e são citados no aviso.
