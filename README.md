# OpenDesk

Sistema operacional visual de governança, automação e observabilidade sobre o
OpenCode. Desktop nativo em Rust/Tauri v2 + React, com toda a configuração do
OpenCode permanecendo nos arquivos nativos do disco.

> **Status:** fases 1–11 prontas. O OpenDesk cobre fundação, Studio com gestão
> completa (efetivo, copiar/mover, toggle, backup e instalação), execução,
> automação, Google Workspace + MCP bridge, resiliência/atualizações, tarefas,
> chat interativo com Signal e runtime inventory do OpenCode no Studio. Repositório privado:
> <https://github.com/ticosanjr/OpenDesk>. Consulte
> [docs/implementation-plan.md](docs/implementation-plan.md) e
> [docs/progress.md](docs/progress.md).

## O que já funciona

- App Tauri v2 com janela, bandeja do sistema e instância única.
- SQLite em WAL em `~/.local/share/opendesk/opendesk.db` com migração inicial.
- API local Axum em `127.0.0.1:<porta dinâmica>` com token Bearer e WebSocket
  `/ws`.
- Gerenciador do daemon OpenCode V2: sobe `opencode serve` em 3100 (ou próxima
  porta livre), captura a senha efêmera, autentica com HTTP Basic, faz
  healthcheck a cada 5 s e reinicia sob demanda.
- Registro de workspaces com criação de `input/`, `output/` e `.opencode/`.
- Configuração da pasta padrão de workspaces e cadastro de pastas existentes em
  qualquer local do disco, com folder picker nativo.
- OpenCode Studio: CRUD visual de agentes, skills, comandos, MCP e JSONC bruto,
  com escrita atômica, backup, hash de concorrência e preservação de
  comentários.
- Watcher bidirecional com debounce de 150 ms: editar um `.md` por fora atualiza
  a UI via WebSocket.
- Agentes customizáveis com nome de exibição, avatar (emoji/2 caracteres) e cor.
- Roteador DSH com políticas de custo/qualidade, regras IF-THEN, resolução de
  modelo e Context Packer com a regra de output.
- Fila SQLite com prioridade, pausa, retry, cancelamento, recuperação de crash e
  worker Tokio.
- Budget Guard diário/mensal/5h com hard cap que pausa a fila.
- Execução real no OpenCode V2 via sessão/prompt/SSE, com streaming de logs,
  tokens/custo, interrupção e broker de permissões auditado.
- Proveniência de artefatos com diff autoritativo e fallback por tool calls.
- Kanban com 5 colunas, aprovação Plan → Build, logs por run e controlador da
  fila; Chat com tarefa rápida e Meta-Builder de agentes/skills/comandos.
- Rotinas com recorrência intuitiva (calendário, dias da semana, mensal e cron
  custom), timezone local editável, prévia dos próximos disparos, edição,
  pasta determinística, idempotência de slot e execução manual.
- Notificações por e-mail e Signal, com opção de só avisar quando o artefato
  principal mudar.
- SMTP por workspace com segredo criptografado (AES-256-GCM, 0600), teste de
  conexão e entrega pós-run com anexos e idempotência.
- Motor OpenCode resiliente: descoberta do binário fora do `PATH`,
  fallback para o serviço compartilhado, mensagens acionáveis e preflight.
- Atualização do OpenCode V2 pela UI (checagem semver, upgrade e reinício).
- Kanban com transições que iniciam Plan/Build/Audit e erro acionável por tarefa.
- Edição de tarefa já executada e reexecução sem recriar o card.
- Agentes com slug automático, 20 presets de avatar e 8 presets de cor.
- Dashboard com atividades clicáveis e status colorido.
- Sobre & diagnóstico com modos simples/avançado e exportação JSON.
- Chat interativo com qualquer agente, histórico, streaming por sessão e custo.
- Signal: vínculo por QR (`signal-cli link`), allowlist e bridge de mensagens.
- Studio efetivo (global + workspace), copiar/mover/duplicar, toggle, backups e
  instalação de GitHub/URL/local/npm/catálogo.
- Explorador de arquivos do workspace com árvore, busca, breadcrumb e prévia.
- Toasts, confirmação consistente, boundary de erro e notificações nativas.
- Workspace editável/removível sem tocar na pasta do disco.
- Kanban horizontal com busca/filtros e formulário único de tarefa.
- Command palette com tarefas, rotinas, artefatos e agentes.
- Página de Configurações/Integrações com navegação por seções.
- Rotas em code splitting e logs de run em atualização contínua.
- Página Execuções com timeline estruturada: raciocínio e ferramentas
  colapsados, resultado aberto, durações e timestamps.
