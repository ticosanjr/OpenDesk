# Plano de melhoria — estabilização operacional do OpenDesk

**Base:** 1.8.0 (Vault, DAG UI, canais Discord/Slack/WhatsApp QR e I/O Bus unificado)
**Data:** 2026-11-24

**TL;DR**
A base já está na `1.8.0` com Vault, DAG UI, canais Discord/Slack/WhatsApp QR e I/O Bus unificado. Os problemas encontrados vêm de **5 falhas estruturais concretas** (mais 3 riscos ocultos encontrados na auditoria): (1) links internos e notificações apontam para `/kanban` sem especificar `?view=queue&job=<id>` porque a Fila vive como aba dentro de `KanbanPage.tsx`; (2) mostradores e o badge da fila no topo só atualizam ao trocar de página porque o `app-store.ts` não reage a todos os eventos WebSocket nem faz resync após `RecvError::Lagged` ou reconexão, e `isWorkQueueJob` ainda esconde jobs de automação; (3) a fila parece rodar 1 tarefa por vez por serialização no worker/quota padrão de agente/grupos de concorrência sem exibir `wait_reason` nem limite máximo da fila; (4) o backup atual de workspace (`WorkspaceBackup`) cobre arquivos sem garantir restauração atômica de 100% das tabelas SQLite + `runs/*.jsonl` + segredos portáveis; e (5) falta um pacote JSON único de exportação/importação rápida de memórias, prompts e configurações operacionais. Abaixo está a especificação atômica pronta para o OpenCode (DeepSeek v4).

---

## Auditoria de Problemas Potenciais e Erros Ocultos Encontrados

| # | Problema / Risco | Origem no Código | Impacto Real |
| --- | --- | --- | --- |
| **1** | **Links quebrados / aba errada** | `src/lib/links.ts`, `use-app-notifications.ts`, `Topbar.tsx`, `AttentionCard.tsx`, `calendar_agenda/items.rs`, `search/all.rs` | Notificação de fila abre `/kanban` (board) em vez da aba Fila; links de Settings/Integrations/Studio/Learning perdem aba ou item focado. |
| **2** | **Contadores travados até trocar de página** | `src-tauri/src/api/ws.rs`, `src/store/app-store.ts`, `Topbar.tsx`, `Sidebar.tsx`, `DashboardPage.tsx` | Mutações de fila/tarefas/runs/artefatos não disparam atualização global reativa; `RecvError::Lagged` no `broadcast` do WS descarta eventos silenciosamente sem forçar resync. |
| **3** | **Filtro `workQueue` escondendo jobs reais** | `src/lib/queue.ts` (`isWorkQueueJob` filtra apenas tarefas manuais) | Jobs de rotinas, receitas, IO Bus e delegações ocupam slots reais da fila, mas somem do contador do topo ou da visão de fila. |
| **4** | **Execução serializada (1 por vez) + fila sem teto** | `queue_manager/worker.rs`, `repos/queue.rs`, `api/queue.rs`, `QueueSettingsCard.tsx`, `AgentProfile.concurrency_quota` | `max_concurrency`, quotas por agente/grupo e claim sequencial limitam paralelismo real; falta `max_queue_size` e diagnóstico `wait_reason` explicando por que cada job aguarda. |
| **5** | **Race condition na troca de workspace** | `src/store/app-store.ts` e fetches em páginas sem `AbortController` / token de sequência | Trocar de workspace rapidamente pode sobrescrever os contadores novos com resposta atrasada do workspace anterior. |
| **6** | **Cache do Studio obsoleto** | `src/lib/studio-cache.ts` | Agentes, skills e saúde de modelos criados/alterados via API ou MCP não atualizam nos seletores sem recarregar ou expirar TTL. |
| **7** | **Backup incompleto / não portável entre máquinas** | `src-tauri/src/backup_create.rs`, `backup_manifest.rs`, `backup_restore.rs`, `BackupsCard.tsx` | Risco de perder estado relacional SQLite (memórias, tarefas, chats, prompts, rotinas, governança, rotas), logs `runs/*.jsonl` fora do `root_path` e segredos atrelados à chave da máquina antiga. |
| **8** | **Ausência de export/import JSON rápido** | Sem endpoint unificado para memórias + prompts + configs | Configurar um novo workspace ou replicar setup pessoal/empresa exige recriar memórias, contexto pessoal, templates de prompt, roteador e governança manualmente. |

---

## 1. Objetivo Final e Escopo

**Resultado Esperado:**

1. **Navegação 100% determinística:** todo link interno (toasts, notificações nativas, Web Push, Topbar, Attention Card, Busca global, Calendário, Kanban, Runs) leva exatamente à rota, aba/view e item focado corretos (ex.: fila → `/kanban?view=queue&job=<id>`).
2. **Contadores e mostradores "online" em tempo real:** badge da fila no Topbar, contadores de Atenção, Dashboard, colunas do Kanban, Runs, Artefatos e Governança atualizam instantaneamente via WebSocket + resync automático em reconexão/lag, sem trocar de página e sem race condition ao alternar workspace.
3. **Fila concorrente real e observável:** múltiplos jobs executam em paralelo conforme `max_concurrency` persistido, com limites configuráveis por grupo e por agente, controle de tamanho máximo da fila (`max_queue_size`), e exibição explícita do motivo de espera (`wait_reason`) e slots ocupados (`ativos/total`).
4. **Backup e Restauração 100% completos do workspace:** ZIP versionado (v2) contendo arquivos do workspace + dump JSON completo de todas as tabelas SQLite do workspace + `runs/*.jsonl` + opção de segredos cifrados com senha do usuário (Argon2id + AES-256-GCM), com restauração atômica (*Novo Workspace* com remapeamento de IDs ou *In-Place* com snapshot prévio e rollback automático em falha).
5. **Importação e Exportação JSON rápida:** export/import seletivo em JSON único (`schema_version: 1`) de todas as memórias (`MEMORY.md`, structured memory + ACLs, personal context, lessons), prompts (templates globais/workspace + assets opcionais do Studio) e configurações (router, budget, queue, governance, harnesses, sandbox, voice, rotas de canais/IO Bus sem segredos em texto claro), com prévia de conflitos (dry-run) e escolha de estratégia de merge.

**FORA de Escopo (NÃO TOCAR):**

