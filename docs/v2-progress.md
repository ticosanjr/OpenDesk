# Progresso V2

Registro de mudanças de comportamento da refatoração V2. O histórico V1 está em
`docs/archive/v1/progress.md`.

## 2026-11-22 — Onda 1: fundação e corte limpo

- Auditoria de 66 tabelas em `docs/v2-migration-audit.md`: 5 fundidas em
  conversas, 3 descartadas (0 referências) e 1 coluna morta removida.
- Criado `opendesk-domain`: IDs, `TaskStatus`, `RunStatus`, HITL, conversas,
  eventos e ports; 19 testes; zero dependência de SQL/HTTP/fs/Tauri.
- Criado `opendesk-storage`:
  - baseline `0001_v2_baseline.sql` com 61 tabelas `STRICT`, `foreign_key_check`
    e `integrity_check` limpos;
  - leitura concorrente + writer serializado + write-buffer;
  - conversas/mensagens unificadas, turnos e HITL;
  - guarda que recusa banco V1 e testes de pragmas.
- `opendesk-core` aponta `init` para o storage e reexporta as conversas; as 59
  migrations V1 foram apagadas.
- `opendesk-cli migrate-v1-export` exporta workspaces, receitas e settings em
  JSON/YAML com manifesto SHA256, sem segredos.
- Docs: `SPEC.md` e `ARCHITECTURE.md`; ADRs/progresso/releases V1 em
  `docs/archive/v1/`.
- Verificação: **340 shell + 286 core + 19 domínio + 15 storage (660 testes)**;
  clippy `--workspace --all-targets --all-features -D warnings` limpo;
  `./scripts/gate.sh` verde (Playwright, goldens, cobertura total 54,61% e core
  64,10%).

## 2026-11-22 — Onda 2 (início): engine, recipes e parser

- Criados `opendesk-engine` e `opendesk-recipes` (deps: domain + storage).
- Recorrência unificada em `engine/scheduler/recurrence.rs`; `recipes/cron.rs`
  consome a mesma implementação.
- `worktree` dividido em `engine/src/worktree/{mod,manifest,git_ops,overlay,sweeper}`.
- `recipes/parser` dividido em `parser/{mod,schema,expressions}`.
- Core reexporta os três módulos; shell sem mudança de API.
- Testes: 340 shell + 267 core + 19 domain + 6 engine + 13 recipes + 15 storage
  (660); clippy e `scripts/gate.sh` verdes.
- Pendente: fila/scheduler/runner/telemetria/router/adapters/DAG/store/fachada,
  event-driven e 75% de cobertura; bloqueado pela extração dos 28 repositórios
  de `db` restantes. ADR 0201.

## 2026-11-22 — Onda 3 (início): domínio de canais e MCP

- Domínio: `ChannelAdapter`/`NormalizedEnvelope` (anexos, thread, dedupe),
  `CareLevel`/`QuietHours`, ports de elicitation/sampling.
- `opendesk-mcp`: `rate` e `rbac` movidos do core (puros), lock sem `expect`.
- `opendesk-channels`: features por canal, registry de descritores e
  `NotificationRouter` (care level, quiet hours, crítico fura a janela).
- `grep` de canais em engine/recipes: vazio.
- Testes: 340 shell + 264 core + 27 domain + 6 engine + 3 mcp + 13 recipes +
  15 storage + 4 channels (672); clippy limpo.
- Pendente: pool/server/config do MCP, adaptadores reais, RBAC/rate/auditoria no
  ponto de chamada e elicitation/sampling. ADR 0202.

## 2026-11-22 — Onda 4 (início): enterprise e gate de limites

- `opendesk-enterprise`: `ent-auth` (mtls, update_signing) e `ent-computer`
  (virtual_display) movidos do core; flags `ent-cluster`/`ent-eval` criadas.
- Gate `scripts/check-rust-limits.py` + baseline-ratchet: falha em novo arquivo
  >500 linhas ou `unwrap/expect` fora de testes; dívida legada congelada em 31
  entradas.
- Core reexporta os módulos movidos; shell sem mudança.
- Testes: 340 shell + 258 core + 27 domain + 6 engine + 3 mcp + 13 recipes +
  15 storage + 4 channels + 6 enterprise (672); clippy e gate verdes.
- Pendente: extinção do core, sandbox/nodes/workspace_sync/skill_*, shell fino,
  frontend, doctor e 75%. ADR 0203.

## 2026-11-22 — Consolidação total do storage

- 26 repositórios de `db` + `paths`/`secrets`/`events`/`server_lock`/`dedupe`
  movidos para `opendesk-storage`; core virou fachada por reexports.
- `tasks`/`schedules`/`calendar`/`memory` divididos via `include!`; `queue` e
  `io_bus` com testes separados. Nenhum `.rs` do storage >500.
- Zero `expect` de produção nos crates (locks, reqwest, test helpers).
- Baseline de limites: 31 → **19** entradas (só god-files de engine/recipes/mcp).
- Core ~43.4k → **29.3k** linhas; storage ~12.3k.
- Testes: **672** (340 shell + 194 core + 27 domain + 6 engine + 3 mcp + 13
  recipes + 15 storage + 4 channels + 6 enterprise + repos no storage);
  clippy limpo. ADR 0204.
- Engine desbloqueado: pode depender do storage sem depender do core.

## 2026-11-22 — 1.4.1: abertura com banco V1

