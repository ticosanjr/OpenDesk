# Plano de implementação — OpenDesk

Atualizado em 2026-11-06. Este documento é a referência de engenharia do
OpenDesk. Ele consolida a especificação original com as decisões tomadas depois
da verificação do OpenCode V2.2.5 instalado.

## 1. Decisões estruturais

| Tema | Decisão |
|---|---|
| OpenCode | **V2-only** (ver ADR 0001). API `/api/*`, Basic auth, arquivos V2. |
| Plataforma | Linux-first, código cross-platform; Windows suportado; macOS validado depois. |
| Processo | Um executável Tauri v2; Axum é a API canônica (REST + `/ws`); IPC só para janela/tray/atalhos. |
| Daemon | Um `opencode serve` dedicado para todos os workspaces; porta 3100 ou 3101–3199; senha efêmera capturada do stdout. |
| Multi-workspace | Uma sessão por workspace usando `location.directory`; um único assinante SSE global. |
| Persistência | SQLite em WAL para estado operacional; configuração do OpenCode nunca é espelhada no banco. |
| Segredos | Keyring do SO para SMTP/Google; fallback AES-GCM por máquina; senha do daemon só em `daemon.json` 0600 e memória. |
| Google | Drive, Docs, Calendar, Gmail e **Tasks**; Keep apenas enterprise. |
| Entrega | Fatias verticais (Rust + React + teste) em vez de backend inteiro antes da UI. |

## 2. Arquitetura

```text
React 19 + Vite + Tailwind 4 + shadcn-style primitives
        │ REST + WebSocket (token local)
        ▼
Axum em 127.0.0.1:<porta dinâmica> ── Core Tokio
        │                               ├── SQLite (WAL)
        │                               ├── watcher notify (Fase 2)
        │                               ├── fila + cron (Fases 3–4)
        │                               └── OpenCode V2 :3100 (Basic auth)
        ▼
Tauri v2 (janela, tray, notificações, atalhos)
```

- **Boot:** diretórios do app → SQLite/migração → listener Axum → token →
  daemon manager → bandeja → API → UI.
- **Shutdown:** mata o filho OpenCode criado pelo app, remove locks; se o app
  crashar, o lock permite reattach no próximo boot.
- **Eventos:** enum `OpenDeskWsEvent` com `event_type` + `payload` e envelope
  com `event_id` monotônico. Consumidor lento ou reconexão recebe
  `ResyncRequired` e refaz snapshots.
- **Contratos:** structs Rust + tipos TypeScript espelhados; o frontend valida
  payloads com Zod quando o contrato estabilizar.

## 3. Mapeamento spec V1 → OpenCode V2

| Documento (V1) | V2 | Implementação |
|---|---|---|
| `opencode serve --port 3100` + `/v1/models` | `/api/*`, `/api/info`, `/api/model` | Daemon dedicado + healthcheck + models |
| Sem auth | Basic `opencode:<senha>` | Senha capturada do stdout; nunca logada |
| `permission: {edit,bash}` | `permissions: [{action,resource,effect}]` | Matriz visual compila regras |
| `command` | `commands` | CRUD em `commands` e `.opencode/commands/` |
| `mcp.<nome>`, `type: sse`, `enabled` | `mcp.servers.<nome>`, `remote`, `disabled` | CRUD V2 |
| Agente `temperature`, `tools`, `permission` | `permissions`, `steps`, `color`, `hidden` | Ferramentas viram regras; temperatura sai |
| `effort` | variante `#high` | Resolve via `/api/model.variants`; senão metadado |
| `POST` síncrono | `prompt` enfileira + eventos + `wait` | Worker aguarda `session.idle` com timeout |
| SSE de execução | `/api/event` global e live-only | Um assinante + fan-out + resync |
| `write/edit/bash` | `session.tool.*`, `filesystem.changed`, `/diff` | Correlação por sessão/tool + diff final |
| `opencode agent list` | não existe | `GET /api/agent` + parser do disco |
| Google Keep | API enterprise | Google Tasks por padrão |

### 3.1 Location scoped

`/api/model`, `/api/agent` e `/api/config` aceitam
`?location[directory]=<path>` (deepObject). O diretório precisa ser um location
reconhecido pelo OpenCode. Na prática, criar `.opencode/` no workspace resolve.
O `AppPaths::ensure_workspace_layout` cria `input/`, `output/` e `.opencode/`.