* Alterar o protocolo de comunicação OpenCode V2 (`docs/protocol/opencode-v2.md`).
* Reescrever a lógica de execução DAG de `opendesk-recipes/src/dag/`.
* Alterar a arquitetura de autenticação Passkey/WebAuthn ou mTLS.
* Instalar dependências npm ou crates pesados desnecessários (usar primitivos já presentes no workspace Cargo e `src/components/ui/`).
* Quebrar baselines de tamanho de arquivo (`scripts/check-rust-limits.py` e `scripts/check-frontend-limits.py`).

---

## 2. Mapa de Arquivos

| Arquivo | Ação | Motivo |
| --- | --- | --- |
| `src/lib/links.ts` | Ler / Modificar | Centralizar todas as URLs canônicas com suporte explícito a `view`, `tab`, `section`, `job`, `task`, `run`, `artifact`, `routine`, `recipe`. |
| `src/lib/links.test.ts` | Modificar | Garantir cobertura de 100% das rotas e query params contra as rotas válidas de `src/App.tsx`. |
| `src/components/layout/app-shell/use-app-notifications.ts` | Ler / Modificar | Corrigir `href` de todos os toasts e notificações nativas (fila → `links.queue(jobId)`, run → `links.run(runId)`, interação → `links.task(taskId)`, etc.). |
| `src/pages/KanbanPage.tsx` | Ler / Modificar | Sincronizar `?view=board\|list\|queue`, `?job=<id>` e `?task=<id>` via `useSearchParams` (reagindo também a mudanças de URL enquanto já está em `/kanban`). |
| `src/components/queue/QueueController.tsx` | Ler / Modificar | Destacar `data-focus-id={job.id}`, exibir slots `ativos/máx`, seletor rápido de concorrência, filtro de origem (`todas` vs `manuais`) e badge `wait_reason`. |
| `src/components/layout/Topbar.tsx` | Ler / Modificar | Linkar indicador da fila para `links.queue()`, consumir contadores reativos do `app-store` (total real de jobs ativos + aguardando). |
| `src/components/layout/Sidebar.tsx` | Ler / Modificar | Exibir badges ao vivo (Atenção, Fila/Kanban, Runs em andamento) lendo direto do `app-store`. |
| `src-tauri/src/api/calendar_agenda/items.rs` | Ler / Modificar | Garantir que `CalendarItem.href` para itens `queued` aponte para `/kanban?view=queue&job=<id>` e demais itens sigam o mesmo contrato de `links.ts`. |
| `src-tauri/src/api/search/all.rs` | Ler / Modificar | Auditar e padronizar `href` de todos os `SearchHit`. |
| `src-tauri/src/api/ws.rs` | Ler / Modificar | Em `RecvError::Lagged`, enviar evento `{"event_type":"sync.required","payload":{"reason":"lagged"}}` em vez de apenas logar e seguir. |
| `src-tauri/crates/opendesk-storage/src/events.rs` | Ler / Modificar | Garantir payload padronizado com `workspace_id` em todos os eventos de mutação (queue, tasks, runs, artifacts, attention, schedules, recipes, memory, studio). |
| `src-tauri/src/api/queue.rs` | Ler / Modificar | Expor `max_concurrency`, `max_queue_size`, `group_limits`, `active_slots`, `wait_reason` por job; emitir evento WS em toda mutação. |
| `src-tauri/crates/opendesk-engine/src/queue_scheduler/queue_manager/worker.rs` | Ler / Modificar | Corrigir loop do worker para preencher **todos** os slots livres em paralelo (`tokio::spawn` / `JoinSet` até `max_concurrency`), respeitando limites por grupo e quota por agente. |
| `src-tauri/crates/opendesk-engine/src/queue_scheduler/queue_manager/mod.rs` | Ler / Modificar | Carregar `max_concurrency`, `max_queue_size` e `group_limits` persistidos no boot; notificar `tokio::sync::Notify` imediatamente ao enfileirar ou aumentar concorrência. |
| `src-tauri/crates/opendesk-storage/src/repos/queue.rs` | Ler / Modificar | Aplicar validação fail-closed de `max_queue_size` no enqueue; retornar diagnóstico `wait_reason` na listagem; corrigir claim concorrente atômico para múltiplos slots. |
| `src/components/settings/QueueSettingsCard.tsx` | Ler / Modificar | Adicionar configuração persistida de `max_concurrency`, `max_queue_size`, limites por `concurrency_group` e comportamento ao atingir teto da fila. |
| `src/store/app-store.ts` & `src/store/app-store-types.ts` | Ler / Modificar | Implementar barra de estado "online" reativa (`queue`, `queueStatus`, `attentionCount`, `runningRuns`, `stats`, `revisions`), resync no reconnect/`sync.required`/`visibilitychange`, invalidação de `studio-cache` e guarda de sequência contra race condition de workspace. |
| `src/lib/queue.ts` | Ler / Modificar | Separar `allQueueCounts(jobs)` (usado no Topbar e limites reais do motor) de `workQueue(jobs)` (filtro visual opcional no Kanban). |
| `src-tauri/src/backup_manifest.rs` | Ler / Modificar | Evoluir manifesto para `version: 2` com `db_tables`, `run_logs`, `secrets_encrypted: bool`, `files_sha256` e `counts`. |
| `src-tauri/src/backup_create.rs` | Ler / Modificar | Incluir no ZIP: todos os arquivos do workspace + `db/workspace_state.json` (todas as tabelas do workspace) + `db/run_logs/*.jsonl` + `db/secrets.enc` opcional (cifrado por senha). |
| `src-tauri/src/backup_restore.rs` | Ler / Modificar | Implementar restauração 100% atômica em 2 modos (`new_workspace` com remapeamento de IDs e `in_place` com pré-backup + rollback), validando SHA-256 e bloqueando zip-slip. |
| `src-tauri/src/api/backups.rs` & `src/components/settings/BackupsCard.tsx` | Ler / Modificar | Expor opções de backup (incluir logs, incluir segredos com senha) e restauração (`new_workspace` vs `in_place`, relatório detalhado pós-restore). |
| `src-tauri/src/config_bundle.rs` | Criar | Motor de exportação, preview (dry-run) e importação JSON (`schema_version: 1`) de memórias, prompts e configurações operacionais. |
| `src-tauri/src/api/config_bundle.rs` | Criar | Endpoints `POST /api/config-bundle/export`, `/preview`, `/import`. |
| `src-tauri/src/api/router_table/system.rs` | Ler / Modificar | Registrar rotas `/api/config-bundle/*`. |
| `src-tauri/src/cli/commands.rs` & `args.rs` | Ler / Modificar | Adicionar `opendesk config export <arquivo.json>` e `opendesk config import <arquivo.json> [--mode merge-skip\|merge-overwrite\|replace]`. |
| `src/components/settings/ConfigBundleCard.tsx` | Criar | UI de exportação/importação JSON rápida com seleção por categoria, prévia de conflitos e download/upload. |
| `src/components/learning/StructuredMemoryCard.tsx` & `src/components/settings/PromptTemplatesCard.tsx` | Ler / Modificar | Adicionar botões de atalho "Exportar JSON" / "Importar JSON" chamando a API unificada filtrada para memórias/prompts. |

