# 0200 — Fundação V2: domínio, storage e corte limpo

- **Status:** aceito
- **Data:** 2026-11-22
- **Contexto:** a V1 acumulou 59 migrations, 36 módulos em `db/`, god-files
  (alguns com ~1.900 linhas) e o A9 deixou reexports transitórios. A auditoria
  encontrou 66 tabelas, 5 delas representando conversas e 3 sem nenhuma
  referência no código.

## Decisão

1. **Oito crates V2:** `opendesk-domain`, `opendesk-storage`, `opendesk-engine`,
   `opendesk-recipes`, `opendesk-mcp`, `opendesk-channels`,
   `opendesk-enterprise` e o shell `opendesk`.
2. **Domínio puro** (`opendesk-domain`): IDs tipados, `TaskStatus`, `RunStatus`,
   `HitlKind`/`HitlStatus`, erros, eventos e ports. Sem SQL, HTTP, filesystem ou
   Tauri.
3. **Baseline única** (`crates/opendesk-storage/migrations/0001_v2_baseline.sql`):
   61 tabelas `STRICT`, tipos normalizados, seeds preservados e índices de
   cursor. As 59 migrations V1 foram removidas do core.
4. **Pools:** leitura concorrente com `query_only`, writer serializado (1
   conexão) e write-buffer em lote. Pragmas no pool (WAL, `NORMAL`, 5 s,
   `cache_size=-64000`, FK), porque `PRAGMA` dentro de migration é no-op.
5. **Conversas unificadas:** `conversations` + `messages` substituem
   `chat_threads`, `chat_messages`, `chat_turns`, `task_messages` e
   `task_interactions`. HITL é `kind=hitl_request`; turno é `kind=turn`.
6. **Clean break:** o V2 recusa um banco V1 e orienta
   `opendesk-cli migrate-v1-export --db <v1.db> --out <dir>`, que exporta
   workspaces, receitas e settings com manifesto SHA256 e sem segredos.
7. **Documentação:** SPEC/ARCHITECTURE únicos; ADRs, progresso e releases V1
   congelados em `docs/archive/v1/`.
8. **`opendesk-core` vira fachada temporária** apontando para domínio + storage;
   a extinção é a Onda 4.

## Consequências

- Um banco V1 não é aberto por acidente; a mensagem aponta o export.
- O schema V2 não tem as 3 tabelas mortas (`config_change_log`,
  `memory_entities`, `runtime_events`) nem a coluna `captured_at`.
- O shell continua compilando durante a transição, mas o SQL já pertence ao
  storage.
- Testes na Onda 1: 340 shell + 286 core + 19 domínio + 15 storage (660).
- Ondas 2–4 seguem o plano em `/home/rafael/.opencode/plan/opendesk-v2-refactor.md`.
