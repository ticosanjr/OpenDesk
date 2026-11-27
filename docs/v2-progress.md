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

## Fechamento (próxima frente)

Plano detalhado em
[`docs/fechamento-opendesk-plan.md`](fechamento-opendesk-plan.md): Credential
Vault Zero-Exposure com `SecretScrubber`, prompts internos blindados e com teto
de tokens, DAG 2.0 no frontend/MCP, I/O unificado (HTTP Poller, HTTP Webhook
Sink, entregas WhatsApp/Push), canais Discord/Slack/WhatsApp QR e importação
opcional via Bitwarden CLI.

### F0 — schema base (concluída)

- ADRs 0221–0224 registram Vault, I/O unificado, canais e contrato DAG.
- `0002_vault_and_triggers.sql` cria `vault_entries`, `vault_acls`,
  `vault_audit`, `io_poller_state` e `io_poller_seen`, e reconstrói as tabelas
  de I/O com `HTTP_POLLER`, `HTTP_WEBHOOK`, `WHATSAPP_MSG`, `WEBPUSH`,
  `RSYNC_FS`, `SIGNAL_MSG`, `target_kind` e `target_id`.
- `secrets.rs` expõe `seal`/`open` reutilizados pelo `SecretStore`; o tipo
  seguro de metadados do Vault está em `repos/vault.rs`.
- `InboundSource`/`InboundInput` expõem `target_kind`/`target_id`; o repositório
  aceita `HTTP_POLLER`, `HTTP_WEBHOOK`, `WHATSAPP_MSG`, `WEBPUSH`, `RSYNC_FS` e
  `SIGNAL_MSG`, inferindo `RECIPE`/`SCHEDULE` de configs antigas.
- Features `ch-discord`, `ch-slack` e `ch-whatsapp-qr` declaradas.
- Teste de upgrade parte do schema `0001`, preserva bindings e aceita os novos
  tipos; um segundo teste confirma as tabelas em banco novo.

### F1 — Credential Vault + SecretScrubber (concluída)

- `repos/vault*` implementam CRUD de metadados, ACL por agente/receita/MCP/sink,
  auditoria e `resolve_for_edge` fail-closed (domínio exato, expiração, ACL e
  HITL). O plaintext só existe dentro dessa função.
- `/api/vault` cobre CRUD, ACL, auditoria, `import-env` e `request-fill`; nenhuma
  rota devolve o segredo.
- MCP ganhou `vault_list_refs` (metadados) e `vault_request_fill` (HITL), sem
  expor o valor ao agente.
- `VaultCard` na aba Credenciais cria, rotaciona, edita ACL, mostra auditoria e
  importa `.env`.
- O webhook HMAC resolve o segredo pelo Vault (fallback legado no `SecretStore`)
  e registra o valor no scrubber global.
- `SecretScrubber` mascara valor exato, Base64, Base64 URL-safe e
  percent-encoded em stdout, SSE, logs, diffs, writes e resumo do run; CLI e
  `session_runner` aplicam antes de persistir/transmitir.
- A migração de boot importa `*_ref` do `io_bus` para o Vault, cria ACL para a
  origem e é idempotente; o `SecretStore` continua como fallback.
- Testes: storage **101**, shell **356**, engine **134** e Vitest **267**.

### F2 — Prompts blindados e com orçamento (concluída)

- `context_packer` renderiza `<system_rules>`, `<memory>`, `<task>`,
  `<output_contract>` e `<untrusted_data>`; dados de canal/chat/follow-up são
  escapados e nunca fecham o bloco.
- `PromptBudget` aplica `P0`–`P5` com teto padrão de 16 000 tokens, fallback
  conservador e marcador `[TRUNCATED: N tokens omitidos]`; regras e tarefa nunca
  são cortadas.
- `harness_id` remove instruções exclusivas do OpenCode quando o run usa outro
  harness; `output_schema` gera contrato JSON estrito.
- `prompt_preview` e o preview do chat usam o mesmo renderizador.
- Starter packs prioritários (ad-creative-studio, afiliados-growth,
  colecionador-vault, compras-dispensa, youtube-pipeline e assistente-pessoal)
  ganharam processos, saídas e critérios reais; um teste bloqueia o boilerplate
  genérico. O comando `opendesk-receita.md` agora documenta DAG completo,
  schemas, `vault_refs` e sinks.
- Testes: storage **101**, shell **358**, engine **139** e Vitest **267**.
- ADR 0225.

### F3 — Receitas DAG 2.0 (concluída)

- Parser de `step` ganhou `retry_max`, `timeout_secs`, `on_schema_error`
  (`retry_format`/`fail`/`hitl`) e `vault_refs`, com validação de limites e
  duplicatas.
- O motor usa timeout por step, repete falhas até `retry_max`, aplica a política
  de schema e cria HITL quando a saída não valida e a política é `hitl`.
- `/api/recipes/validate` retorna `valid`, `issues` e a receita normalizada,
  incluindo checagem de `vault_refs`/ACL; `/api/recipe-runs/{id}/resume` e
  `/nodes/{*path}/retry` preservam nós concluídos e retornam 409 para runs
  concluídos/ativos.