---

## 3. Contratos, Schemas e Invariantes

### 3.1. Contrato Canônico de Deep-Links (`src/lib/links.ts` + Backend)

```ts
export const links = {
  dashboard: () => "/",
  kanban: (taskId?: string) => taskId ? `/kanban?view=board&task=${encodeURIComponent(taskId)}` : "/kanban?view=board",
  queue: (jobId?: string) => jobId ? `/kanban?view=queue&job=${encodeURIComponent(jobId)}` : "/kanban?view=queue",
  task: (taskId: string) => `/kanban?view=board&task=${encodeURIComponent(taskId)}`,
  run: (runId: string) => `/runs/${encodeURIComponent(runId)}`,
  runs: (scope?: string) => scope ? `/runs?scope=${encodeURIComponent(scope)}` : "/runs",
  artifact: (artifactId: string) => `/artifacts?artifact=${encodeURIComponent(artifactId)}`,
  routine: (scheduleId: string) => `/routines?routine=${encodeURIComponent(scheduleId)}`,
  recipe: (recipeId: string) => `/recipes?recipe=${encodeURIComponent(recipeId)}`,
  recipeRun: (runId: string) => `/recipes/runs/${encodeURIComponent(runId)}`,
  chat: (threadId?: string) => threadId ? `/chat?thread=${encodeURIComponent(threadId)}` : "/chat",
  studio: (tab: "agents" | "skills" | "commands" | "mcp" | "config", id?: string) =>
    id ? `/studio?tab=${tab}&focus=${encodeURIComponent(id)}` : `/studio?tab=${tab}`,
  settings: (section: string) => `/settings?section=${encodeURIComponent(section)}`,
  integrations: (section?: string) => section ? `/integrations?section=${encodeURIComponent(section)}` : "/integrations",
  learning: (tab?: string) => tab ? `/learning?tab=${encodeURIComponent(tab)}` : "/learning",
  governance: (section?: string) => section ? `/governance?section=${encodeURIComponent(section)}` : "/governance",
};
```

**Invariante:** Toda página com abas/seções (`KanbanPage`, `SettingsPage`, `IntegrationsPage`, `StudioPage`, `LearningPage`, `GovernancePage`) **DEVE** reagir à alteração dos query params em `useEffect([searchParams])` — não apenas no estado inicial `useState(...)` — para que clicar em um toast enquanto já está na mesma rota troque a aba imediatamente.

### 3.2. Contrato de Fila Concorrente e Backpressure

```rust
pub struct QueueConfig {
    pub max_concurrency: usize,                    // min 1, max 32, default 3 (persistido em app_settings)
    pub max_queue_size: usize,                     // min 1, max 10_000, default 200 (0 proibido; fail-closed)
    pub default_agent_concurrency: usize,          // min 1, default 2 (não travar em 1 por padrão)
    pub group_limits: HashMap<String, usize>,      // ex: local_llm_heavy: 1, web_scraper: 2, light_api: 6, default: 3
}

pub enum QueueWaitReason {
    Ready,                      // aguardando próximo tick/slot livre
    QueuePaused,                // fila pausada manualmente ou por Budget Guard
    GlobalSlotsFull { active: usize, max: usize },
    GroupSlotsFull { group: String, active: usize, max: usize },
    AgentQuotaFull { agent: String, active: usize, max: usize },
    ScheduledFuture { at: String },
    TaskHeld,
}
```

**Invariantes Fail-Closed da Fila:**

1. Ao enfileirar, se `waiting_count + running_count >= max_queue_size`, retornar `429 Too Many Requests` (`QUEUE_FULL`) com contagem atual e limite — nunca descartar job silenciosamente.
2. O loop do worker (`worker.rs`) usa `tokio::sync::Notify` + `JoinSet`: a cada wake/tick, faz `claim_next_eligible` em loop enquanto `active_tasks.len() < max_concurrency` e existir job elegível cujos limites de grupo e quota de agente permitam execução.
3. Um job elegível de outro grupo/agente **nunca** pode ficar bloqueado atrás de um job cujo grupo/agente está cheio (eliminação de *Head-of-Line Blocking* no SQL de `claim_next_eligible`).
4. Alterar `max_concurrency` via `PUT /api/queue/concurrency` persiste no SQLite, atualiza memória, dispara `notify.notify_waiters()` e emite evento WS `queue.updated` imediatamente.

### 3.3. Contrato de Eventos em Tempo Real (WebSocket + Store)

```ts
export interface LiveRevisions {
  queue: number;
  tasks: number;
  runs: number;
  artifacts: number;
  attention: number;
  schedules: number;
  recipes: number;
  memory: number;
  governance: number;
  studio: number;
}
```

**Invariantes de Sincronização Live:**

1. Todo endpoint de mutação no backend (e transições internas do `queue_manager`, `cron_engine`, `recipes`, `io_bus`, `chat_turn`) emite `WsEnvelope` contendo `workspace_id` no payload.
2. Em `src-tauri/src/api/ws.rs`, `Err(RecvError::Lagged(n))` envia `{"event_type":"sync.required","payload":{"missed":n}}` antes de continuar o loop.
3. No frontend (`app-store.ts`), um dispatcher único atualiza o estado global (`queue`, `queueStatus`, `tasks`, `stats`, `attentionSummary`) com debounce de `150ms` por domínio e incrementa `revisions[domain]`; ao receber `sync.required`, reconectar WS ou voltar do background (`visibilitychange`), executa `refreshLiveState(activeWorkspaceId)`.
4. Toda carga assíncrona em `app-store.ts` usa um contador `workspaceRequestSeq`: respostas de um `seq` antigo são descartadas antes de `set(...)`.

