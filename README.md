# OpenDesk

Sistema operacional visual de governança, automação e observabilidade sobre o
OpenCode. Desktop nativo em Rust/Tauri v2 + React, com toda a configuração do
OpenCode permanecendo nos arquivos nativos do disco.

> **Status:** release 1.4.1, fundação V2 (Ondas 1 e consolidação do storage concluídas; Ondas 2–4 em andamento). O desktop
> continua sendo a interface principal; `opendeskd`, PWA e CLI operam o mesmo
> estado sem duplicar regra. Produto e requisitos em [SPEC.md](SPEC.md),
> arquitetura em [ARCHITECTURE.md](ARCHITECTURE.md); histórico V1 congelado em
> [docs/archive/v1](docs/archive/v1).
> Repositório: <https://github.com/ticosanjr/OpenDesk>.

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
- Notificações por e-mail, Signal e Telegram, com opção de só avisar quando o
  artefato principal mudar.
- SMTP por workspace com segredo criptografado (AES-256-GCM, 0600), teste de
  conexão e entrega pós-run com anexos e idempotência.
- Motor OpenCode resiliente: descoberta do binário fora do `PATH`,
  fallback para o serviço compartilhado, mensagens acionáveis e preflight.
- Atualização do OpenCode V2 pela UI (checagem semver, upgrade e reinício).
- Kanban com transições que iniciam Plan/Build/Audit e erro acionável por tarefa.
- Edição de tarefa já executada e reexecução sem recriar o card.
- Agentes com slug automático, 20 presets de avatar e 8 presets de cor.
- Dashboard com atividades clicáveis, status colorido e rodapé discreto de
  integrações ligadas.
- Sobre & diagnóstico com modos simples/avançado, exportação JSON e reparo do
  atalho do AppImage no menu/barra de tarefas.
- Configurações → Prompt mostra o prompt completo enviado a qualquer tarefa,
  seção por seção e com a origem de cada bloco.
- Telegram recebe arquivos no workspace e sinks entregam artefatos; tarefas
  avisam conclusões no Telegram/Signal.
- HITL 2.0 com aprovação, escolha única, múltipla escolha e texto livre; o
  agente escolhe a estrutura e toda decisão aceita payload opcional.
- Modal global de decisão em qualquer página, além de Telegram, Signal e
  notificação nativa.
- Tela única de execução com agente, modelo, comando/skill, status ao vivo e
  parada no mesmo modal.
- Chat e Telegram podem enviar arquivos do workspace; arquivos criados por
  shell também entram na proveniência.
- Turnos de chat com ferramentas/arquivos aparecem como execuções e em
  Artefatos, com canal de origem; o chat mostra “pensando”, permite parar e não
  é cortado em cinco minutos.
- Observabilidade por origem (trabalho, rotina, receita, automação,
  Telegram/Signal/Desktop) e templates editáveis de prompt.
- Configurações → Prompt com três abas e previews Seções/OpenDesk/OpenCode/
  Final reconstruído; `learning_save` e `personal_context_save` no MCP.
- Prévia HTML com recarga, JSON formatado, contexto pessoal sem título e
  ferramentas externas no Dashboard.
- Chat interativo com qualquer agente, histórico, streaming por sessão e custo;
  pelo Telegram o agente cria tarefas, consulta execuções e resultados, executa
  rotinas/receitas e responde HITL.
- Signal: vínculo por QR (`signal-cli link`), allowlist e bridge de mensagens.
- Telegram: Bot API por long polling, token criptografado e allowlist; chat
  operacional com “digitando…”, atualização ao vivo e botões para HITL e
  permissões.
- Studio efetivo (global + workspace), copiar/mover/duplicar, toggle, backups e
  instalação de GitHub/URL/local/npm/catálogo.
- Explorador de arquivos do workspace com árvore, busca, breadcrumb, prévia e
  abertura de pastas validada pelo workspace.
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
- Governança de agentes com organograma, metas, budgets, quota, performance,
  delegação HITL e heartbeat opt-in.
- Aprendizado contínuo: lições de runs, MEMORY.md e propostas de skill com
  aprovação humana.
- Observabilidade operacional (p95, fila, falhas) e CLI headless
  (`opendesk --headless`).
- Integração com OpenCode Browse para tarefas de navegação web e backup/restore
  completo do workspace em ZIP.
- Contratos opt-in de OpenCode V2 e Google (`npm run contract:opencode`,
  `npm run contract:google`).
- Goldens visuais de **18 rotas + 5 modais** em 700/1024/1100/1440/1920, com
  atualização via `npm run test:visual:update`.