- MCP ganhou `recipe_validate`; `recipe_resume` agora usa o endpoint de resume.
- `recipe_run` com `dry_run=true` valida `vault_refs`/ACL antes de simular.
- Frontend cobre os nove tipos de nó, preserva campos desconhecidos no
  round-trip, adiciona `DagFlowEditor`/`DagNodeEditor`, `SchemaContractEditor`,
  campos de retry/timeout/schema/Vault no step, gaveta de versões com
  diff/rollback, botão Retomar e retry por nó no detalhe do run, além de
  “Gerar com IA” via `/opendesk-receita`.
- HITL novo nasce com `DESKTOP_UI` e `PUSH`.
- Testes: storage **101**, shell **362**, engine **139**, recipes **51** e
  Vitest **268**.

### F4 — I/O unificado e entregas (concluída)

- `HTTP_POLLER` ganhou worker real com RSS/Atom/JSON/HTML, `ETag`/
  `Last-Modified`, deduplicação por item/hash (`io_poller_seen`), bearer do
  Vault, limite de 8 MiB, intervalo mínimo de 60 s e erro persistido sem
  derrubar o worker.
- `HTTP_WEBHOOK` outbound renderiza `{{run.*}}`/`{{artifact.paths}}`, resolve
  `{{vault:<id>}}` e bearer no Vault, envia
  `X-OpenDesk-Idempotency-Key`, rejeita redes privadas/metadata por padrão e
  repete 5xx/429 sem seguir redirects.
- Webhook HMAC aceita `vault_ref` além de `secret_ref`.
- `DeliveryChannel` ganhou `WHATSAPP` e `WEBPUSH`; entregas pós-run usam o
  telefone configurado do WhatsApp e todos os dispositivos WebPush, com opt-in
  pelo roteador de notificações. Sinks `WHATSAPP_MSG`/`WEBPUSH` também foram
  ligados ao barramento.
- `target_kind=TICKET` agora cria ticket idempotente via `io_bus`; o formulário
  de entradas expõe destino (`TASK`/`RECIPE`/`TICKET`/`SCHEDULE`) e id.
- UI ganhou campos tipados para HTTP Poller, webhook HTTP (preset n8n),
  WhatsApp e WebPush, mantendo o JSON avançado como escape.
- Testes: storage **101**, shell **367**, engine **139**, recipes **51** e
  Vitest **268**.
- Poller aceita HTTP/HTTPS público, revalida cada redirect e permite rede
  privada via `allow_private_network`; WhatsApp/WebPush têm retry próprio; o
  scanner de pasta de receitas atua apenas como fallback quando a fonte de I/O
  ainda não foi materializada.

### F5 — Discord, Slack e WhatsApp QR (concluída)

- Discord: gateway WebSocket com HELLO/IDENTIFY/heartbeat, intents de mensagens,
  allowlist por guild/canal/usuário, REST para texto, botões de HITL e resolução
  de `hitl:<id>:<decision>`.
- Slack: Socket Mode (`apps.connections.open`), ack de envelope, allowlist por
  canal/usuário, `chat.postMessage` e Block Kit para HITL.
- WhatsApp Web pessoal: bridge de sidecar externo via JSON Lines (QR, status,
  mensagens e erros), allowlist, envio de texto, backoff e circuit breaker após
  cinco falhas. Nenhum Baileys é embutido.
- Mensagens recebidas de Discord/Slack/QR entram no chat do workspace e a
  resposta do agente volta pelo canal; botões resolvem HITL de receita e
  interações de tarefa.
- Tokens ficam no `SecretStore`; configurações e allowlists em `app_settings`.
- `ChannelTransport` comum e `TrustLevel` (allowlist = trusted; fora dela,
  untrusted) cobrem texto/HITL; threads e anexos de Discord/Slack são repassados
  ao chat; QR publica `ChannelStatus` para o Attention Card.
- API `/api/channels/discord`, `/slack` e `/whatsapp-qr` (status, PUT, start,
  stop, send); aba **Discord & Slack** na UI com formulários e QR.
- Testes: storage **101**, shell **369**, engine **139**, recipes **51**,
  channels **15** com features e Vitest **268**.
- Bridges exigem credenciais reais (bot/app tokens) e o WhatsApp QR depende do
  sidecar externo configurado; nenhum token é persistido em `config_json`.

### F6 — Bitwarden, doctor e fechamento (concluída)

- `bw` entrou no catálogo de ferramentas com detecção manual/PATH, sem `pkexec`.
- `POST /api/vault/import-bitwarden` importa itens selecionados usando
  `BW_SESSION` apenas no ambiente do processo `bw`; a sessão não é persistida
  nem aparece em erros/logs.
- Itens importados usam os hosts das URIs como `allowed_domains`, nascem com
  `requires_hitl=true` e sem ACL; o operador libera a injeção depois.
- O Vault ganhou botão **Bitwarden** na UI, com lista opcional de IDs de itens.
- `doctor --live` agora verifica Vault (expirados/sem ACL), HTTP Pollers,
  receitas habilitadas, Discord (`/users/@me`), Slack (`auth.test`) e WhatsApp
  QR (sidecar/status), além dos checks existentes.