### 3.4. Contrato de Backup 100% do Workspace (`backup_manifest.rs` v2)

```json
{
  "version": 2,
  "app_version": "1.8.x",
  "created_at": "2026-11-24T10:30:00Z",
  "workspace": { "id": "...", "name": "...", "slug": "...", "original_root_path": "..." },
  "includes": {
    "workspace_files": true,
    "sqlite_state": true,
    "run_logs": true,
    "encrypted_secrets": false
  },
  "counts": {
    "files": 142,
    "tasks": 38,
    "runs": 95,
    "artifacts": 64,
    "memories": 47,
    "personal_context": 8,
    "schedules": 6,
    "recipes": 12,
    "chat_threads": 19,
    "vault_entries": 5
  },
  "db_sha256": "...",
  "files_sha256": { ".opencode/opencode.jsonc": "..." }
}
```

**Invariantes de Backup/Restore:**

* **100% das tabelas SQLite do workspace** são exportadas dentro de `db/workspace_state.json` em ordem topológica de chaves estrangeiras (workspaces, tasks, task_messages, task_watchers, task_dedupe, runs, artifacts, chat_threads, chat_messages, chat_turns, interactions, schedules, missed_runs, recipe_files, recipe_runs, recipe_nodes, recipe_versions, recipe_hitl, memory_namespaces, memory_notes, memory_acls, personal_context, learning_lessons, learning_settings, proposals, skill_eval_cases, agent_profiles, agent_goals, agent_delegations, handoffs, router_rules, router_settings, budget_settings, prompt_templates, calendar_sources, calendar_events, inbound_sources, outbound_sinks, channel_routes, tickets, vault_entries_meta, vault_acls, starter_packs_installed).
* **Segredos portáveis:** se o usuário fornecer `passphrase` no backup, os segredos do workspace (Vault + SMTP + tokens) são re-cifrados em `db/secrets.enc` com Argon2id + AES-256-GCM; sem passphrase, `secrets.enc` é omitido e o relatório de restore lista exatamente quais integrações/vault_entries exigem re-inserção de segredo.
* **Restore In-Place atômico:** antes de sobrescrever, gera automaticamente um backup de segurança em `backups/pre-restore-<timestamp>.zip`; extrai arquivos em diretório temporário adjacente, valida 100% dos SHA-256, executa toda a restauração SQLite em **uma única transação** e faz swap atômico; qualquer erro reverte DB e filesystem sem corrupção.
* **Compatibilidade retroativa:** backups `version: 1` continuam restaurando arquivos normalmente com aviso explícito `"Backup v1 (somente arquivos; estado SQLite não incluído)"`.

### 3.5. Contrato do Pacote JSON de Configuração Rápida (`opendesk-config-bundle.v1.json`)

```json
{
  "schema_version": 1,
  "exported_at": "2026-11-24T10:30:00Z",
  "app_version": "1.8.x",
  "source_workspace": { "name": "Meu Workspace", "timezone": "Europe/Rome" },
  "sections": {
    "memory_md": "...",
    "structured_memory": { "namespaces": [], "notes": [], "acls": [] },
    "personal_context": { "global": [], "workspace": [] },
    "learning": { "settings": {}, "lessons": [] },
    "prompt_templates": { "global": {}, "workspace": {} },
    "router": { "policy": "COST_BENEFIT", "default_model": null, "manual_model": null, "router_model": null, "default_harness": "opencode", "cost_blend": 0.5, "rules": [] },
    "budget": {},
    "queue": { "max_concurrency": 3, "max_queue_size": 200, "group_limits": {}, "dedupe": {} },
    "governance": { "profiles": [], "goals": [] },
    "runtime": { "harnesses": [], "sandbox": {}, "voice": {} },
    "automation_routes": { "channel_routes": [], "inbound_sources": [], "outbound_sinks": [] },
    "studio_assets": { "agents": [], "skills": [], "commands": [], "recipes": [], "routines": [] },
    "vault_metadata": []
  }
}
```

**Invariantes Fail-Closed do Config Bundle:**

* **Zero segredos em texto claro:** exportação faz strip de qualquer campo contendo `password`, `token`, `secret`, `api_key`, `ciphertext` (mantendo apenas nomes `vault_ref`). Teste unitário obrigatório valida que nenhum segredo do `SecretStore` ou `vault_entries` aparece na saída JSON.
* **Validação estrita na importação:** `schema_version != 1` retorna `400 Bad Request`; toda seção importada roda dentro de transação SQLite + escrita atômica de arquivos (`.opencode/*`, `MEMORY.md`).
* **Modos de conflito:** `merge_skip_existing` (default seguro), `merge_overwrite` (upsert por slug/chave natural), `replace_section` (substitui apenas as seções marcadas no request).

---

## 4. Fases Atômicas de Implementação (Microspecs)

### Fase 1 [P0] — Correção Completa de Links, Abas e Navegação Profunda

*Objetivo: Garantir que 100% dos cliques em notificações, Topbar, Attention Card, Calendário, Busca e Push abram exatamente a rota, aba e item corretos.*

* **Arquivos da fase:**
* Ler: `src/App.tsx`, `src/lib/links.ts`, `src/lib/links.test.ts`, `src/components/layout/app-shell/use-app-notifications.ts`, `src/components/layout/Topbar.tsx`, `src/pages/KanbanPage.tsx`, `src/components/queue/QueueController.tsx`, `src/pages/SettingsPage.tsx`, `src/pages/IntegrationsPage.tsx`, `src/pages/StudioPage.tsx`, `src/pages/LearningPage.tsx`, `src/pages/GovernancePage.tsx`, `src-tauri/src/api/calendar_agenda/items.rs`, `src-tauri/src/api/search/all.rs`.
* Modificar: `src/lib/links.ts`, `src/lib/links.test.ts`, `src/components/layout/app-shell/use-app-notifications.ts`, `src/components/layout/Topbar.tsx`, `src/pages/KanbanPage.tsx`, `src/components/queue/QueueController.tsx`, `src/pages/SettingsPage.tsx`, `src/pages/IntegrationsPage.tsx`, `src/pages/StudioPage.tsx`, `src/pages/LearningPage.tsx`, `src/pages/GovernancePage.tsx`, `src-tauri/src/api/calendar_agenda/items.rs`, `src-tauri/src/api/search/all.rs`.