- Pré-voo detecta V1 antes de janela/tray/locks e oferece "Exportar e continuar".
- Migração exporta workspaces/receitas/settings, preserva `.db`+WAL/SHM e relança
  com V2; o binário desktop ganhou `migrate-v1-export`.
- Janela oculta até o backend subir; erros de startup viram diálogo.
- Release script sempre reconstrói o daemon; RPM antigo é limpo.
- ADR 0205.

## 2026-11-22 — 1.4.2: hardening

- Zero `unwrap/expect` de produção no shell; baseline vazia.
- Limite de 500 linhas aplicado ao shell e ao frontend; 20 arquivos divididos.
- Cobertura medida com `--workspace`; piso 64,0% total / 80,5% no escopo de negócio.
- Spawns longos com `kill_on_drop` + process group.
- Fila e cron event-driven com `Notify`, sono até o lease e fallback de relógio.
- Shutdown drena jobs; TERM→KILL com testes de processo.
- ADR 0206.
- Primeiro corte do engine: sandbox/runtime/browse/config_sync/dsh_router/adapters/
  opencode_client e portas movidos para `opendesk-engine` com reexports (ADR 0207).

## 2026-11-22 — 1.5.0: engine e receitas

- `recipes` (DAG, store, engine, HITL, parser), `RecipeContext` e export movidos
  para `opendesk-recipes`; core reexporta.
- `queue_scheduler` inteiro movido para `opendesk-engine`.
- Cron usa a porta `ScheduledRecipeRunner`, implementada por
  `ContextRecipeRunner` (sem ciclo).
- Worktree store no engine; core com ~6,2 mil linhas.
- God-files de engine/recipes divididos; baseline de limites 19 → 3.
- ADR 0208.

## 2026-11-22 — 1.7.0: core extinto

- `opendesk-core` removido; módulos restantes no shell/storage.
- `crate::db` é fachada de `opendesk_storage::repos`.
- Baseline de limites vazia; nenhum arquivo em `src-tauri/crates` > 500 linhas.
- `cargo check --workspace --no-default-features` e testes all-features verdes.
- Canais completos (Telegram/Signal/WhatsApp/e-mail/WebPush) e MCP no crate.
- ADR 0212.

## 2026-11-23 — 1.7.1: I/O bus fail-closed

- `io_bus` não aceita mais `token`, `access_token`, `password`, `secret`,
  `api_key`, `database_url` ou `private_key` em texto plano.
- Migração única no boot move credenciais legadas para o `SecretStore` e
  reescreve o config com `*_ref`; falhas mantêm a linha intacta e o runtime a
  rejeita.
- Validação na API (sources/sinks/rotinas), no runtime de inbound/outbound e
  redaction de credenciais em erros SQL.
- 11 testes novos no shell (346 no total). ADR 0213.
- Studio install fail-closed: apenas HTTPS, DNS/IP privado bloqueado, redirects
  revalidados, download em streaming (25 MiB), ZIP com 250 entradas/50 MiB/ratio
  50/sem symlink e diretório local canonicalizado. 8 testes novos (354 no shell).
  ADR 0214.
- Frontend runtime adapter: links, caminhos locais e Quick Capture passam por
  `src/lib/runtime-env.ts`; PWA não chama Tauri, esquemas inválidos são
  bloqueados e caminhos locais viram toast. 10 testes novos no Vitest (266).
  ADR 0215.
- Storage sem `include!` de produção: tasks/schedules/calendar/memory viraram
  módulos reais (`model`/`mutations`/`events`/`notes`), com a mesma API.
  `Recurrence` foi consolidado em `engine::scheduler::recurrence`. Fachadas de
  conversas mantidas (sem SQL duplicado). ADR 0216.
- Canais: `io_bus` deixou de duplicar Telegram; envio, polling e callback usam
  `opendesk-channels` (novo `send_text_with_parse_mode`/`get_updates_value`).
  Matrix permanece no shell. ADR 0217.
- Manutenção do banco movida para `opendesk-storage` (SQL direto para cerimônias
  WebAuthn); MCPs consolidados em `src/mcp/`, com o registro de sessões puro em
  `opendesk-mcp::session`. Passkey/audit no shell e sandbox no engine por decisão
  arquitetural. Shell 349, storage 89, MCP 12. ADR 0218.
- Chat event-driven: `RestClient::event_receiver` + parser SSE compartilhado;
  `wait_for_turn` acorda em idle/execução, ignora outra sessão, aplica timeout
  configurável e usa fallback apenas quando necessário; parsing em
  `chat_parse.rs`. Shell 352. ADR 0219.

## 2026-11-23 — 1.7.1: limite no shell

- Limite de 500 linhas e proibição de `include!` de produção passam a valer
  também em `src-tauri/src`; baseline vazia.
- 31 arquivos de produção divididos por concern (CLI, packs, APIs, MCP,
  integrações, chat, doctor, computer, learning, governance, backup, updates,
  io_bus, calendário).
- `docs/implementation-plan.md` arquivado; caminho da chave de assinatura
  corrigido; CSS sem uso removido (495 → 386 linhas).
- ADR 0220. Cobertura: 64,08% total / 76,40% no escopo de negócio.

## Próximas ondas

2. `opendesk-engine`/`opendesk-recipes`, god-files e event-driven.
3. `opendesk-mcp`/`opendesk-channels` e desacoplamento.
4. `opendesk-enterprise`, extinção do core, shell fino, doctor e gate de limites.