- Builds Windows/macOS com `npm run build:platform` e assinatura local
  documentada em `docs/platform-builds.md`.
- Versão do OpenDesk visível no menu da bandeja e no tooltip.
- Modelos sem variante (`provider/modelo`) validados e aceitos em todas as
  chamadas, sem `variant: null`.
- Notificações deduplicadas: tarefa só no status terminal, rotina só quando o
  run muda e um aviso por run para artefatos.
- Preferências de notificação e opção de fechar para a bandeja.
- Follow-up de tarefa reutiliza a mesma sessão OpenCode.
- Kanban com visão em lista, seleção múltipla e ações em lote.
- Paginação em artefatos/execuções e visualizador carregado sob demanda.
- Botão Executar indica engine offline e ajuda de atalhos com `?`.
- Lease/heartbeat de jobs e Attention Card no Dashboard.
- Cron com misfire policy, modo seguro e dry-run.
- Artefatos fixáveis, MEMORY.md, fork de tarefa e model mix/economia.
- Permissões interativas com Attention Card e estados de agente.
- Compactação de contexto, A/B swarm, fork por run e rollback de artefato.
- Quick Capture global e blueprints de equipe.
- Prompt caching com métrica de cache hit, metadados de mídia via ffprobe.
- Aprovação remota de permissões e HITL de tarefas pelo Signal e Telegram, com
  botões inline, contexto e fallback por texto.
- Janela maximizada, “X” mantendo o app na bandeja e abas em Configurações,
  Integrações e Kanban.
- Timeline de execução ao vivo com links para artefatos e pasta de resultados.
- Barramento universal de I/O: dropzone/OCR/áudio, IMAP, webhooks, SQL poller,
  Telegram/Matrix e sinks de PDF/XLSX, FS, S3, Git, SQL e chat.
- Ferramentas externas opcionais em pasta própria, com detecção e download.
- Receitas YAML com editor visual, HITL por UI/e-mail/Signal/Telegram e sinks.
- Rsync para pastas locais e remotas, com chave SSH e `--delete` opt-in.
- Agentes podem propor agentes, skills, comandos, receitas, rotinas e MCPs;
  toda mudança exige Aprovar/Negar no Attention Card.
- Workspaces padrão em `~/Documents/OpenDesk Workareas`.
- Backup rotativo do SQLite antes de migrações.
- Limpeza automática de artefatos de build após cada release.
- Auto-atualização do OpenDesk no Linux por release privada com assinatura,
  SHA256 e backup; macOS/Windows usam o instalador da release.
- Google Workspace: OAuth2 loopback + PKCE, Docs, Drive, Calendar, Gmail e
  Tasks; export pós-run para Google Docs/Drive.
- MCP bridge `opendesk mcp-google` com ferramentas de Gmail, Docs, Drive,
  Calendar e Tasks, registrado em `mcp.servers`.
- Dashboard executivo, shell de navegação com 18 rotas, command bar
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
npm run test:rust      # 397 testes Rust
npm run test:ui        # Vitest + cobertura da lógica frontend
npm run test:e2e       # 93 testes Playwright (headless + Vite)
npm run verify         # verificação proporcional (só o que mudou)
npm run gate:fast      # checagem ampla sem cobertura/E2E
scripts/gate.sh        # gate completo (cobertura, E2E e goldens) — sob demanda
npm run clean          # limpeza transitória; preserva o cache de build
npm run clean:deep     # remove target e node_modules (build frio depois)
cd src-tauri && cargo check
cd src-tauri && cargo test
scripts/release.sh     # verify + build rápido + release local + retenção de 3 versões
```

O release padrão usa verificação proporcional e perfil rápido (thin LTO,
incremental), preservando `src-tauri/target`. Use `scripts/release.sh
--quality-build` para LTO completo e `--full-gate` para o gate completo.

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
  pages/                    páginas roteadas do produto
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

- [SPEC.md](SPEC.md) — produto, requisitos e HITL
- [ARCHITECTURE.md](ARCHITECTURE.md) — os 8 crates, storage e fluxos
- [DESIGN.md](DESIGN.md) — sistema visual
- [ADR 0200 — Fundação V2](docs/decisions/0200-v2-foundation.md)
- [Auditoria de schema V1 → V2](docs/v2-migration-audit.md)
- [Progresso V2](docs/v2-progress.md)
- [Histórico V1](docs/archive/v1/) — decisões, progresso e releases congelados
- [Contrato OpenCode V2](docs/protocol/opencode-v2.md)
- [Fluxo de release](docs/release-flow.md)