* **Passos determinísticos:**
1. Atualizar `src/lib/links.ts` com todos os construtores canônicos da Seção 3.1 (especialmente `links.queue(jobId?)` → `/kanban?view=queue&job=...` e `links.kanban(taskId?)` → `/kanban?view=board&task=...`).
2. Em `src/pages/KanbanPage.tsx`, sincronizar `view` (`board` | `list` | `queue`) com `searchParams.get("view")` via `useEffect([searchParams])`; ao clicar nas abas da UI, atualizar o query param `?view=` com `setSearchParams(..., { replace: true })`; passar `useFocusParam("job")` para `QueueController.tsx` e `useFocusParam("task")` para o board/lista.
3. Em `QueueController.tsx`, adicionar `data-focus-id={job.id}` em cada linha/card de job para que o scroll e highlight funcionem ao abrir `/kanban?view=queue&job=<id>`.
4. Em `use-app-notifications.ts` e `Topbar.tsx`, substituir todos os links de eventos de fila (`job.enqueued`, `queue.paused`, `queue.resumed`, clique no badge da fila no Topbar) para usar `links.queue(jobId)` em vez de `/kanban`.
5. Auditar e corrigir `useEffect([searchParams])` em `SettingsPage.tsx` (`?section=`), `IntegrationsPage.tsx` (`?section=`), `StudioPage.tsx` (`?tab=&focus=`), `LearningPage.tsx` (`?tab=`) e `GovernancePage.tsx` (`?section=`).
6. Auditar e alinhar todas as URLs geradas no backend em `calendar_agenda/items.rs` (itens `queued` → `/kanban?view=queue&job=<id>`) e `search/all.rs` com `src/lib/links.ts`.

* **Refatoração / Simplificação / Limpeza acoplada:**
* Remover strings de rota hardcoded espalhadas em componentes (`"/kanban"`, `"/settings"`, `"/runs/" + id`) substituindo por chamadas a `links.*()`.

* **Edge cases obrigatórios:**
* Clicar em uma notificação de fila enquanto o usuário **já está** em `/kanban?view=board` deve alternar imediatamente para `view=queue` e rolar até o job.
* Query param `?view=invalido` ou `?section=inexistente` deve fazer fallback seguro para a aba padrão sem quebrar renderização.

* **Validação:**
```bash
npm run check && npm run test:ui -- links KanbanPage QueueController
(cd src-tauri && cargo test calendar && cargo test global_search)
```

*Critério PASS:* Todos os testes de contrato de link passam; navegar para `/kanban?view=queue&job=123` abre diretamente a visão da Fila e aplica foco no item `123`.

---

### Fase 2 [P0] — Execução Concorrente Real, Controle de Tamanho da Fila e Diagnóstico de Espera

*Objetivo: Eliminar o gargalo de 1 execução por vez, permitir gerenciamento completo de concorrência/tamanho da fila e mostrar exatamente por que cada job está esperando.*

* **Arquivos da fase:**
* Ler: `src-tauri/crates/opendesk-engine/src/queue_scheduler/queue_manager/worker.rs`, `execute.rs`, `mod.rs`, `src-tauri/crates/opendesk-storage/src/repos/queue.rs`, `src-tauri/src/api/queue.rs`, `src-tauri/src/bootstrap.rs`, `src/lib/queue.ts`, `src/components/settings/QueueSettingsCard.tsx`, `src/components/queue/QueueController.tsx`.
* Modificar: `worker.rs`, `mod.rs`, `repos/queue.rs`, `api/queue.rs`, `bootstrap.rs`, `src/lib/queue.ts`, `src/components/settings/QueueSettingsCard.tsx`, `src/components/queue/QueueController.tsx`, `src/lib/types/core.ts`.

* **Passos determinísticos:**
1. Em `repos/queue.rs` e `api/queue.rs`, persistir `QueueConfig` (`max_concurrency` default `3`, `max_queue_size` default `200`, `default_agent_concurrency` default `2`, `group_limits`) em `app_settings` e carregar no startup em `bootstrap.rs`.
2. Em `repos/queue.rs`, corrigir `claim_next_eligible` para aceitar o conjunto de `(active_groups, active_agents)` e selecionar o job de maior prioridade cujos limites de grupo e quota de agente ainda tenham slot livre (evitando Head-of-Line Blocking). Validar `max_queue_size` no enqueue retornando erro tipado `QueueFull` (HTTP `429`).
3. Em `queue_manager/worker.rs`, refatorar o loop principal para manter um `tokio::task::JoinSet` de jobs em execução simultânea até `max_concurrency`, acordando imediatamente via `Arc<tokio::sync::Notify>` sempre que um job for enfileirado, concluído, retomado ou quando `max_concurrency` aumentar.
4. Em `api/queue.rs` (`list_queue` e `queue_status`), retornar `active_slots`, `max_concurrency`, `max_queue_size`, `group_usage` e `wait_reason` calculado por job em espera.
5. Em `src/lib/queue.ts`, criar `allQueueCounts(jobs)` contando **todos** os jobs ativos/aguardando do motor (incluindo rotinas/receitas/IO Bus/delegações) e manter `workQueue(jobs)` apenas como filtro opcional na UI.
6. Em `QueueController.tsx`, exibir cabeçalho de capacidade (`Slots: X / Y ativos · Fila: Z / MAX`), controle rápido `+ / -` de concorrência, toggle `"Mostrar automações (rotinas/receitas)"` ativo por padrão e badge `wait_reason` em cada job `WAITING`.
7. Em `QueueSettingsCard.tsx`, adicionar edição de `max_concurrency` (1–32), `max_queue_size`, `default_agent_concurrency` e limites por grupo (`local_llm_heavy`, `web_scraper`, `light_api`, `default`).

* **Refatoração / Simplificação / Limpeza acoplada:**
* Remover qualquer `Mutex` segurado através de `.await` durante execução de job no `worker.rs` (causa comum de serialização acidental em Rust async).
* Garantir que `AgentProfile.concurrency_quota` use `default_agent_concurrency` quando não explicitamente configurado como `1`.

