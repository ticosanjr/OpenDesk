# Auditoria de schema V1 → V2

- **Data:** 2026-11-22
- **Fonte:** 59 migrations (`0001`…`0059_skill_shadow.sql`) aplicadas em um banco temporário e verificadas contra o código real.
- **Método:** varredura por palavra inteira de cada tabela/coluna em `src-tauri/` e `src/` (Rust, TS e TSX), excluindo `target/` e as migrations. `rg` não está instalado neste ambiente; o equivalente foi feito com `grep`/Python e a mesma semântica de palavra.
- **Resultado:** 66 tabelas V1 → **58 mantidas + 2 novas + `v2_meta`**, 5 fundidas, 3 descartadas.

## 1. Tabelas fundidas em `conversations` + `messages`

| V1 | V2 | Campos preservados |
|---|---|---|
| `chat_threads` (3 refs) | conversations (channel=chat) | threads, agent, model, session, source/external_id |
| `chat_messages` (9 refs) | messages (kind=chat) | role/body/metadata |
| `chat_turns` (16 refs) | messages (kind=turn) | tokens/custo/duração/erro em payload_json |
| `task_messages` (10 refs) | messages (kind=comment) | comentários e follow-ups da tarefa |
| `task_interactions` (21 refs) | messages (kind=hitl_request) | pergunta/opções/status/decisão |

O stream é único e ordenado por `messages.id` (UUIDv7), o que dá paginação por cursor sem `OFFSET`. HITL virou `kind=hitl_request` com `status`, `expires_at`, `choice_json` e o payload da pergunta.

## 2. Tabelas descartadas (0 referências)

| Tabela | Evidência |
|---|---|
| `config_change_log` | nenhuma referência em Rust/TS; substituída por proposals + logs |
| `memory_entities` | memória usa namespaces + notes; entidades nunca foram lidas |
| `runtime_events` | telemetria usa task_runs/telemetry; tabela nunca foi escrita |

`learning_run_captures.captured_at` também foi removida: o código insere apenas `run_id`, `workspace_id` e `lesson_id` e nunca lê a coluna.

## 3. Tabelas mantidas e destino

| Tabela | Refs | Destino V2 |
|---|---:|---|
| `adapter_registry` | 4 | repos/ops.rs |
| `agent_delegations` | 4 | repos/tasks.rs |
| `agent_goals` | 6 | repos/tasks.rs |
| `agent_profiles` | 7 | repos/tasks.rs |
| `alert_dismissals` | 5 | repos/integrations.rs |
| `api_audit` | 5 | enterprise/ent-auth |
| `app_settings` | 8 | repos/workspaces.rs |
| `artifacts_provenance` | 17 | repos/runs.rs |
| `auth_sessions` | 18 | enterprise/ent-auth |
| `budget_settings` | 3 | repos/queue.rs |
| `calendar_events` | 13 | repos/integrations.rs |
| `calendar_sources` | 12 | repos/integrations.rs |
| `calendar_trigger_fires` | 5 | repos/integrations.rs |
| `channel_routes` | 7 | repos/io_bus.rs |
| `computer_sessions` | 8 | enterprise/ent-computer |
| `config_proposals` | 5 | repos/ops.rs |
| `execution_queue` | 34 | repos/queue.rs |
| `handoffs` | 59 | repos/integrations.rs |
| `hitl_requests` | 10 | repos/recipes.rs |
| `integration_deliveries` | 13 | repos/io_bus.rs |
| `io_inbound_sources` | 7 | repos/io_bus.rs |
| `io_outbound_sinks` | 6 | repos/io_bus.rs |
| `learning_lessons` | 16 | repos/runs.rs |
| `learning_run_captures` | 2 | repos/runs.rs |
| `memory_acl` | 7 | repos/integrations.rs |
| `memory_namespaces` | 7 | repos/integrations.rs |
| `memory_notes` | 13 | repos/integrations.rs |
| `missed_runs` | 54 | repos/schedules.rs |
| `model_health` | 25 | repos/queue.rs |
| `passkey_ceremonies` | 5 | enterprise/ent-auth |
| `passkey_credentials` | 10 | enterprise/ent-auth |
| `permission_audit` | 1 | repos/tasks.rs |
| `permission_requests` | 8 | repos/tasks.rs |
| `personal_context_groups` | 6 | repos/integrations.rs |
| `push_subscriptions` | 9 | repos/integrations.rs |
| `recipe_file_state` | 5 | repos/recipes.rs |
| `recipe_run_nodes` | 16 | repos/recipes.rs |
| `recipe_runs` | 18 | repos/recipes.rs |
| `recipe_versions` | 8 | repos/recipes.rs |
| `recipes_index` | 17 | repos/recipes.rs |
| `router_rules` | 5 | repos/integrations.rs |
| `run_ab_groups` | 4 | repos/runs.rs |
| `run_worktrees` | 7 | repos/runs.rs |
| `schedules` | 223 | repos/schedules.rs |
| `session_context_packets` | 2 | repos/ops.rs |
| `skill_eval_cases` | 4 | enterprise/ent-eval |
| `skill_eval_gates` | 4 | enterprise/ent-eval |
| `skill_shadow_results` | 2 | enterprise/ent-eval |
| `skill_shadow_runs` | 5 | enterprise/ent-eval |
| `starter_pack_installs` | 6 | repos/ops.rs |
| `task_runs` | 58 | repos/runs.rs |
| `task_sink_bindings` | 8 | repos/tasks.rs |
| `task_watchers` | 12 | repos/tasks.rs |
| `tasks` | 724 | repos/tasks.rs |
| `tickets` | 35 | repos/integrations.rs |
| `workers` | 13 | enterprise/ent-cluster |
| `workspace_integrations` | 5 | repos/workspaces.rs |
| `workspaces` | 461 | repos/workspaces.rs |

## 4. Baseline

- `crates/opendesk-storage/migrations/0001_v2_baseline.sql`: 61 tabelas, todas `STRICT`.
- Tipos normalizados: 5 colunas `BOOLEAN` → `INTEGER` (0/1); `JSON` → `TEXT` (com `CHECK (json_valid(...))` quando aplicável).
- Seeds preservados: `budget_settings(id=1)` e os 5 harnesses de `adapter_registry`.
- Índices de cursor: `conversations(workspace_id, updated_at DESC, id DESC)`, `messages(conversation_id, created_at, id)`, únicos parciais para `thread_key`/`external_id`/`dedupe_key`.
- `PRAGMA foreign_keys`, `journal_mode`, `synchronous`, `busy_timeout` e `cache_size` são aplicados no pool (não na migration, que roda em transação).

## 5. Divergências encontradas

1. `0001_initial_schema.sql` e `0025_recipe_workspace_key.sql` usam `PRAGMA journal_mode/foreign_keys` dentro de migrations: são no-op sob a transação do sqlx. O V2 configura tudo no pool e testa os valores efetivos.
2. O V1 não tinha guarda de versão: abrir um banco antigo com o baseline novo o corromperia. O V2 recusa um banco com `_sqlx_migrations` sem `v2_meta` e aponta para `opendesk migrate-v1-export`.
3. `busy_timeout` do V1 era 15 s por causa de `database is locked`; o V2 adota 5 s conforme o contrato, com teste de pragmas e rollback documentado para 15 s se o stress test acusar lock.

## 6. Validação

```bash
cargo test -p opendesk-domain -p opendesk-storage
# baseline aplica em banco vazio; foreign_key_check vazio; integrity_check = ok; 61 tabelas STRICT
```