- Testes: storage **101**, shell **372**, engine **139**, recipes **51**,
  channels **15** e Vitest **268**.

### F7 — Documentação, revisão final e gate completo (concluída)

- `scripts/gate.sh` foi executado como verificação final opt-in (TypeScript,
  ESLint, Vitest, `cargo fmt --check`, clippy `-D warnings`, limites de 500
  linhas/sem `unwrap`, testes Rust, build do frontend, cobertura Rust,
  Playwright e goldens visuais).
- A cobertura Rust estava em 62,68% e o gate falhou no piso de 63,0%. A lacuna
  foi fechada com testes — nenhum piso foi reduzido — cobrindo Bitwarden/Vault,
  HTTP Poller, HTTP Webhook, bridges Discord/Slack/WhatsApp QR, probes do
  `doctor --live`, canais de equipe, runtime de receitas por pasta, voz,
  exportação V1 e utilitários de update.
- Números finais medidos: **64,60%** no total (piso 63,0%) e **76,17%** no
  escopo de negócio (piso 75,5%). `engine`+`recipes` isolados ficam em 68,0%.
- Suíte Rust: storage **101**, shell **401**, channels **43**, domain **27**,
  engine **139**, enterprise **13**, mcp **12** e recipes **51**; Vitest **268**.
- Clippy também pegou um `PathBuf` importado sem uso em `vault_bitwarden.rs`,
  corrigido junto com o split de `import_with_bw` (permite testar a importação
  com um `bw` falso sem tocar no PATH do host).
- Revisão final de desempenho: cliente HTTP compartilhado em
  `opendesk-channels` (pool único para Discord/Slack/WhatsApp),
  `SecretScrubber` com caminho rápido `Cow` sem cópia e `kill_on_drop` no
  processo do `bw` para não deixar filho órfão no timeout.