* **Edge cases obrigatórios:**
* Reduzir `max_concurrency` de `4` para `2` enquanto 4 jobs rodam: nunca matar os 2 excedentes — apenas parar de reivindicar novos jobs até `active_slots < 2`.
* Dois jobs na mesma tarefa (`task_id` idêntico) ou mesma `session_id`: `claim_next_eligible` deve impedir execução simultânea no mesmo `task_id`/`session_id` para evitar corrupção de sessão, marcando `wait_reason = "Mesma tarefa/sessão já em execução"`.
* 3 jobs enfileirados simultaneamente com `max_concurrency = 3` devem iniciar em paralelo dentro de `< 300ms` (validado em teste determinístico com executor fake com barreira `tokio::sync::Barrier`).

* **Validação:**
```bash
(cd src-tauri && cargo test -p opendesk-engine queue_manager && cargo test -p opendesk-storage queue_tests && cargo test queue)
npm run check && npm run test:ui -- queue QueueController QueueSettingsCard
```

*Critério PASS:* Teste de concorrência real com barreira prova 3 jobs rodando simultaneamente; teste de `max_queue_size` retorna `429`; teste de Head-of-Line Blocking prova que job `light_api` roda enquanto `local_llm_heavy` está lotado.

---

### Fase 3 [P0] — Mostradores 100% "Online" em Tempo Real (WebSocket + Store Reativo + Resync)

*Objetivo: Fazer com que Topbar, Sidebar, Dashboard, Kanban, Fila, Runs, Artefatos e Atenção reflitam instantaneamente toda criação, início, progresso e término sem precisar trocar de página.*

* **Arquivos da fase:**
* Ler: `src-tauri/src/api/ws.rs`, `src-tauri/crates/opendesk-storage/src/events.rs`, `src/store/app-store.ts`, `src/store/app-store-types.ts`, `src/store/app-store.test.ts`, `src/components/layout/Topbar.tsx`, `src/components/layout/Sidebar.tsx`, `src/components/dashboard/AttentionCard.tsx`, `src/pages/DashboardPage.tsx`, `src/pages/KanbanPage.tsx`, `src/pages/RunsPage.tsx`, `src/pages/ArtifactsPage.tsx`, `src/lib/studio-cache.ts`.
* Modificar: `src-tauri/src/api/ws.rs`, `src-tauri/crates/opendesk-storage/src/events.rs`, `src/store/app-store.ts`, `src/store/app-store-types.ts`, `src/store/app-store.test.ts`, `src/components/layout/Topbar.tsx`, `src/components/layout/Sidebar.tsx`, `src/components/dashboard/AttentionCard.tsx`, `src/pages/DashboardPage.tsx`, `src/pages/KanbanPage.tsx`, `src/pages/RunsPage.tsx`, `src/pages/ArtifactsPage.tsx`, `src/lib/studio-cache.ts`.

* **Passos determinísticos:**
1. Em `src-tauri/src/api/ws.rs`, tratar `Err(RecvError::Lagged(missed))` enviando `WsEnvelope { event_type: "sync.required", payload: json!({ "missed": missed }) }` ao cliente, e garantir que `send_initial` envie snapshot inicial de `queue_status`, `engine_status` e `attention_counts`.
2. Auditar emissores de eventos em `src-tauri/src/api/` e `queue_manager/` garantindo emissão de eventos padronizados com `workspace_id` para: `queue.*` (enqueued, claimed, updated, finished, paused, resumed, concurrency_changed), `task.*`, `run.*`, `artifact.*`, `attention.*` (permission/interaction/hitl/proposal/missed_run), `schedule.*`, `recipe.*`, `memory.*`, `studio.*`.
3. Em `src/store/app-store.ts`, adicionar:
* Estado global live: `queue: QueueJob[]`, `queueStatus: QueueStatus | null`, `attentionCount: number`, `revisions: LiveRevisions`, `wsConnected: boolean`;
* `workspaceRequestSeq` para descartar respostas obsoletas ao trocar de workspace;
* Dispatcher central no handler `ws.onmessage` que, com debounce de `150ms` por domínio, atualiza `queue`/`queueStatus`/`stats`/`attentionCount` e incrementa `revisions[domain]`, além de invalidar `studio-cache` em eventos `studio.*` / `model_health.*`;
* Resync automático (`refreshLiveState`) em: reconexão WS (`onopen` após queda), evento `sync.required` e `document.addEventListener("visibilitychange")` quando a aba volta a ficar visível.
4. Atualizar `Topbar.tsx` e `Sidebar.tsx` para ler `allQueueCounts(queue)`, `queueStatus` e `attentionCount` diretamente do `app-store` (zero estado local desconectado).
5. Conectar `DashboardPage.tsx`, `AttentionCard.tsx`, `KanbanPage.tsx`, `RunsPage.tsx` e `ArtifactsPage.tsx` aos respectivos `revisions.*` do `app-store` para recarga incremental silenciosa (sem spinner bloqueante piscando na tela).

* **Refatoração / Simplificação / Limpeza acoplada:**
* Remover timers `setInterval` redundantes espalhados em componentes que faziam polling cego onde já existe evento WS; manter apenas fallback lento (`30s`) ativo **somente** quando `wsConnected === false`.

* **Edge cases obrigatórios:**
* Rajada de 50 eventos em 200ms (ex.: receita `map` criando 20 artefatos): debounce de `150ms` garante no máximo 1–2 fetches sem travar a UI.
* Troca rápida Workspace A → Workspace B enquanto fetch de A demora 1s: resultado de A é descartado pelo `workspaceRequestSeq`, mantendo números de B intactos.
* Eventos de outro workspace (`payload.workspace_id !== activeWorkspaceId`) não disparam refresh desnecessário do workspace ativo (exceto contadores globais de engine/fila).

* **Validação:**
```bash
(cd src-tauri && cargo test ws && cargo test queue)
npm run check && npm run test:ui -- app-store Topbar AttentionCard
```

*Critério PASS:* Teste de `app-store.test.ts` simula eventos `queue.updated`, `task.updated`, `run.finished`, `sync.required` e troca rápida de workspace comprovando atualização imediata dos contadores sem trocar de rota e sem vazamento entre workspaces.

---

### Fase 4 [P0] — Backup e Restauração 100% Completos do Workspace (ZIP v2 + SQLite + Logs + Segredos Portáveis)

*Objetivo: Garantir que restaurar um backup recrie integralmente arquivos, tarefas, histórico de runs, chats, memórias, prompts, receitas, rotinas, governança, rotas e opcionalmente segredos.*

