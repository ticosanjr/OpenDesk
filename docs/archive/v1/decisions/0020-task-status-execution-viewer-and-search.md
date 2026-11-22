# ADR 0020 — Status Paperclip, execução conversacional e visualizadores

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o Kanban misturava fase e status em `column_stage`, o worker
  disparava fallback de modelo e auditoria automática, não havia mensagens do
  usuário por tentativa e os artefatos só tinham prévia textual. Também faltava
  busca global, tooltips e um piso de legibilidade para fontes de 8–10px.

## Decisão

### Tarefa e execução
- `tasks.status` é canônico e usa os estados Paperclip: `BACKLOG`, `TODO`,
  `IN_PROGRESS`, `IN_REVIEW`, `DONE`, `CANCELLED`, `BLOCKED`.
- `tasks.phase` separa o workflow (`plan`, `build`, `audit`, `follow_up`).
- `last_run_id`, `last_run_status`, `last_run_at`, `blocked_reason` e
  `cancelled_at` mantêm o último resultado visível no card.
- Cada `task_runs` é uma tentativa com `attempt_number` e `session_id`.
- Build bem-sucedido vai direto para `DONE`; falha vai para `BLOCKED`;
  cancelamento vai para `CANCELLED`.
- Não existe fallback automático de modelo, auditoria automática nem retry
  automático. Interrupções por restart viram `FAILED`/`BLOCKED` e exigem ação
  manual.
- Ações explícitas: Cancelar, Bloquear, Em espera, Reabrir, Concluir, Executar.
- Movimento no Kanban é só drag-and-drop; o dropdown “mover” foi removido.

### Execução conversacional
- `task_messages` guarda mensagens do usuário por tarefa/tentativa.
- `RunTimeline.events` preserva a ordem real: passo, pensamento, ferramenta,
  comentário, resultado.
- `RunLogView` mostra a conversa intercalada por padrão, com modo “faixas” e
  “bruto”.
- O rodapé da execução permite responder, comentar, escolher agente/modelo e
  continuar a execução como uma nova tentativa.
- `POST /api/tasks/{id}/messages` grava a mensagem e, quando solicitado,
  enfileira uma continuação usando o mesmo contexto e a conversa.

### Visualizadores
- `FileViewer` compartilhado entre Arquivos e Artefatos:
  - Markdown renderizado;
  - HTML em iframe `sandbox` sem scripts;
  - imagens com zoom;
  - PDF em iframe;
  - vídeo e áudio nativos;
  - código/JSON/CSV como texto;
  - binários com download.
- Endpoints `/api/files/raw` e `/api/artifacts/{id}/raw` servem conteúdo inline
  com MIME, autenticação e limite de 250 MB.

### Busca e UI
- `/search` com abas Todos, Tarefas, Comentários, Arquivos, Artefatos, Agentes,
  Projetos, Rotinas e Execuções.
- `/api/search` pesquisa SQLite + conteúdo textual do workspace.
- Piso tipográfico global: 8–10px viram 10–12px; UI usa mais peso e ícones.
- `Tooltip` acessível para status e controles.
- `WorkingIndicator` com engrenagens animadas (respeita `prefers-reduced-motion`).
- Colunas do Kanban têm scroll interno; logs, artefatos, Studio e modais usam
  `min-h-0` + `overflow` consistente.
- Studio ganhou detalhes expansíveis e catálogo curado de repositórios
  (Anthropic Skills, Superpowers, Playwright MCP, Context7, MCP servers).

## Consequências

- O status da tarefa reflete a última execução e não fica preso em fase.
- Uma falha nunca dispara uma cascata invisível de retries.
- O log vira uma conversa auditável com a participação do usuário.
- Arquivos e artefatos podem ser inspecionados sem sair do app.
- Busca e tipografia tornam o produto utilizável em sessões longas.
