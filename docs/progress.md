# Histórico de progresso

## 2026-11-06 — Fundação (Fase 1)

### Entregue

- Scaffold Tauri v2 + React 19 + TypeScript + Vite 8 + Tailwind CSS 4.
- Migração SQLite completa e corrigida em relação à spec:
  ordem de foreign keys, índices, `app_settings`, `integration_deliveries`,
  `config_change_log`, `permission_audit` e colunas de fase/telemetria.
- Repositórios de workspaces e settings; seed de budget; workspace ativo em
  `app_settings`.
- API local Axum com token Bearer, CORS restrito, `/ws` e eventos tipados.
- Daemon manager OpenCode V2:
  - spawn de `opencode serve --hostname 127.0.0.1 --port 3100` (ou próxima
    porta livre);
  - captura da senha efêmera no stdout/stderr;
  - Basic auth;
  - healthcheck de 5 s;
  - restart e reattach por `daemon.json` 0600;
  - shutdown do filho no exit do app.
- Bandeja do sistema com status, fila, orçamento, abrir dashboard, reiniciar
  daemon e sair.
- Frontend:
  - shell com sidebar, topbar, workspace switcher, orçamento, fila, status do
    daemon e command bar (Cmd/Ctrl+K);
  - dashboard com KPIs, agentes reais via `/api/agent`, engine e workspace;
  - 9 views roteadas com estados de fundação para Kanban, Chat, Artefatos,
    Arquivos, Rotinas, Studio, Roteador e Integrações;
  - Zustand com snapshot REST + WebSocket com reconexão e `ResyncRequired`.
- Criação de workspace cria `input/`, `output/` e `.opencode/`; o último é
  necessário para o OpenCode reconhecer o location e retornar agentes/modelos
  scoped.

### Verificado

- `cargo check` — passou (warnings de dead code aceitos para APIs de fases
  futuras).
- `npm run check` — passou.
- `npm run build` — passou.
- App iniciado com `npm run tauri dev`.
- Daemon subiu em `127.0.0.1:3101` porque a 3100 estava ocupada pelo harness
  Paperclip; `/api/health` reportou `online: true`, versão `2.2.5`.
- `GET /api/daemon/status` sem token retornou `401`.
- Criação de workspace via API criou `input/`, `output/`; depois validei que
  `.opencode/` é necessário para location-scoped agents/models.
- `GET /api/agents` e `/api/models` retornaram dados quando o workspace tinha
  `.opencode/` (75 modelos, 7 agentes built-in no location).
- `GET /api/dashboard` retornou zeros para workspace novo.
- `GET /api/settings/active_workspace_id` persistiu o workspace ativo.

### Observações de integração

- `/api/agent`, `/api/model` e `/api/config` usam
  `?location[directory]=<path>` (deepObject). Sem um location reconhecido,
  retornam `LocationNotFoundError` ou lista vazia.
- `GET /api/config` retorna a lista de documentos/diretórios carregados, não um
  objeto único.
- A senha do daemon é efêmera e aparece no stdout como
  `server password <valor>`; deve ser removida de qualquer log.
- O harness Paperclip ocupa a porta 3100; o fallback para 3101 é essencial.

### Próximo

1. Fase 2 — Studio SSOT:
   - `jsonc_editor` com CST + escrita atômica + hash de concorrência;
   - parsers/serializers de agentes, skills, comandos e MCP V2;
   - watcher `notify` com debounce;
   - UI do Studio e testes de paridade <500 ms.
2. Adicionar testes de integração Rust para a criação de workspace e layout.
3. Revalidar o app com a criação automática de `.opencode/`.