* **Arquivos da fase:**
* Ler: `src-tauri/src/backup.rs`, `backup_create.rs`, `backup_manifest.rs`, `backup_restore.rs`, `backup_tests.rs`, `src-tauri/src/api/backups.rs`, `src-tauri/src/api/tests/backups.rs`, `src/components/settings/BackupsCard.tsx`, `src/lib/types/backup.ts`.
* Modificar: `backup_manifest.rs`, `backup_create.rs`, `backup_restore.rs`, `backup_tests.rs`, `src-tauri/src/api/backups.rs`, `src-tauri/src/api/tests/backups.rs`, `src/components/settings/BackupsCard.tsx`, `src/lib/types/backup.ts`.

* **Passos determinísticos:**
1. Atualizar `backup_manifest.rs` para suportar `version: 2` (mantendo leitura de `version: 1`) com campos `includes`, `counts`, `db_sha256` e `files_sha256`.
2. Em `backup_create.rs`, exportar:
* Todos os arquivos do `workspace.root_path` (incluindo `.opencode/**`, `MEMORY.md`, `AGENTS.md`, `input/**`, `output/**`), ignorando apenas diretórios transitórios de worktree efêmero (`.opendesk-worktrees/`);
* `db/workspace_state.json`: snapshot consistente (dentro de transação `BEGIN DEFERRED` de leitura) de todas as tabelas vinculadas ao `workspace_id` (listadas na Seção 3.4);
* `db/run_logs/<run_id>.jsonl`: logs de execução pertencentes aos runs do workspace armazenados em `OPENDESK_DATA_DIR/runs/`;
* `db/secrets.enc` (opcional quando `passphrase` é fornecida): segredos do workspace re-cifrados com Argon2id + AES-256-GCM.
3. Em `backup_restore.rs`, implementar dois modos explícitos:
* `new_workspace`: gera novo `workspace_id`, remapeia todas as FKs (`workspace_id`, `task_id`, `run_id`, `schedule_id`, `thread_id`, `artifact_id`, `log_path`, `absolute_path`) de forma determinística via `HashMap<old_id, new_id>`, grava arquivos no `target_path` escolhido e insere todas as linhas em transação única;
* `in_place`: cria backup de segurança automático antes de tocar no disco, extrai em staging temporário, valida SHA-256 de cada arquivo + `db_sha256`, substitui linhas do workspace no SQLite em transação única e faz swap atômico de diretórios (revertendo em caso de erro).
4. Retornar `RestoreReport` com contagens restauradas por entidade e `missing_secrets: Vec<String>` (quando `secrets.enc` não foi incluído).
5. Atualizar `BackupsCard.tsx` com checkboxes (`Incluir logs de execução`, `Incluir segredos protegidos por senha`), campo opcional de senha na criação/restauração, seletor de modo (`Restaurar como novo workspace` vs `Substituir workspace atual`) e resumo visual pós-restore.

* **Refatoração / Simplificação / Limpeza acoplada:**
* Extrair lista única de tabelas do workspace (`WORKSPACE_TABLES`) usada tanto no export quanto no restore e no teste de paridade, evitando esquecer tabelas novas.
* Criar teste de verificação automática em `backup_tests.rs` que lê `sqlite_master` buscando toda tabela com coluna `workspace_id` e **falha o teste** se alguma tabela operacional ficar fora do backup.

* **Edge cases obrigatórios:**
* Zip-slip (`../etc/passwd`, symlinks apontando para fora do destino): rejeitar arquivo inteiro fail-closed antes de escrever qualquer byte.
* Senha errada ao abrir `db/secrets.enc`: abortar restauração com erro `400 Bad Request` (`INVALID_BACKUP_PASSPHRASE`) antes de modificar banco ou arquivos.
* Restaurar backup no mesmo host onde o workspace original ainda existe (`new_workspace`): zero colisão de PKs/UNIQUE constraints e `output_dir_path`/`absolute_path` atualizados para o novo `root_path`.

* **Validação:**
```bash
(cd src-tauri && cargo test backup)
npm run check && npm run test:ui -- BackupsCard
```

*Critério PASS:* Teste round-trip cria workspace populado com todas as entidades (arquivos, tarefas, runs+jsonl, artefatos, memórias, prompts, receitas, rotinas, chats, governança, vault com senha), faz backup, restaura em diretório limpo e verifica 100% de igualdade em todas as entidades; teste de `sqlite_master` garante 0 tabelas órfãs.

---

### Fase 5 [P1] — Importação e Exportação JSON Rápida (Memórias, Prompts e Configurações)

*Objetivo: Permitir clonar ou configurar rapidamente qualquer workspace exportando e importando memórias, contexto pessoal, prompts e configurações em um único arquivo JSON legível e seguro.*

* **Arquivos da fase:**
* Ler: `src-tauri/src/api/memory.rs`, `personal_context.rs`, `prompt_templates.rs`, `router.rs`, `budget.rs`, `governance.rs`, `learning.rs`, `src-tauri/src/api/router_table/system.rs`, `src-tauri/src/cli/args.rs`, `commands.rs`, `src/pages/SettingsPage.tsx`, `src/components/learning/StructuredMemoryCard.tsx`, `src/components/settings/PromptTemplatesCard.tsx`.
* Criar: `src-tauri/src/config_bundle.rs`, `src-tauri/src/api/config_bundle.rs`, `src-tauri/src/api/tests/config_bundle.rs`, `src/components/settings/ConfigBundleCard.tsx`, `src/components/settings/ConfigBundleCard.test.tsx`.
* Modificar: `src-tauri/src/lib.rs`, `src-tauri/src/api/mod.rs`, `src-tauri/src/api/router_table/system.rs`, `src-tauri/src/cli/args.rs`, `src-tauri/src/cli/commands.rs`, `src/pages/SettingsPage.tsx`, `src/components/learning/StructuredMemoryCard.tsx`, `src/components/settings/PromptTemplatesCard.tsx`.