## 4. Banco de dados

Arquivo: `~/.local/share/opendesk/opendesk.db` (Windows:
`%APPDATA%/opendesk/opendesk.db`). WAL, `foreign_keys=ON`, `busy_timeout=5s`.

Correções aplicadas em relação à spec original:

- `schedules` criada antes de `execution_queue` (foreign key).
- Índices para kanban, fila, runs, artefatos, cron e regras.
- Colunas adicionais: `task_runs.phase`, `parent_run_id`, `provider_id`,
  `pricing_snapshot_json`, `log_path`, `tokens_reasoning`;
  `schedules.timezone`; `execution_queue.scheduled_for`, `job_kind`;
  `artifacts_provenance.content_sha256`, `previous_sha256`;
  `workspace_integrations.smtp_secret_ref`, `google_scopes`.
- Tabelas novas: `app_settings`, `integration_deliveries`,
  `config_change_log`, `permission_audit`.
- `workspaces.is_active` não é exclusividade; o workspace ativo fica em
  `app_settings.active_workspace_id`.
- IDs UUID v7 gerados no Rust; timestamps UTC em texto RFC 3339.
- `budget_settings` tem seed de linha única.

## 5. API interna

Rotas atuais (Fase 1):

```text
GET    /healthz
GET    /api/health
GET    /api/meta
GET    /api/daemon/status
POST   /api/daemon/restart
GET    /api/models?directory=
GET    /api/agents?directory=
GET    /api/dashboard?workspace_id=
GET    /api/workspaces
POST   /api/workspaces
GET    /api/workspaces/{id}
PATCH  /api/workspaces/{id}
DELETE /api/workspaces/{id}
POST   /api/workspaces/{id}/activate
GET    /api/settings
GET    /api/settings/{key}
PUT    /api/settings/{key}
GET    /ws?token=
```

Todas as rotas `/api/*` exigem `Authorization: Bearer <token>`. O token é
gerado no boot e gravado em `api.json` 0600 para ferramentas locais.

Próximas rotas: tasks, fila, runs, artefatos, schedules, router rules, budget,
Studio, integrações e ações de entrega.

## 6. Fases e status

### Fase 1 — Fundação (concluída)

- [x] Scaffold Tauri v2 + React 19 + TypeScript + Vite + Tailwind 4.
- [x] SQLite + migração `0001_initial_schema.sql` + repositórios de workspaces
      e settings.
- [x] Axum + token + CORS + WebSocket + eventos.
- [x] Daemon manager V2 com auth Basic, healthcheck, restart e reattach.
- [x] Bandeja com status e ações básicas.
- [x] Shell React: sidebar 9 views, topbar, workspace switcher, budget, fila,
      command bar e dashboard.
- [x] Criação de workspace com `input/`, `output/`, `.opencode/`.
- [x] Endpoints de agentes/modelos scoped por workspace.

**Critério de saída:** app abre, daemon fica online, status muda em <5 s,
workspace persiste, API responde com e sem token.

### Fase 2 — Studio SSOT (concluída)

- [x] `jsonc_editor` com CST, preservação de comentários e escrita atômica.
- [x] Leitura/escrita de `agents/*.md`, `skills/*/SKILL.md`,
      `commands/*.md` e `mcp.servers`.
- [x] Hash de concorrência: `409` se o arquivo mudou por fora.
- [x] Watcher com debounce de 150 ms + índice + `FS_SYNC_EVENT`.
- [x] Studio UI completa e editor JSONC bruto.
- [x] Testes de round-trip e de paridade <500 ms (teste de watcher).
- [x] Pasta padrão de workspaces + adicionar workspace de qualquer pasta.
- [x] Agentes com nome de exibição, avatar e cor (ADR 0002).
- [x] Testes Playwright end-to-end.

### Fase 3 — Execução (concluída)

- [x] Roteador DSH: políticas, regras IF-THEN, classificador, context packer,
      output rule, Meta-Builder.
- [x] Fila com prioridade, pausa, cancelamento, retry e recuperação de crash.
- [x] Budget Guard com hard/soft cap diário, mensal e janela de 5h.
- [x] Cliente de sessão/prompt/eventos/wait/interrupt + broker de permissões.
- [x] Stream interceptor + proveniência + logs + custo/tokens.
- [x] Kanban, aprovação de plano, controlador de fila, Chat e Meta-Builder.
- [x] Auditoria automática na política BEST_QUALITY.
- [x] Concorrência configurável (pendente; hoje 1 job por vez).

