# Arquitetura OpenDesk V2

Documento único da arquitetura. Produto e requisitos ficam em `SPEC.md`; decisões
históricas da V1 em `docs/archive/v1/decisions/`.

## 1. Princípios

1. **SSOT no disco.** Workspaces, `opencode.jsonc`, agentes, skills, comandos e
   MCP vivem em arquivos nativos. O SQLite guarda estado operacional.
2. **Loop delegado.** O OpenDesk governa; o daemon OpenCode V2 executa.
   Nenhuma ferramenta é reimplementada.
3. **V2-only.** `/api/*`, Basic auth, `permissions`, `commands`, `mcp.servers`,
   `disabled`, `type: "remote"`.
4. **Entrega determinística.** Toda automação escreve em
   `<workspace>/output/YYYY-MM/<slug>/` e permanece rastreável.
5. **Segredos fora do modelo.** Valores só no `SecretStore`; configurações e
   banco carregam `secret_ref`.
6. **Fronteiras explícitas.** Domínio puro; engine sem canais; shell fino.

## 2. Crates (8)

```
opendesk-domain      IDs, estados, erros, eventos, ports          (sem SQL/HTTP/fs/Tauri)
      ▲
opendesk-storage     baseline STRICT, pools, repos, write-buffer
      ▲
opendesk-engine      queue, scheduler, runner, telemetry, router, worktree, adapters
opendesk-recipes     parser, dag, hitl, fachada do motor
      ▲
opendesk-mcp         config, pool, transportes, RBAC/rate/audit
opendesk-channels    telegram/whatsapp/email/signal/webpush + NotificationRouter
opendesk-enterprise  ent-auth, ent-cluster, ent-computer, ent-eval
      ▲
opendesk             shell Tauri: tray, webview, auto-attach, diálogos
```

Bins: `opendesk` (shell), `opendeskd` (daemon headless), `opendesk-cli`,
`opendesk-node`.

### Regras de dependência

- `domain` não depende de nenhum outro crate do workspace.
- `storage` → `domain`.
- `engine`/`recipes` → `domain`, `storage`; **nunca** `channels`/`enterprise`.
- `mcp`/`channels`/`enterprise` → `domain` (+ traits de `engine`).
- O shell orquestra; não contém regra de domínio.

### Estado da migração

- **Onda 1 (feita):** `domain`, `storage`, baseline `0001_v2_baseline.sql`,
  `conversations`+`messages`, pools, write-buffer, `migrate-v1-export`.
- **Onda 2:** `engine`, `recipes` e decomposição dos god-files.
- **Onda 3:** `mcp`, `channels`, desacoplamento verificado.
- **Onda 4:** `enterprise`, extinção de `opendesk-core`, shell fino, frontend,
  doctor e gate de limites.

## 3. Fluxos

### Prompt → execução

1. UI/API/MCP criam a tarefa e enfileiram (`execution_queue`).
2. `engine::queue` respeita concorrência, lease e retry; o cron enfileira por
   `next_run_at`.
3. `engine::runner` abre/retoma a sessão OpenCode, consome SSE, grava eventos,
   uso, diffs e artefatos.
4. Settlement grava `task_runs`, atualiza a tarefa e emite eventos de domínio.
5. Delivery e sinks rodam depois do run, com idempotência própria.

### Recorrência

`recipes`/`schedules` calculam a próxima ocorrência com timezone, política de
misfire (`RUN_IMMEDIATELY`/`SKIP_TO_NEXT`) e modo seguro somente-leitura. O
scheduler dorme até `next_run_at` e acorda por `Notify` em inserções/edições.

### Receitas e HITL

O YAML em `.opencode/recipes/` é a verdade; o índice no SQLite espelha
metadados. Steps usam a mesma fila/sessão. HITL persiste como mensagem
(`kind=hitl_request`) com token one-shot; o primeiro canal que responde vence.

### Canais

`ChannelAdapter` normaliza entrada/saída, anexos, threads e `dedupe_key`.
`NotificationRouter` decide canal por severidade/quiet hours. O engine emite
apenas `NotificationIntent`.

## 4. Storage

### Baseline

- `crates/opendesk-storage/migrations/0001_v2_baseline.sql` é a única migration.
- 61 tabelas, todas `STRICT`; tipos normalizados (`BOOLEAN`→`INTEGER`,
  `JSON`→`TEXT`).
- Seed: `budget_settings(1)` e os harnesses embutidos.

### Conexões

- **Writer:** 1 conexão, transações serializadas (`BEGIN IMMEDIATE`).
- **Reader:** N conexões com `query_only=1`.
- **Write-buffer:** heartbeats/presença em lote (coalescing por chave, flush
  250 ms).
- Pragmas por conexão: WAL, `synchronous=NORMAL`, `busy_timeout=5000`,
  `cache_size=-64000`, `foreign_keys=ON`. Testados em
  `pool::tests::connection_pragmas_match_the_contract`.

### Conversas

`conversations` + `messages` substituem `chat_threads`, `chat_messages`,
`chat_turns`, `task_messages` e `task_interactions`. Cursor por `id` (UUIDv7),
sem `OFFSET`. Turnos guardam tokens/custo/erro em `payload_json`; HITL guarda
pergunta/opções/decisão.

### Clean break

O V2 recusa abrir um banco V1 (`_sqlx_migrations` sem `v2_meta`) e orienta
`opendesk-cli migrate-v1-export --db <v1.db> --out <dir>`, que exporta
workspaces, receitas e settings em JSON/YAML com manifesto SHA256, sem segredos.

## 5. Segredos

`SecretStore` (AES-256-GCM, arquivos `0600`, keyring planejado) resolve
`secret_ref` no momento do uso. Webhooks exigem HMAC em tempo constante; IMAP
sempre valida certificado. Nenhum token vai para prompt, log, SSE ou
`config_json`.

## 6. Eventos e tempo real

- Envelope WS `{ event_id, event_type, payload }`.
- Ao perder eventos (`Lagged`), o servidor emite `ResyncRequired` e o cliente
  refaz o snapshot.
- `DomainEvent` cobre mudanças de tarefa/run/HITL/mensagem; transporte
  (WS/push/canais) é assinante.

## 7. Shell e frontend

O desktop é casca: tray, janela, auto-start/attach ao `opendeskd`, diálogo de
pasta e deep links. O frontend React (20 páginas) consome a API local e o WS;
telas densas usam virtualização/memoização no streaming.

## 8. Verificação

- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test -p opendesk-domain -p opendesk-storage`.
- Limite de 500 linhas e ausência de `unwrap/expect` fora de testes (gate V2).
- `scripts/gate.sh` (Vitest, Playwright, goldens, cobertura no piso acordado).
- Sem release durante a refatoração; release só sob pedido.
