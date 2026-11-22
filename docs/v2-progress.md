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

## Próximas ondas

2. `opendesk-engine`/`opendesk-recipes`, god-files e event-driven.
3. `opendesk-mcp`/`opendesk-channels` e desacoplamento.
4. `opendesk-enterprise`, extinção do core, shell fino, doctor e gate de limites.