- Notificações e referências clicáveis com deep links para runs, tarefas,
  artefatos, rotinas, agentes, arquivos e configurações.
- Kanban com sete status Paperclip, último run no card, drag-and-drop e ações de
  cancelar/bloquear/espera/reabrir.
- Execução conversacional com pensamento, ferramentas, comentários,
  respostas do usuário e resultado intercalados.
- Visualizador interno para Markdown, HTML, imagens, PDF, vídeo, áudio e código.
- Busca global com abas por tipologia.
- Contrato de scroll com auditoria Playwright em todas as rotas e tamanhos.
- Versão do OpenDesk visível no menu da bandeja e no tooltip.
- Modelos sem variante (`provider/modelo`) validados e aceitos em todas as
  chamadas, sem `variant: null`.
- Notificações deduplicadas: tarefa só no status terminal, rotina só quando o
  run muda e um aviso por run para artefatos.
- Backup rotativo do SQLite antes de migrações.
- Limpeza automática de artefatos de build após cada release.
- Auto-atualização do OpenDesk por release privada com SHA256 e backup.
- Google Workspace: OAuth2 loopback + PKCE, Docs, Drive, Calendar, Gmail e
  Tasks; export pós-run para Google Docs/Drive.
- MCP bridge `opendesk mcp-google` com ferramentas de Gmail, Docs, Drive,
  Calendar e Tasks, registrado em `mcp.servers`.
- Dashboard executivo, shell de navegação com 10 views, command bar
  (Cmd/Ctrl+K), seletor de workspace e indicadores de daemon, fila e orçamento.
- Endpoints de leitura de agentes e modelos direto do OpenCode V2
  (`/api/agent`, `/api/model`), com escopo por workspace.
- Modo headless (`opendesk --headless`) para testes de navegador e instalações
  sem janela.
- Releases locais em `releases/` (máximo 3 versões), sem CI.

## Requisitos

- Rust 1.85+ (edition 2024)
- Node.js 20+ e npm
- OpenCode V2 instalado e no `PATH` (`opencode --version` → 2.x)
- Linux: `webkit2gtk-4.1`, `gtk+-3.0`, `libsoup-3.0`,
  `libayatana-appindicator3-dev` e ferramentas de build do Tauri
- Windows: WebView2
- macOS: será validado em uma fase futura
- Playwright (apenas para e2e): `npx playwright install chromium`

## Rodar

```bash
npm install
npm run tauri dev
```

No primeiro boot o OpenDesk:

1. cria `~/.local/share/opendesk/` e o banco SQLite;
2. sobe um daemon OpenCode dedicado em `127.0.0.1:3100` (ou 3101–3199 se a
   porta estiver ocupada);
3. grava `daemon.json` e `api.json` com permissão `0600` (senha efêmera e token
   local);
4. abre a janela e conecta o WebSocket.

> O OpenCode V2 exige autenticação Basic (`opencode:<senha>`). A senha é
> capturada do stdout do processo filho e nunca é escrita em logs.

## Verificações

```bash
npm run check          # TypeScript
npm run build          # frontend
npm run test:rust      # 96 testes Rust
npm run test:ui        # Vitest + cobertura da lógica frontend
npm run test:e2e       # 10 testes Playwright (headless + Vite)
scripts/gate.sh        # gate local antes de release
npm run clean          # remove build/cobertura após release
npm run clean:deep     # remove também node_modules
cd src-tauri && cargo check
cd src-tauri && cargo test
scripts/release.sh     # build + release local + retenção de 3 versões
```

Para inspecionar a API local durante o desenvolvimento:

```bash
PORT=$(jq -r .port ~/.local/share/opendesk/api.json)
TOKEN=$(jq -r .token ~/.local/share/opendesk/api.json)
curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:$PORT/api/health" | jq
```

## Estrutura

```text
src-tauri/
  migrations/0001_initial_schema.sql
  src/
    main.rs / lib.rs        entrada Tauri, setup, shutdown
    app_state.rs            estado compartilhado (DB, daemon, eventos, API)
    api/                    Axum REST + WebSocket + comandos Tauri
    db/                     SQLite, repositórios e modelos
    events.rs               contrato de eventos em tempo real
    opencode_client/        daemon manager + cliente REST V2
    paths.rs                diretórios de app e layout de workspace
    tray.rs                 menu da bandeja e status dinâmico
src/
  components/               layout, UI primitives
  pages/                    9 views do produto
  store/                    Zustand + WebSocket
  lib/                      API client, tipos e formatação
docs/                       plano, decisões e histórico
```

## Princípios

