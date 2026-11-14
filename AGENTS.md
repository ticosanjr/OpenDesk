# AGENTS.md — instruções para agentes que trabalham no OpenDesk

## Identidade do projeto

OpenDesk é um desktop app Tauri v2 (Rust) + React que governa o OpenCode V2.
O documento mestre atual é `Especificacao Complementar OpenDesk.md`. A base
original foi escrita para o OpenCode V1; **a implementação é V2-only** (ver
`docs/decisions/0001-opencode-v2-only.md`).

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
6. **Verificação proporcional.** Sem release, rode `npm run verify` (ou
   `npm run check:changed`): só as áreas tocadas são verificadas
   (fmt/clippy/testes Rust quando houver `.rs`; typecheck/ESLint/Vitest quando
   houver frontend). `npm run gate:fast` cobre tudo sem cobertura/E2E. O gate
   completo (`scripts/gate.sh`, cobertura, 93 Playwright e 49 goldens) roda
   **somente quando o usuário pedir**. Release padrão usa a verificação
   proporcional; `--full-gate` é opt-in.
7. **Cache de build.** `scripts/clean.sh` preserva `src-tauri/target` por padrão
   para o próximo release ser incremental. `--all` remove `target` e `--deep`
   também remove `node_modules`; só use quando o disco exigir.

## Comandos

```bash
npm install
npm run tauri dev      # app completo
npm run check          # TypeScript
npm run lint           # ESLint (hooks + TypeScript)
npm run build          # frontend
npm run test:rust      # testes Rust
npm run test:e2e       # Playwright Chromium (npx playwright install chromium)
npm run test:features  # suíte funcional completa (sob demanda, fora do gate)
npm run test:visual    # goldens de 18 rotas + 5 modais (também no gate)
npm run test:visual:update  # regravar goldens após mudança visual intencional
npm run check:changed  # verificação proporcional (sem release)
npm run verify         # alias de check:changed
npm run gate:fast      # gate rápido: fmt Rust, TS, lint e Vitest
scripts/gate-fast.sh --rust  # inclui clippy e testes Rust
scripts/gate-fast.sh --changed  # equivalente a npm run verify
cd src-tauri && cargo check
cd src-tauri && cargo test
scripts/release.sh     # verify proporcional + build rápido + retenção de 3 releases
scripts/release.sh --full-gate  # gate completo antes do build (sob demanda)
scripts/release.sh --skip-gate  # build já verificado externamente
scripts/release.sh --reuse-build  # reaproveita o binário se nenhum fonte for mais novo
scripts/release.sh --quality-build  # LTO completo e codegen-units=1
scripts/release.sh --no-appimage  # só .deb/.rpm (release mais curto)
scripts/release.sh --refresh-appimage  # refaz o AppImage com linuxdeploy
scripts/release.sh --clean  # limpeza transitória ao final (preserva target)
npm run clean          # limpeza transitória; preserva src-tauri/target
npm run clean:deep     # remove target e node_modules (build frio depois)
```

## Onde está o quê

- `src-tauri/src/api/` — rotas Axum, `/ws` e comandos Tauri.
- `src-tauri/src/opencode_client/` — daemon manager e cliente REST V2.
- `src-tauri/src/mcp_control.rs` — MCP `opendesk-control` (tarefas, rotinas,
  receitas, fila, artefatos e atenção). `mcp_config.rs` continua com propostas.
- `src-tauri/src/mcp_server.rs` — loop JSON-RPC compartilhado pelos três MCPs.
- `starter-packs/packs/controle-opendesk/` — skill e comandos do pacote base
  instalado automaticamente em novos workspaces (ADR 0044).
- `src-tauri/src/db/` — migrações, modelos e repositórios.
- `src-tauri/src/integrations/telegram.rs` — bot Telegram (long polling,
  allowlist, chat, aprovações e avisos).
