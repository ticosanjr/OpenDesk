# OpenDesk

Sistema operacional visual de governança, automação e observabilidade sobre o
OpenCode. Desktop nativo em Rust/Tauri v2 + React, com toda a configuração do
OpenCode permanecendo nos arquivos nativos do disco.

> **Status:** fundação (Fase 1) e Studio SSOT (Fase 2) prontos. A próxima
> entrega é o núcleo de execução (Fase 3). Consulte
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
npm run test:rust      # testes Rust
npm run test:e2e       # Playwright (headless + Vite)
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

- Fila, Kanban, cron, artefatos e integrações ainda são telas de fundação com
  suas fases indicadas na UI. O Studio já opera no disco.
- O Playwright usa o backend headless com `OPENDESK_SKIP_DAEMON=1`; testes de
  execução real do OpenCode ficam para a Fase 3.
- Releases são construídas localmente; não há CI.
- O frontend em modo browser puro exige `VITE_OPENDESK_PORT` e
  `VITE_OPENDESK_TOKEN`; o caminho normal é `npm run tauri dev`.
- O daemon OpenCode é encerrado no shutdown do app, mas sobrevive a um crash
  para permitir reattach no próximo boot (lock file `daemon.json`).

## Documentação

- [Plano de implementação](docs/implementation-plan.md)
- [ADR 0001 — OpenCode V2 only](docs/decisions/0001-opencode-v2-only.md)
- [ADR 0002 — Metadados de apresentação de agentes](docs/decisions/0002-agent-presentation-metadata.md)
- [Releases locais](releases/README.md)
- [Histórico de progresso](docs/progress.md)
- [Especificação original](OpenDesk%20Requisitos.txt)