- Playwright **103** e **49** goldens verdes. Os goldens de `search-1440` e
  `recipes-1024` foram atualizados por mudanças intencionais (atividade recente
  determinística e botões **Gerar com IA**/**Versões**); o teste de busca agora
  espera a seção de atividades antes do screenshot.
- ADR 0226 documenta a importação Bitwarden e a ampliação do `doctor --live`.
- Versão **1.8.0**: release local com verificação, build LTO, `.deb`, `.rpm`,
  AppImage, checksums, `latest.json` assinado, commit e tag.

## 2026-11-24 — Estabilização operacional: Fase 1 (navegação)

- `src/lib/links.ts` virou o contrato único de deep-links, com `queue(jobId)`,
  `kanban(taskId)`, abas do Studio, `?artifact=`, `?routine=` e seções de
  Settings/Integrações tipadas; componentes deixaram de montar `/kanban` à mão.
- `KanbanPage` mantém `?view=board|list|queue` na URL e reage a mudanças de
  `searchParams` sem recarregar; `?task=` volta para o board e `?job=` abre a
  fila com foco no job (`data-focus-id` + `useFocusParam`).
- Topbar e Sidebar apontam a fila para `links.queue()`; notificações passam a
  usar o link canônico da tarefa/artefato/rotina.
- Backend alinhado: itens `queued` do calendário vão para
  `/kanban?view=queue&job=<id>`; busca global emite `view=board` em tarefas e
  comentários e `?routine=` em rotinas.
- Testes: Vitest **272**, contrato de links, foco da fila e E2E
  `kanban-views` (troca de aba em rota quente) verdes.
- Fases 2–6 do `docs/plano-melhorias-operacionais.md` seguem pendentes.

## 2026-11-24 — Estabilização operacional: Fase 2 (fila)

- `QueuePolicy` persistida em `app_settings` (`queue_policy`) com
  `max_concurrency` (1–32, default 3), `max_queue_size` (default 200),
  `default_agent_concurrency` (default 2) e `group_limits`; o antigo
  `queue_concurrency` é migrado no boot.
- `next_eligible` aplica limites configuráveis por grupo, quota por agente
  (perfil explícito ou default) e impede dois jobs simultâneos na mesma
  tarefa/sessão, sem head-of-line blocking entre grupos.
- Enqueue fail-closed respeita `max_queue_size` e retorna `QueueFull` (HTTP 429).
- API ganhou `GET/PUT /api/queue/config`, `wait_reason` por job, `max_*` e
  `group_usage` no status; `/api/queue/concurrency` aceita 1–32.
- UI: controlador mostra `slots ativos/máx`, teto da fila, motivo de espera,
  seletor 1–32 com +/− e escopo “Tudo” por padrão; Settings edita a política
  completa. Sidebar usa `allQueueCounts` (autom ações incluídas).
- Testes: claims paralelos com barreira (3 slots), fila cheia, quota de agente,
  mesmo task/sessão, head-of-line blocking e motivos de espera.

## 2026-11-24 — Estabilização operacional: Fase 3 (tempo real)

- Store ganhou `queueStatus`, `attentionCount`, `revisions` por domínio,
  `wsConnected`, `refreshLiveState` e `workspaceRequestSeq`; respostas de um
  workspace anterior são descartadas.
- `QueueUpdated` atualiza fila, status, dashboard e revisão sem esperar troca de
  página; `TaskUpdated`/`RunFinished` disparam refresh live com debounce de
  150 ms; artefatos incrementam contadores na hora.
- Resync acontece em `ResyncRequired`, reconexão do WebSocket, retorno de
  `visibilitychange` e snapshot inicial do servidor.
- `attentionCount` é calculado pelo store a partir de `/api/attention` e
  atualizado pelo Attention Card; Sidebar mostra atenção, fila e runs ao vivo.
- Topbar usa fila/status do store (inclui automações); Dashboard, Kanban, Runs e
  Artefatos recarregam por `revisions`; cache do Studio é invalidado em
  resync/aprendizado/engine.
- Backend: `send_initial` envia `ResyncRequired` após o status do daemon para o
  cliente buscar o snapshot do workspace ativo.
- Testes: 276 Vitest, incluindo rajada de 50 eventos em um único refresh e
  descarte de resposta atrasada de outro workspace.

## 2026-11-24 — Estabilização operacional: Fase 4 (backup v2)

- Backup v2 inclui `db/workspace_state.json` com todas as tabelas que têm
  `workspace_id` (descobertas via `sqlite_master`), `db/run_logs/*.jsonl` e,
  opcionalmente, `db/secrets.enc` protegido por senha.
- Manifesto ganhou `includes`, `counts`, `db_sha256`, `files_sha256` e
  `workspace_root`; backups v1 continuam restaurando arquivos.
- Restauração `new_workspace`: novo `workspace_id`, remapeamento de PKs UUID e
  referências `*_id`, reescrita de caminhos absolutos para o novo root.
- Restauração `in_place`: backup de segurança automático, staging, troca de
  arquivos com rollback e substituição de linhas em transação única com
  `foreign_key_check`.
- Segredos portáveis: Argon2id + AES-256-GCM para `SecretStore` e plaintext do
  Vault; `secrets.enc` sem senha correta aborta antes de escrever. Segredos
  globais de outras integrações não entram no arquivo.
- API/UI: criação com logs/senha, restauração por modo, relatório com
  `missing_secrets` e `pre_restore_backup`.
- Testes: round-trip com tarefa restaurada, restauração in-place, senha
  errada e arquivo corrompido.

## 2026-11-24 — Estabilização operacional: Fase 5 (config bundle)

- JSON único (`schema_version: 1`) com memórias (`MEMORY.md` + notas,
  namespaces e ACLs), contexto pessoal, aprendizado, templates de prompt,
  roteador, budget, política da fila/dedupe, rotas de automação e governança
  (perfis/metas). Assets do Studio continuam nativos no workspace e não são
  duplicados no JSON.
- Exportação remove recursivamente campos de senha/token/segredo/api_key/
  ciphertext; referências `*_ref` e metadados operacionais permanecem.
- Endpoints `POST /api/config-bundle/export|preview|import` com limite de 10 MB,
  validação estrita de `schema_version` e prévia de conflitos
  (`incoming`, `existing`, `create`, `update`).
- Modos de importação: manter existentes, sobrescrever e substituir seção;
  memória cria namespaces ausentes e remapeia ACLs para os novos IDs.
- CLI: `opendesk config export <arquivo.json>` e
  `opendesk config import <arquivo.json> [--dry-run] [--mode ...]`.
- UI: card em Configurações › Geral com presets, download, upload, prévia e
  confirmação; atalhos de exportação em Memória estruturada e Prompts.
- Testes: sanitização de segredos, recusa de schema desconhecido, round-trip de
  memórias sem duplicatas, preview/import via API e atualização por overwrite.

## 2026-11-24 — Fase 6: gate final e release 1.9.0

- Gate completo: TypeScript, ESLint, Vitest (287), fmt, clippy, limites, testes
  Rust, build, cobertura, 103 E2E verdes e 49 goldens.
- Cobertura Rust final: **64,56%** total (piso 63,0%) e **76,30%** no escopo de
  negócio (piso 75,5%); frontend branches **90,09%** (piso 90%).
- Goldens de `learning-1440` e `settings-1920` atualizados por mudanças
  intencionais (exportação JSON).
- Versão **1.9.0** preparada para release local com AppImage, `.deb`, `.rpm`,
  binários portáveis, checksums e `latest.json` assinado.

## 2026-11-25 — Melhorias v2, Fase 1: integridade SQLite e receitas

- Novo `opendesk-storage/src/integrity.rs`: `quick_check(5)` no boot, backup
  `<db>.corrupt-<ts>/` com `.db`, `-wal` e `-shm`, reparo via `VACUUM INTO` e
  fallback `sqlite3 .recover`, validação `integrity_check` e troca atômica.
- Banco inexistente é aceito; banco corrompido nunca abre o app em estado
  inválido. Em falha de reparo o original é preservado e o erro aponta o backup.
- `opendesk repair-db [--db <path>]` adicionado ao binário desktop.
- Erros SQLite `code 11` agora retornam mensagem acionável orientando o reparo.
- `doctor --live` passou a reportar `integrity_check` detalhado.
- Criação/edição de receita: escrita `.tmp` + rename atômico sob o mesmo lock do
  watcher, seguida de sync/versão, eliminando corrida de upsert.
- Testes: banco saudável/ausente, corrupção com backup e falha fechada, e duas
  gravações concorrentes da mesma receita mantendo um único arquivo válido.

## 2026-11-25 — Melhorias v2, Fase 2: navegação por abas e Kanban

- `Execuções` e `Worktrees` unificados em `/runs` com abas; `/worktrees`
  redireciona para `/runs?tab=worktrees`.
- `Chat` e `Criador` agora são abas em `/chat` (`?tab=builder`), sem empilhar o
  Meta-Builder abaixo da conversa.
- `Buscar` e `Arquivos` unificados em `/search` (`?tab=files`); `/files`
  redireciona preservando `path`/`dir`. Deep link abre o arquivo na aba certa.
- Sidebar e Command Palette deixaram de listar Worktrees/Arquivos duplicados.
- Kanban ocupa 100% da altura: `PageBodyFill`, colunas `h-full` e scroll apenas
  dentro da coluna.
- Testes E2E novos/atualizados: abas consolidadas, redirects, deep links,
  worktrees, arquivos, scroll e Kanban. Goldens de kanban/chat/files/search/
  worktrees regravados.

## 2026-11-25 — Melhorias v2, Fase 3: UX de receitas e IconTag

- YAML do editor de receitas inicia fechado, com botão “Mostrar YAML” e painel
  lateral recolhível (`aria-label` alinhado ao texto visível).
- `RecipeStepper` ganhou recolher/expandir por nó e “Recolher todos”/“Expandir
  todos”; o cabeçalho mantém nome, tipo e resumo quando minimizado.
- Passos receberam borda lateral colorida por tipo (azul/âmbar/esmeralda/roxo).
- Novo `IconTag` acessível (tooltip, `aria-label`, foco por teclado) com mapas
  de política do roteador, origem e gatilho.
- `TaskCard`, `RoutineCard` e `RecipeListCard` trocaram tags verbosas por ícones
  com tooltip, mantendo horário/recorrência e contagem de execuções em texto.
- Testes: IconTag (a11y/tooltip), E2E de receitas com recolher/expandir e axe
  nos modais; Vitest **289**.

## 2026-11-25 — Melhorias v2, Fase 4: anexos clicáveis no chat

- Backend persiste `metadata_json.attachments` nas mensagens do usuário
  (`direction: "sent"`, `source: "ui"`) após salvar os arquivos em
  `input/chat`, mantendo o caminho no conteúdo enviado ao agente.
- Resposta do assistente inclui artefatos do `run_id` como anexos
  `direction: "received"`/`source: "agent"` (nome, caminho e tamanho).
- `Transcript` renderiza chips com direção (↑/↓), nome e tamanho; metadados
  inválidos são ignorados sem quebrar a conversa.
- Clique no chip busca `/api/files/content` e abre o `LazyFileViewer` em modal;
  arquivo removido mostra erro amigável.
- Teste Vitest do transcript cobre chips enviados/recebidos e JSON inválido;
  E2E de chat e e-mail verdes; `cargo test chat` **14** testes.

## 2026-11-25 — Melhorias v2, Fase 5: visualizador rico de arquivos

- Imagens ganharam `ImageZoomPane`: zoom de 25% a 600% por botões, roda e
  duplo clique, pan com pointer events, modo Ajustar/100%/200% e reset ao
  trocar de arquivo (sem drag fantasma da imagem).
- JSON/JSON Lines usa `JsonTreeView`: árvore colapsável, expandir/recolher
  tudo, contagem de nós, busca de chave/valor com destaque, cópia por nó e do
  JSON formatado, alternância Árvore/Código e aviso acima de 5.000 nós.
- CSV/TSV usa `CsvTableView` com parser de aspas/escapes, cabeçalho fixo,
  ordenação por coluna, filtro rápido, alternância Tabela/Bruto e paginação
  de 500 linhas.
- Markdown (`MarkdownPreview`) ganhou sumário lateral/popover a partir de 3
  títulos, cópia em cada bloco de código, cópia do markdown bruto e busca com
  contador/ocorrências reutilizando o visualizador de código.
- Código/log/texto usa `CodeTextView`: números de linha, busca com contador e
  anterior/próximo, quebra de linha e cópia; o toggle global virou
  Visualização/Fonte.
- `kindFor` reconhece `csv`/`tsv`; integração em `FilePreviewBody` mantém todos
  os arquivos do viewer abaixo de 500 linhas.
- Testes: `ImageZoomPane` (controles, roda, duplo clique, reset) e E2E
  `files-rich-viewers` (JSON, CSV, Markdown, código e imagem).

## 2026-11-25 — Melhorias v2, Fase 6: simetria de importação e exportação

- Envelope único `{schema_version, kind, exported_at, source_workspace, items}`
  com validação fail-closed de versão/kind em todos os imports.
- Contexto pessoal: `GET /api/personal-context/export` e
  `POST /api/personal-context/import` (`merge`/`replace`), com botões simétricos
  no cartão.
- Memória estruturada: `GET /api/memory/export` e `POST /api/memory/import`
  recriando namespaces, ACLs e notas; cartão ganhou Importar ao lado de
  Exportar.
- Prompts: `GET /api/prompt-templates/export` e
  `POST /api/prompt-templates/import`, rejeitando chaves desconhecidas antes de
  gravar.
- Pacotes: export individual em `/api/starter-packs/{id}/export`, import em
  `/api/starter-packs/import`, lote em `/api/bundles/packs/export|import`.
  Import valida manifesto, caminhos e conteúdo de agentes, comandos, skills,
  receitas e rotinas antes de escrever.
- Workspaces: `/api/bundles/workspaces/export|import` leva definição, seções de
  configuração, receitas e rotinas; imports reutilizam o workspace existente
  por id/nome ou criam um novo na raiz padrão.
- Frontend: helper `import-export.ts`, botão reutilizável `JsonImportButton` com
  diálogo único Mesclar/Substituir e `PacksBundleActions`.
- Testes: suíte Rust de bundles/contexto/memória/prompts e E2E
  `import-export-ui` (kind incompatível + import aplicado).

- Goldens de `packs` (700/1024/1100) regravados pelos novos botões de
  exportação/importação no cabeçalho.

## 2026-11-25 — Melhorias v2, Fase 7: backup e restore completo

- Formato v2 grava `state/workspace-state.json` (com fallback de leitura para
  `db/workspace_state.json`), `state_sha256` no manifesto e SHA-256 por arquivo.
- Snapshot inclui todas as tabelas com `workspace_id`; colunas de credenciais
  em texto plano (`sync_token`, tokens/senhas) são zeradas no estado. Segredos
  portáveis continuam apenas no `secrets.enc` protegido por senha.
- Restore `replace_in_place` (alias `in_place`) limpa todas as tabelas
  workspace-scoped em transação única, valida chaves estrangeiras, faz backup
  de segurança antes e troca arquivos por staging atômico. `new_workspace`
  remapeia ids, paths e FKs.
- Extração fail-closed: caminho traversal, symlink e divergência de SHA-256
  abortam antes de tocar no workspace ou no banco.
- Novo `POST /api/backups/import` importa `.zip` após dry-run completo
  (manifesto, arquivos e snapshot) e registra no catálogo local.
- `BackupsCard` ganhou **Importar .zip**, contagem de registros de banco por
  backup, versão e rótulos explícitos dos dois modos de restauração.
- Teste round-trip restaura arquivo, tarefa, rotina, receita e contexto pessoal
  idênticos, além do teste de importação e rejeição de ZIP truncado.

## 2026-11-25 — Melhorias v2, Fase 8: STT/TTS reais e microfone universal

- TTS agora retorna bytes de áudio em `POST /api/voice/tts` com
  `Content-Type` e `X-Voice-Provider`, sem tocar áudio no servidor.
  Provedores em ordem configurável: Piper, espeak-ng/espeak, endpoint
  OpenAI-compatible (`/v1/audio/speech`, `secret_ref`) e comando custom.
- STT em `POST /api/voice/stt`: whisper.cpp local com conversão automática
  webm/ogg → WAV 16 kHz mono via ffmpeg, ou endpoint OpenAI-compatible
  (`/v1/audio/transcriptions`). Áudio abaixo de 1 KB é rejeitado.
- Processos de voz usam `kill_on_drop`, process group e timeout rígido de 60s.
- Sem provedor, a API responde **503** com `error` e `install_hint` acionável.
- Novos hooks compartilhados: `use-voice-input.ts` (MediaRecorder, tempo,
  transcrição, liberação de tracks) e `use-voice-output.ts` (`Audio` +
  Object URL revogado no fim/erro).
- Novo `VoiceDictationButton` com estados gravando/transcrevendo, erro de
  permissão e tooltip; integrado em Chat, tarefa (título e descrição), Quick
  Capture e compositor de execução.
- `VoiceCard` ganhou provedores/endpoints/secret_refs e botões **Testar fala
  (TTS)** e **Testar transcrição (STT)**.
- Removido `src/pages/chat/useVoiceInput.ts` em favor do hook compartilhado.
- Testes: hooks/helpers de voz, botão acessível, TTS custom com bytes RIFF e
  E2E de configurações/formulário.

## 2026-11-25 — Melhorias v2, Fase 9: Ferramentas (Scripts como MCP)

- Nova migration `0003_script_tools.sql` com escopo global/workspace, slug único,
  schema de entrada, `secret_refs`, timeout, aprovação HITL e enabled.
- Repositório CRUD completo em `opendesk-storage/repos/script_tools.rs`.
- Executor seguro em `script_tools.rs`:
  - slug/runtime/script validados; input conferido contra JSON Schema;
  - `env_secret_refs_json` só aceita referências existentes no SecretStore;
  - arquivo temporário `0700`, `cwd` na raiz do workspace, stdin JSON e
    `OPENDESK_INPUT_JSON`;
  - `kill_on_drop`, process group e timeout de 1–300s, com stdout/stderr
    limitados a 1 MiB.
- Novo MCP stdio `opendesk mcp-scripts`, registrado automaticamente como
  `mcp.servers.opendesk-scripts`; expõe `script_<slug>` e executa via executor.
  Ferramentas com `require_approval` são recusadas até aprovação explícita.
- REST: lista/cria/edita/exclui, `/test`, `/export` e `/import` com envelope
  validado; resumo das ferramentas ativas entra no prompt de tarefas/preview.
- Frontend: `ScriptToolsCard` + `ScriptToolModal` com abas Script, Parâmetros,
  Prompt para o Agente e Segredos & Limites, teste inline de stdout/stderr/exit,
  toggle e export/import simétricos.
- Testes: CRUD, schema, timeout, import, `tools/list`/`tools/call` reais e
  rejeição de segredo em texto plano; E2E de criação/teste na UI.

## 2026-11-25 — Melhorias v2, Fase 10: Testar tudo e gate final

- Novo `self_test.rs` com etapas determinísticas: banco, motor, tarefa,
  rotina, receita, script MCP, voz e limpeza sempre executada.
- Modo `fast` funciona offline e não consome tokens; `full_ai` executa tarefa e
  receita mínimas com o motor quando online.
- Guarda `AtomicBool` impede execuções simultâneas (**409**).
- Limpeza remove tarefa, rotina, receita (arquivo + índice + runs) e script
  temporários com prefixo `__selftest_*`.
- `POST /api/diagnostics/self-test` classificado como rota sensível.
- `SelfTestCard` em Configurações → Diagnóstico com checklist, tempos, botões
  rápido/com IA e cópia do relatório JSON.
- Teste Rust valida modo rápido offline e zero resíduos no banco e no disco;
  E2E cobre o fluxo na UI.
- Gate final (`./scripts/gate.sh`) passou com 111 testes Playwright, 49
  goldens, cobertura frontend 98,55% linhas e 90,2% branches. Ajustes de
  compatibilidade: título da aba Arquivos, YAML aberto no E2E de receitas e
  separação do botão de ditado do label do título da tarefa.

## 2026-11-27 — Plano v3, F1: layout full-width, headers estáveis e botões

- `PageBody` perdeu o teto de 1440px e o cabeçalho passou a usar gutter
  uniforme `12px`/`16px`, com `PageHeader` de altura mínima estável e descrição
  truncada.
- `PageBodyFill` ocupa largura/altura totais com padding uniforme.
- OpenCode Studio usa cabeçalho compacto e barras de abas mais enxutas.
- Dashboard moveu `IntegrationFootprint` para imediatamente após
  `CostMixCard`.
- Chat/Criador e lista de conversas passaram a usar a altura disponível, sem
  cards de altura fixa.
- Botões ganharam `shrink-0` e padding horizontal mínimo por variante; o botão
  “Criar arquivo” do MetaBuilder ganhou largura/padding seguros.
- `DESIGN.md` atualizado para o contrato full-width.
- Goldens visuais regravados para o novo layout; scroll e a11y verdes.

## 2026-11-27 — Plano v3, F2: dropdown do Chat e status visual unificado

- `SlashTextarea` agora mede o viewport e renderiza o dropdown de `/`, `@` e `#`
  em portal fixo: abre para cima perto da base, limita a altura, rola com
  `overscroll-contain` e mantém o item ativo visível por teclado.
- `WorkingIndicator` usa `Loader2` girando; removidos `Cog`, `.opendesk-gear` e
  os keyframes da engrenagem.
- `RunStatusIcon`/`WorkflowStatusBadge` foram alinhados à tabela canônica:
  spinner para execução, relógio para fila, aprovação/HITL, pausa, sucesso,
  falha e cancelamento com `Ban`.
- Ícones de status aplicados em Runs (tabela e lista), Dashboard, ActivityFeed,
  fila, Kanban via badge de workflow e calendário.
- `prefers-reduced-motion` mantém o spinner visível em velocidade reduzida.
- Testes novos cobrem dropdown acima do campo e estados de status; Vitest
  **324** testes.

## 2026-11-27 — Plano v3, F3: subabas estruturais e Arquivos em altura total

- Buscar/Arquivos/Artefatos consolidados em `/search` com um único cabeçalho:
  `?tab=files` e `?tab=artifacts`; `/artifacts` redireciona preservando
  `?artifact=`/`?id=`.
- `ArtifactsTabContent` extraído para reutilização sem `PageHeader` duplicado.
- Arquivos embedded ocupam altura total com scroll independente na árvore e no
  preview.
- Sidebar/Command Palette deixaram de listar Artefatos como página separada;
  links `artifact` agora apontam para a aba canônica.
- Governança dividida em Agentes, Metas, Delegações, Handoffs e Performance.
- Aprendizado dividido em Lições & Loop, Contexto Pessoal, Memória Estruturada e
  Avaliação de Skills.
- Roteador dividido em Políticas & Modelos, Regras, Budget Guard e Análise de
  Custos.
- E2E atualizados para navegar pelas novas subabas; scroll, a11y, artifacts,
  files-search, governance, learning e router verdes.
- Goldens de artifacts/files/governance/learning/router regravados.

## 2026-11-27 — Plano v3, F4: política única de importar/exportar

- `FilePreferences` ganhou `importExportAsk` (default `true`), persistido em
  `import_export_ask` e editável em Configurações → Geral.
- Novo `saveExportedData` / `pickImportFile` em `import-export.ts`:
  - Tauri: diálogo nativo de salvar ou gravação direta na pasta configurada;
  - fallback para diálogo se a pasta sumir;
  - browser/PWA: download nativo e input file;
  - cancelamento não gera sucesso falso.
- Novo endpoint `POST /api/files/save-bytes` com validação de destino em
  `$HOME`/temporário, escrita atômica e limite de 128 MB.
- `exportJsonBundle`, Config Bundle, exports de runs, downloads autenticados e
  backups ZIP passam pela mesma política.
- Botões de exportação só exibem toast de sucesso quando o arquivo foi salvo.
- `pickJsonBundle` reutiliza o seletor unificado de importação.

## 2026-11-27 — Plano v3, F5: multi-bot por workspace e contexto isolado

- Migration `0004_channel_bots_and_memory_scope.sql` cria `channel_bots` e
  `channel_chat_contexts`, com escopo `normal | chat | both` em memórias e
  contexto pessoal (preparando a F7).
- `repos/channel_bots.rs` cobre CRUD por workspace, contextos por conversa,
  auto-registro de chats vistos e status dos bots.
- `BotSupervisor` mantém um handle isolado por bot:
  - Telegram com long polling, offset e status próprios;
  - Signal com processo `signal-cli` e handler isolados;
  - falha de um bot não altera os demais;
  - shutdown cancela todos os handles.
- Migração suave: configuração global legada de Telegram/Signal é semeada em
  `channel_bots` no primeiro boot, evitando perder o bot atual.
- Mensagens são roteadas por `(bot_id, external_chat_id)`:
  - threads/sessões distintas por grupo;
  - `agent_override` e `system_context` do grupo entram no prompt;
  - `memory_scope` do grupo é propagado;
  - contexto desabilitado ignora a mensagem sem executar o agente.
- API: `/api/channels/bots` e `/api/channels/bots/{id}/contexts`, com
  `start`, `stop` e `test`; tokens ficam apenas no `SecretStore` e nunca são
  devolvidos no JSON.
- Rotas legadas de Telegram/Signal continuam operando quando não há bot novo.

## 2026-11-27 — Plano v3, F6: UI de bots por workspace e contextos por grupo

- Tipos `ChannelBot` e `ChannelChatContext` adicionados ao contrato TypeScript.
- `ChannelBotsPanel` reutilizável:
  - cabeçalho com workspace ativa e resumo de bots em outras workspaces;
  - lista com status ao vivo, token, agente, iniciar/parar, testar, contextos,
    editar e excluir;
  - formulário de criação/edição com token write-only, allowlist, agente padrão
    e opções específicas de Signal (conta e caminho do CLI);
  - recarrega ao trocar de workspace, sem vazar bots da workspace anterior.
- `ChannelBotContextsModal`:
  - grupos detectados automaticamente + inclusão manual por ID;
  - instruções exclusivas, agente do grupo, escopo de memória
    (`Chat | Normal | Ambos | Nenhuma`), ativação e remoção;
  - reinício da sessão apenas do grupo selecionado.
- `TelegramCard`, `SignalCard` e `ChannelsCard` incorporam o painel; a API de
  listagem aceita `all=true` para o resumo entre workspaces.
- Testes: 4 Vitest do painel e E2E próprio de bots/contextos; integrações e a11y
  continuam verdes.

## 2026-11-27 — Plano v3, F7: escopo de memória normal/chat/both

- `MemoryUsageScope` (`normal | chat | both`) adicionado ao domínio com parsing
  estrito e fail-closed.
- Migration 0004 já aplica `usage_scope` em `memory_notes` e
  `personal_context_groups`; registros legados ficam em `both`.
- Repositórios de memória filtram por escopo (`normal/both` ou `chat/both`) e
  aceitam `usage_scope` em create/update/list/search.
- Contexto pessoal ganhou `usage_scope` em create/update e renderização
  separada para execuções normais e chat.
- Prompts:
  - tarefas, rotinas, receitas e previews normais usam apenas `normal/both`;
  - chat, Telegram, Signal e preview de chat usam apenas `chat/both`;
  - grupo com `memory_scope=none` não injeta contexto pessoal nem memória.
- API HTTP aceita/retorna `usage_scope`; create pela UI/API usa `normal` por
  padrão e imports legados usam `both`.
- MCP `opendesk-control` ganhou `memory_create/list/update/set_scope` e
  `personal_context_list/create/update/set_scope`, com validação estrita e
  erro JSON-RPC `-32602` para escopo inválido.
- UI de Aprendizado ganhou filtros Todos/Normal/Chat/Ambos e seletor inline com
  salvamento imediato em contexto pessoal e memória estruturada.