- `src-tauri/src/desktop_integration.rs` — atalho e ícone do AppImage no Linux.
- `src-tauri/src/events.rs` — contrato dos eventos em tempo real.
- `src-tauri/src/paths.rs` — diretórios do app e layout de workspace.
- `src/pages/` — as 20 páginas roteadas.
- `src/store/app-store.ts` — snapshot + WebSocket + ações.
- `docs/` — plano, ADRs e histórico.
- `PRODUCT.md` — verdade de produto e usuários.
- `DESIGN.md` — tokens, escala, componentes e regras visuais (ADR 0046).
- `scripts/check-changed.sh` — verificação proporcional padrão (`npm run verify`).
- `scripts/gate-fast.sh` — checagem ampla sem cobertura/E2E; `--changed` delega
  para `check-changed.sh`; o gate completo é `scripts/gate.sh` (opt-in).
- `docs/release-flow.md` — fluxo rápido de verificação, release e cache de build.

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

Fases 1–19 concluídas: fundação, Studio SSOT com gestão completa (efetivo,
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
Studio, versão no tray, contrato de scroll com auditoria E2E, model refs sem
`variant: null`, política de notificações sem repetição, continuidade de sessão,
preferências de notificação, Kanban em lista/lote, paginação, lease/heartbeat,
Attention Card, cron com misfire/safe/dry-run, pin de artefatos, MEMORY.md,
fork de tarefa, model mix/economia, permissões interativas, compactação de
contexto, A/B swarm, fork por run, rollback de artefato, Quick Capture global e
blueprints de equipe, janela maximizada, ciclo de bandeja, abas em
Configurações/Integrações/Kanban, timeline ao vivo, controlador de fila em aba,
barramento universal de I/O (UEN), sinks determinísticos, ferramentas externas
opcionais, hardening de SQLite/watcher/permissões, receitas YAML com HITL
modular, Rsync local/remoto, auto-organização de agentes com propostas aprovadas via HITL
e retry durável de Google Docs/Drive, além de aprendizado contínuo leve
(lições de runs e propostas de skill com aprovação HITL), separação entre
trabalho e automações no Kanban, deduplicação de tarefas/jobs com carência
configurável, HITL com pergunta e consequências explícitas, e governança de
agentes com organograma, metas, budgets, quota, performance, delegação HITL e
heartbeat opt-in, CLI headless, export de sessão em Markdown/HTML/JSON e
integração com OpenCode Browse para tarefas de navegação web e backup/restore
completo do workspace em ZIP, observabilidade operacional (p95, fila, falhas) e
contract test opt-in do OpenCode V2, rate limit e auditoria da API local,
integração Telegram por long polling (chat, aprovações, HITL e avisos de
rotinas) e atalho automático do AppImage no menu/barra de tarefas.
Pacotes iniciais opcionais (`starter-packs/`) embutidos no binário, incluindo a
Fábrica de Capacidades default (`meta-forge`). O MCP `opendesk-control` permite
aos agentes operar tarefas, execuções, rotinas, receitas, fila e artefatos; o
pacote base `controle-opendesk` é instalado em novos workspaces e rotinas e
receitas novas nascem desligadas (ADR 0044). A suíte funcional sob demanda vive
em `npm run test:features`.
Próxima frente: cobertura Rust ≥80% (plano em `docs/coverage-plan.md`; hoje
64,81% total/71,14% core), builds Windows/macOS (`docs/platform-builds.md`) e
contrato Google com conta dedicada (`docs/google-contract.md`).

Releases são locais (`releases/`, máximo 3 versões) e nunca via CI. O remoto é
`origin` em https://github.com/ticosanjr/OpenDesk (privado); `scripts/release.sh`
faz verificação proporcional, build rápido (thin LTO/incremental), empacota
`.deb`/`.rpm` e reaproveita o AppDir do AppImage (linuxdeploy só quando
dependências/ícones mudam), depois commit, tag e push, preservando
`src-tauri/target` para o próximo release. Use `--quality-build` para o perfil
completo de LTO e `--refresh-appimage` para refazer o AppImage do zero. `name` e
`avatar` de agentes são metadados do OpenDesk no frontmatter; `color` é nativo
(ADR 0002).
