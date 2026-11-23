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