1. **SSOT no disco:** agentes, skills, comandos, MCP e `opencode.jsonc` vivem
   apenas nos arquivos nativos do OpenCode. O SQLite guarda estado operacional.
2. **Loop delegado:** o OpenDesk nunca implementa execução de ferramentas; ele
   envia sessões/prompts ao daemon V2 e consome eventos SSE.
3. **Roteamento modular:** Plan → Build → Audit, com política de custo,
   variantes de modelo e contexto empacotado por etapa.
4. **Entrega determinística:** `output/YYYY-MM/<task-slug>/` e proveniência de
   cada artefato.
5. **V2-only:** sem camada de compatibilidade V1.

## Limitações atuais

- O OAuth Google exige que o usuário crie um OAuth client (Desktop app) no
  Google Cloud e cadastre o callback `http://127.0.0.1:53682/callback`.
- As APIs Google não foram testadas com uma conta real neste ambiente; o fluxo
  de autorização, o store de segredos e o bridge MCP foram validados.
- Google Keep continua indisponível para contas comuns; o backlog usa Tasks.
- A fila executa até 4 jobs simultâneos conforme configuração do Kanban.
- O broker de permissões de runs autônomos responde `once` e audita em
  `permission_audit`; políticas por workspace ainda não têm UI.
- O Playwright usa o backend headless com `OPENDESK_SKIP_DAEMON=1`; o teste live
  com `deepseek/deepseek-flash` foi feito manualmente e gerou custo real de
  ~US$ 0,0023.
- Releases são construídas localmente; não há CI.
- O frontend em modo browser puro exige `VITE_OPENDESK_PORT` e
  `VITE_OPENDESK_TOKEN`; o caminho normal é `npm run tauri dev`.
- O daemon OpenCode é encerrado no shutdown do app, mas sobrevive a um crash
  para permitir reattach no próximo boot (lock file `daemon.json`).

## Documentação

- [Plano de implementação](docs/implementation-plan.md)
- [ADR 0001 — OpenCode V2 only](docs/decisions/0001-opencode-v2-only.md)
- [ADR 0002 — Metadados de apresentação de agentes](docs/decisions/0002-agent-presentation-metadata.md)
- [ADR 0003 — Permissões em runs autônomos](docs/decisions/0003-unattended-permissions.md)
- [ADR 0004 — Proveniência com fallback por tool calls](docs/decisions/0004-artifact-diff-fallback.md)
- [ADR 0005 — Secret store local criptografado](docs/decisions/0005-local-secret-store.md)
- [ADR 0006 — Cron dirigido pelo SQLite](docs/decisions/0006-cron-db-tick.md)
- [ADR 0007 — OAuth Google loopback e credenciais](docs/decisions/0007-google-oauth-loopback.md)
- [ADR 0008 — MCP bridge stdio](docs/decisions/0008-mcp-bridge-stdio.md)
- [ADR 0009 — Resolução e fallback do motor](docs/decisions/0009-opencode-engine-resolution.md)
- [ADR 0010 — Atualizações](docs/decisions/0010-updates.md)
- [ADR 0011 — Workflow e erros](docs/decisions/0011-task-workflow-and-errors.md)
- [ADR 0012 — Chat e Signal](docs/decisions/0012-chat-and-signal.md)
- [ADR 0013 — Gestão do Studio](docs/decisions/0013-studio-management.md)
- [ADR 0014 — Overrides e saúde de modelos](docs/decisions/0014-model-overrides-and-health.md)
- [ADR 0015 — Instalação com retry e rollback](docs/decisions/0015-install-retry-rollback.md)
- [ADR 0016 — Runtime inventory e streaming](docs/decisions/0016-runtime-inventory-and-streaming.md)
- [ADR 0017 — Recorrência intuitiva e notificações](docs/decisions/0017-intuitive-recurrence-and-notifications.md)
- [ADR 0018 — Arquivos, feedback e gestão de workspace](docs/decisions/0018-files-feedback-and-workspace-management.md)
- [ADR 0019 — Timeline de execução e referências navegáveis](docs/decisions/0019-run-timeline-and-entity-links.md)
- [ADR 0020 — Status Paperclip, execução e visualizadores](docs/decisions/0020-task-status-execution-viewer-and-search.md)
- [ADR 0021 — Contrato de layout e scroll](docs/decisions/0021-layout-and-scroll-contract.md)
- [ADR 0022 — Model refs e notificações](docs/decisions/0022-model-refs-and-notification-policy.md)
- [Releases locais](releases/README.md)
- [Histórico de progresso](docs/progress.md)
- [Especificação original](OpenDesk%20Requisitos.txt)