* **Passos determinísticos:**
1. Implementar `src-tauri/src/config_bundle.rs` com `export_bundle(state, workspace_id, sections)`, `preview_import(state, workspace_id, bundle, mode)` e `apply_import(state, workspace_id, bundle, sections, mode)` seguindo exatamente o schema da Seção 3.5.
2. Garantir sanitização fail-closed no export: passar toda estrutura de canais/IO Bus/MCP por filtro que remove valores secretos e valida ausência de plaintext de qualquer chave do Vault/SecretStore.
3. Expor endpoints REST em `src-tauri/src/api/config_bundle.rs` e registrar em `router_table/system.rs`:
* `POST /api/config-bundle/export`
* `POST /api/config-bundle/preview`
* `POST /api/config-bundle/import`
4. Adicionar comandos CLI em `src-tauri/src/cli/`:
* `opendesk config export <path> [--workspace <id>] [--sections all|memories,prompts,...]`
* `opendesk config import <path> [--workspace <id>] [--mode merge-skip|merge-overwrite|replace] [--dry-run]`
5. Criar `ConfigBundleCard.tsx` (exibida em `SettingsPage` na seção Geral/Backups) com:
* Presets rápidos: **"Tudo (exceto segredos)"**, **"Somente Memórias & Contexto"**, **"Somente Prompts & Studio"**, **"Roteador, Fila & Governança"**;
* Seletor granular por seção;
* Upload de `.json` → chamada automática a `/preview` exibindo tabela `Criar / Atualizar / Ignorar / Conflito` antes do botão confirmar;
* Seletor de estratégia (`Manter existentes`, `Sobrescrever conflitos`, `Substituir seções selecionadas`).
6. Adicionar botões de atalho **"Exportar JSON"** e **"Importar JSON"** em `StructuredMemoryCard.tsx` (pré-selecionando memórias/contexto/lições) e em `PromptTemplatesCard.tsx` (pré-selecionando prompts).

* **Refatoração / Simplificação / Limpeza acoplada:**
* Reutilizar as funções existentes dos repositórios (`memory`, `personal_context`, `prompt_templates`, `router`, `governance`, `config_sync`) sem duplicar SQL ou lógica de escrita de frontmatter.
* Manter `config_bundle.rs` enxuto (dividir em `config_bundle/export.rs`, `config_bundle/import.rs`, `config_bundle/types.rs` se ultrapassar o limite de linhas de `scripts/check-rust-limits.py`).

* **Edge cases obrigatórios:**
* JSON com `schema_version` desconhecido, sintaxe inválida ou payload > 10 MB: rejeitar com `400`/`413` antes de iniciar transação.
* Importar `structured_memory` com notas cujo namespace ainda não existe: criar namespaces automaticamente antes das notas e ACLs na mesma transação.
* Importar em `replace_section` quando apenas `prompt_templates` foi selecionado: **nunca** apagar memórias ou outras seções não marcadas.

* **Validação:**
```bash
(cd src-tauri && cargo test config_bundle)
npm run check && npm run test:ui -- ConfigBundleCard
```

*Critério PASS:* Testes de export/preview/import comprovam round-trip completo de memórias + prompts + configs, respeito às 3 estratégias de conflito e 0 vazamento de segredos no JSON exportado.

---

### Fase 6 [P1] — Gate Final de Qualidade, Limites e Regressão E2E

*Objetivo: Validar toda a base sem regressões de tipos, lint, limites de tamanho ou testes E2E.*

* **Arquivos da fase:**
* Ler / Executar: `scripts/gate-fast.sh`, `scripts/check-rust-limits.py`, `scripts/check-frontend-limits.py`, `tests/e2e/kanban-views.spec.ts`, `tests/e2e/backups-ui.spec.ts`, `tests/e2e/settings-views.spec.ts`.
* Modificar (se necessário): testes E2E impactados pelos novos links `/kanban?view=queue` e novos controles de fila/backup/config bundle.

* **Passos determinísticos:**
1. Rodar `scripts/gate-fast.sh --rust` e corrigir qualquer aviso de `clippy -D warnings`, `tsc`, `eslint` ou teste unitário.
2. Rodar `python3 scripts/check-rust-limits.py` e `python3 scripts/check-frontend-limits.py`, dividindo qualquer arquivo modificado que ultrapasse o limite do baseline.
3. Rodar os testes E2E focados nas áreas alteradas (`kanban-views`, `backups-ui`, `attention-ui`, `dashboard-data`, `settings-views`).

* **Validação:**
```bash
scripts/gate-fast.sh --rust
python3 scripts/check-rust-limits.py
python3 scripts/check-frontend-limits.py
npx playwright test tests/e2e/kanban-views.spec.ts tests/e2e/backups-ui.spec.ts tests/e2e/attention-ui.spec.ts tests/e2e/dashboard-data.spec.ts
```

*Critério PASS:* Todos os comandos terminam com exit code `0`.

---

## 5. Prompt de Execução para o OpenCode

```text
Você vai implementar a especificação de estabilização e fechamento operacional do OpenDesk (Fases 1 a 6). Siga estas regras estritas sem desvios:

1. UMA FASE POR VEZ: Comece exclusivamente pela Fase 1 [P0]. Leia primeiro APENAS os arquivos listados em "Arquivos da fase" antes de editar qualquer código. Nunca altere arquivos fora do escopo da fase atual.
2. SIMPLICIDADE E LIMITES: Reutilize os componentes existentes de `src/components/ui/` e repositórios de `opendesk-storage`. Não instale novas dependências externas. Mantenha arquivos enxutos para respeitar `scripts/check-rust-limits.py` e `scripts/check-frontend-limits.py`.
3. FAIL-CLOSED: Nunca silencie erros (`unwrap_or_default()`, `catch {}` vazio) em restauração de backup, validação de zip-slip, verificação de senha de segredos, limite `max_queue_size`, validação de `schema_version` do JSON ou verificação de URLs.
4. SEM CONCORRÊNCIA FAKE: Na Fase 2, não segure `Mutex` através de `.await` no worker da fila e prove execução paralela real (3 jobs simultâneos) com teste determinístico usando `tokio::sync::Barrier`.
5. INTEGRIDADE DE TESTES: Nunca apague nem enfraqueça testes existentes. Atualize imports, tipagens TypeScript e testes unitários/API na mesma fase.
6. VALIDAÇÃO OBRIGATÓRIA: Ao concluir os passos da fase atual, rode exatamente os comandos da seção "Validação" da fase e corrija todas as falhas antes de responder.
7. PONTO DE PARADA: Ao finalizar a Fase 1 com todos os testes verdes, entregue um resumo curto (arquivos modificados, testes rodados e resultado PASS) e aguarde confirmação antes de iniciar a Fase 2.

Inicie agora a **Fase 1 [P0] — Correção Completa de Links, Abas e Navegação Profunda**.
```