### Fase 4 — Automação (concluída)

- [x] Cron dirigido pelo SQLite, idempotência e timezone.
- [x] Pastas determinísticas `output/YYYY-MM/<slug>/`.
- [x] SMTP com templates HTML, anexos e segredo criptografado.
- [x] Pipeline pós-run com `integration_deliveries` e idempotência.
- [x] UI de Rotinas e Integrações SMTP.
- [ ] Retry automático de entregas falhas (próxima iteração).

### Fase 5 — Google + MCP (concluída)

- [x] OAuth2 loopback/PKCE + secret store + refresh.
- [x] Drive, Docs, Calendar, Gmail e Tasks.
- [x] Subcomando `opendesk mcp-google` via stdio.
- [x] Registro em `mcp.servers` e UI de Integrações.
- [x] Export pós-run para Google Docs/Drive.
- [ ] Teste com conta Google real (depende de credenciais do usuário).

### Fase 6 — Observabilidade e polimento

- [ ] Custo por modelo, janela de 5 h, cache hit e snapshot de preço.
- [ ] Dashboard completo, Provenance Explorer e File Explorer.
- [ ] Janela rápida da bandeja, notificações e atalho global.
- [ ] Retenção, backup, segurança, empacotamento e assinatura.
- [ ] Critérios de aceite 1–4 verdes.

## 7. Critérios de aceite

| Critério | Fase | Prova |
|---|---|---|
| Paridade CLI ↔ UI em <500 ms | 2 | teste com diretório temporário + evento WS cronometrado |
| Proveniência de todo arquivo criado | 3 | servidor OpenCode falso com SSE roteirizado + DB |
| Hard cap pausa a fila e notifica | 3 | limite abaixo do gasto + evento nativo |
| Rotina sem chat, pasta + e-mail | 4/5 | cron de teste + SMTP/Drive mockados |

## 8. Riscos e mitigação

| Risco | Mitigação |
|---|---|
| API V2 experimental | Trait adaptador, gate de versão, testes de contrato |
| Parse da senha do daemon | Timeout, fallback para lock file, erro acionável |
| SSE sem replay | `ResyncRequired` + snapshots de sessão/diff |
| Permissão `ask` travar job | Broker com timeout e política por workspace |
| Artefato via bash sem diff | `filesystem.changed` + snapshot + `/diff` |
| Keyring ausente no Linux | Fallback AES-GCM por máquina |
| Tray no Linux | `libayatana-appindicator` + teste de empacotamento |
| OAuth restrito | Projeto Google do usuário em Testing |
| Meta-Builder inseguro | Validação de slug/path/YAML, preview e backup |
| Custo impreciso | `assistant.cost` + snapshot de preço + reconciliação |

## 9. Releases locais

`scripts/release.sh` compila, empacota e mantém no máximo 3 versões em
`releases/<versão>/` com `manifest.json`, `latest.json`, `SHA256SUMS` e
`LATEST`. Não há CI. Binários são ignorados pelo git; metadados e checksums são
versionados. O script cria commit, tag, GitHub Release privada e envia os assets
com `gh` quando o remoto é GitHub.

## 9.1 Fase 6 — resiliência e atualizações

- [x] Resolver de binário OpenCode com persistência e validação V2.
- [x] Fallback para o serviço compartilhado (`service.json`).
- [x] Banner, card do motor, diagnóstico e log rotativo.
- [x] Preflight 503 em dispatch/aprovação/run-now.
- [x] Atualização do OpenCode V2 pela UI.
- [x] Auto-atualização do OpenDesk via release privada, SHA256 e backup.
- [ ] Cobertura ≥80% nos módulos de lógica.
- [ ] Rollback automático de versão e retry de entregas.

## 10. Como contribuir

1. Leia o ADR 0001 e este plano.
2. Não grave configuração do OpenCode no SQLite.
3. Toda alteração de comportamento atualiza `docs/progress.md` e, se mudar uma
   decisão, cria/atualiza um ADR.
4. Rode `npm run check`, `npm run build`, `cargo test` e `npm run test:e2e`
   antes de concluir.
