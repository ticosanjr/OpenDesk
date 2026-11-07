# AGENTS.md — instruções para agentes que trabalham no OpenDesk

## Identidade do projeto

OpenDesk é um desktop app Tauri v2 (Rust) + React que governa o OpenCode V2.
O documento mestre é `OpenDesk Requisitos.txt`, mas ele foi escrito para o
OpenCode V1. **A implementação é V2-only** (ver `docs/decisions/0001-opencode-v2-only.md`).

## Regras invioláveis

1. **SSOT no disco.** Configuração do OpenCode (workspaces, `opencode.jsonc`,
   `.opencode/agents/*.md`, `.opencode/skills/*/SKILL.md`,
   `.opencode/commands/*.md`, `mcp.servers`) é lida e escrita diretamente nos
   arquivos nativos. O SQLite só guarda estado operacional (fila, runs,
   proveniência, telemetria, credenciais).
2. **Loop delegado.** Nunca implemente execução de ferramentas. Sessões e
   prompts vão para o daemon OpenCode via `/api/session`, `/api/session/:id/prompt`
   e eventos SSE em `/api/event`.
3. **OpenCode V2.** Use `/api/*`, Basic auth `opencode:<senha>`, `permissions`,
   `commands`, `mcp.servers`, `disabled`, `type: "remote"`. Não use `/v1/*`,
   `permission` objeto, `command` singular, `type: "sse"` nem `enabled`.
4. **Entrega determinística.** Todo Build/rotina cria e usa
   `<workspace>/output/YYYY-MM/<slug>/`.
5. **Segredos.** Senha do daemon e token da API local ficam em arquivos `0600`
   ou no keyring. Nunca logue senhas, tokens, prompts completos ou conteúdo de
   `.env`.

## Comandos

```bash
npm install
npm run tauri dev      # app completo
npm run check          # TypeScript
npm run build          # frontend
npm run test:rust      # testes Rust
npm run test:e2e       # Playwright Chromium (npx playwright install chromium)
cd src-tauri && cargo check
cd src-tauri && cargo test
scripts/release.sh     # build local + retenção de 3 releases (sem CI)
```

## Onde está o quê

- `src-tauri/src/api/` — rotas Axum, `/ws` e comandos Tauri.
- `src-tauri/src/opencode_client/` — daemon manager e cliente REST V2.
- `src-tauri/src/db/` — migrações, modelos e repositórios.
- `src-tauri/src/events.rs` — contrato dos eventos em tempo real.
- `src-tauri/src/paths.rs` — diretórios do app e layout de workspace.
- `src/pages/` — as 9 views.
- `src/store/app-store.ts` — snapshot + WebSocket + ações.
- `docs/` — plano, ADRs e histórico.

## Convenções

- Rust edition 2024; erros com `AppError`/`thiserror`; nunca `unwrap` em código
  de produção.
- TypeScript estrito, sem `any` novo; componentes em `src/components`.
- Eventos WS sempre `{ event_id, event_type, payload }`; ao perder eventos,
  emitir `ResyncRequired` e refazer o snapshot.
- Toda mudança de comportamento atualiza `docs/progress.md`.
- Toda decisão arquitetural nova vira um ADR em `docs/decisions/`.
- Rode `cargo fmt` antes de finalizar.

## Estado atual

Fases 1–16 concluídas: fundação, Studio SSOT com gestão completa (efetivo,
copiar/mover/duplicar, toggle, backups e instalação), workspaces configuráveis,
agentes com nome/avatar/cor, roteador DSH, fila, budget guard, execução OpenCode
V2 via SSE, proveniência, Kanban com transições, erros acionáveis, edição de
tarefas, chat interativo com qualquer agente, bridge Signal, Meta-Builder, cron,
pastas determinísticas, SMTP, entregas pós-run, Google Workspace, MCP bridge,
resolução resiliente do motor OpenCode, fallback para serviço compartilhado,
atualização do OpenCode, auto-atualização do OpenDesk, concorrência da fila,
Sobre/diagnóstico, runtime inventory do Studio, SSE sem timeout total, daemons
órfãos, verificação real de tarefas, rotinas com recorrência intuitiva,
timezone local, edição, notificações Signal e aviso só por novidade, explorador
de arquivos, toasts/confirmação global, notificações nativas, gestão de
workspace, formulários compartilhados de agente/modelo/prompt, command palette
global, histórico de rotinas, code splitting e backup do SQLite, timeline
estruturada de execução, página de execuções, links/deep links universais,
notificações clicáveis, status Paperclip, execução conversacional, visualizador
de arquivos/artefatos, busca global, tipografia/tooltips, catálogo curado do
Studio, versão no tray e contrato de scroll com auditoria E2E.
Próxima frente: retry durável de Google/Drive, assinatura de updates, cobertura
Rust ≥80%, Windows/macOS e testes de contrato Google/OpenCode.

Releases são locais (`releases/`, máximo 3 versões) e nunca via CI. O remoto é
`origin` em https://github.com/ticosanjr/OpenDesk (privado); `scripts/release.sh`
faz commit, tag e push. `name` e `avatar` de agentes são metadados do OpenDesk no
frontmatter; `color` é nativo (ADR 0002).
