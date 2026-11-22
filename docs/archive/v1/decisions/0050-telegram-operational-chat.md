# 0050 — Chat operacional e HITL de tarefas no Telegram

- **Status:** aceito
- **Data:** 2026-11-14
- **Contexto:** o chat já conversava com agentes, mas não garantia que o agente
  soubesse operar o OpenDesk pelo celular. Tarefas criadas com `ask_user`
  ficavam esperando uma resposta apenas no Attention Card/Kanban; o Telegram
  recebia permissões como texto e não tinha botões nem indicador de digitação.

## Decisão

1. Injetar um **contexto operacional** no prompt do OpenCode para mensagens do
   Telegram: o agente deve usar `opendesk-control` para tarefas, execuções,
   rotinas, receitas, fila, atenção e artefatos, e `ask_user` para decisões
   humanas. O contexto não é persistido como mensagem visível.
2. Adicionar a ferramenta MCP `artifact_read`, que lê o conteúdo textual de um
   artefato via `/api/artifacts/{id}/content` com limite de bytes. O caminho
   continua confinado ao workspace e binários retornam apenas metadados.
3. Enviar `sendChatAction(action="typing")` a cada 4 s enquanto o agente prepara
   a resposta, com `TypingGuard` cancelado no fim.
4. Responder à mensagem original usando `reply_parameters.message_id`.
5. Reutilizar a thread externa em qualquer workspace quando o workspace não
   estiver fixo; se a sessão do OpenCode sumir (404), recriar a sessão e repetir
   o prompt uma vez.
6. Publicar `ChatUpdated { thread_id, workspace_id }` após mensagens e mudanças
   de thread. O ChatPage recarrega a conversa selecionada e a lista ao vivo.
7. Notificar `task_interactions` no Telegram com pergunta, contexto, código
   curto e **botões inline** para as opções. O callback resolve a interação e
   retoma a tarefa; texto `#<código> <opção|texto>` é o fallback.
8. Aplicar o mesmo padrão às permissões do OpenCode: botões `Aprovar`/`Negar`
   (`pm:<request_id>:<once|reject>`) e fallback `APROVAR/NEGAR <prefixo>`.
9. `callback_data` nunca contém segredo; a allowlist é validada no callback.

## Consequências

- O bot precisa que o OpenDesk esteja rodando e que a allowlist tenha um chat
  conhecido.
- O agente ainda depende do MCP estar registrado no workspace (já é o padrão) e
  de um agente que aceite as ferramentas.
- O contexto extra aumenta um pouco o prompt por mensagem, mas evita que
  agentes genéricos ignorem as ferramentas do OpenDesk.
- O indicador “digitando” pode permanecer enquanto o agente aguarda uma decisão
  HITL; ele para quando a resposta ou o erro chega.
- O pacote base `controle-opendesk` foi atualizado para 1.2.0; workspaces
  antigos precisam de **Pacotes → Atualizar todos** para receber o prompt/skill
  revisados.
