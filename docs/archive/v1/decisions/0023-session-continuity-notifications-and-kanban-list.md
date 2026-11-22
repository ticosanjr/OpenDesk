# ADR 0023 — Continuidade de sessão, preferências de notificação e Kanban em lista

- **Status:** aceito
- **Data:** 2026-11-07
- **Contexto:** o follow-up de tarefa criava uma nova sessão OpenCode em vez de
  continuar a conversa. Notificações não tinham preferências, o botão Executar
  não indicava engine offline e o Kanban não tinha visão de lista nem ações em
  lote. Listas longas de artefatos e execuções não tinham paginação.

## Decisão

### Execução
- `execution_queue.session_id` permite reutilizar a sessão OpenCode em
  follow-ups.
- `task_runs.session_id` é gravado após a execução e atualizado por
  `runs::set_session`.
- `runs::latest_session_for_task` alimenta a próxima tentativa.
- O runner usa `session_id` existente quando presente; só cria sessão nova se
  não houver.

### Notificações
- Configurações → Notificações:
  - nativas on/off;
  - avisos de artefatos on/off;
  - fechar janela: sair (padrão) ou manter na bandeja.
- `close_to_tray` fica em `app_settings` e é lido no `CloseRequested`.
- Preferências de notificação ficam em `localStorage` e são aplicadas pela
  política central.

### Kanban e listas
- Alternância Board/Lista.
- Tabela com status, agente, último run e prioridade.
- Seleção múltipla + ações em lote.
- Botão Executar desabilitado com tooltip quando o engine está offline.
- Artefatos e execuções têm “carregar mais” (limite crescente).

### Performance
- `LazyFileViewer` carrega o chunk do Markdown/HTML/PDF/vídeo apenas quando um
  arquivo é aberto.
- Atalhos globais documentados com `?`.

## Consequências

- Follow-up mantém a continuidade nativa do OpenCode.
- Notificações podem ser ajustadas sem alterar código.
- O usuário consegue operar o Kanban em lote ou em tabela.
- Listas longas não ficam presas em um limite fixo.
- O visualizador não pesa no carregamento inicial das páginas.
