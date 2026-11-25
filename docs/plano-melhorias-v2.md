# Plano de melhorias v2 — estabilidade, ergonomia, voz, backup e scripts MCP

**Data:** 2026-11-25
**Base:** OpenDesk 1.9.0
**Escopo:** 10 microspecs atômicas (`P0` a `P2`), execução sequencial pelo DeepSeek v4 no OpenCode.

**TL;DR:** Transformei os seus 12 pedidos em uma especificação determinística de **10 microspecs atômicas (`P0` a `P2`)** pronta para execução sequencial pelo DeepSeek v4 no OpenCode, respeitando o limite rígido de 500 linhas/arquivo e zero `unwrap()` já existente no repositório. **Ponto crítico imediato:** o erro `(code: 11) database disk image is malformed` ao criar receitas indica corrupção física/índice no SQLite local (ou falha de escrita/WAL), então a **Fase 1 (`P0`)** corrige o fluxo de receitas e adiciona verificação de integridade + autorreparo seguro no boot antes de qualquer mudança de UI.

---

### Crítica Rápida e Pontos Cegos (Antes de Codar)

* **Erro `(code: 11) database disk image is malformed`:** Não é um bug cosmético do formulário de receitas — é corrupção real no `opendesk.db` (comum após crash durante checkpoint WAL, troca de arquivo sem `-wal`/`-shm` ou índice FTS/B-tree corrompido). Só mexer na tela de receitas não resolve o problema na sua máquina; é obrigatório ter `PRAGMA quick_check` no startup e rotina de reparo seguro (`VACUUM INTO` / dump de recuperação + backup automático do `.db` corrompido).
* **TTS no Linux/Tauri (WebKitGTK):** Depender de `window.speechSynthesis` no WebKitGTK falha silenciosamente na maioria das distros Linux, e tocar áudio via CLI no backend quebra em modo PWA/remoto. O caminho mais simples e confiável é **backend sintetiza WAV/OGG (Piper / `espeak-ng` / API compatível) → retorna bytes na API → frontend toca via `<audio>` nativo**.
* **Scripts como MCP sem overengineering:** Criar 1 processo servidor MCP por script vira pesadelo de memória e processos órfãos. A arquitetura limpa é **um único servidor MCP local (`opendesk mcp-scripts`)** que lê a lista de scripts cadastrados e expõe cada um como uma tool dinâmica (`script_<slug>`), injetando também o prompt de uso na camada do agente.
* **Botão "Testar Tudo" vs. Custo/Tempo:** Rodar tarefa + rotina + receita com LLM real consome tokens e leva 20–60s. Por isso o teste precisa de **dois modos explícitos**: **Rápido (offline/dry-run, ~2s, $0)** e **Completo com IA (execução real mínima + limpeza automática)**.
* **Compatibilidade de Rotas e Testes E2E:** O repositório tem 55+ specs Playwright (como `worktrees-ui.spec.ts`, `files-viewers.spec.ts`, `recipes-ui.spec.ts`). Ao unificar `Worktrees` em `Execuções` e `Arquivos` em `Buscar`, as rotas antigas (`/worktrees`, `/files`) devem redirecionar para as novas abas (`/runs?tab=worktrees`, `/search?tab=files`) para não quebrar links profundos nem testes.

---

## 1. Objetivo Final e Escopo

### Objetivo Final

Entregar o ciclo de melhorias de estabilidade, ergonomia, simetria de dados, voz, backup completo, scripts-MCP e auto-teste do OpenDesk v1.7+, mantendo:

* Zero regressões nos testes Rust, Vitest e Playwright existentes.
* Todos os arquivos `.rs`, `.ts` e `.tsx` de produção `<= 500` linhas e zero `.unwrap()`/`.expect()` fora de `#[cfg(test)]`.
* Comportamento **fail-closed** em integridade do SQLite, importações, restore de backup e execução de scripts.

### Fora de Escopo (NÃO Alterar)

* Reestruturação dos 8 crates Rust do workspace ou reabertura do `opendesk-core`.
* Alteração destrutiva nas 61 tabelas existentes de `0001_v2_baseline.sql` (apenas adição de 1 tabela isolada `script_tools` via migration `0002_script_tools.sql` ou bloco idempotente `CREATE TABLE IF NOT EXISTS` na inicialização V2).
* Reescrita de canais externos (WhatsApp/Signal/Email) fora do que toca anexos no chat.
* Mudanças de paleta/design system definidas em `DESIGN.md`.

---

## 2. Mapa de Arquivos

| Arquivo | Ação | Motivo |
| --- | --- | --- |
| `src-tauri/crates/opendesk-storage/src/db.rs`, `pool.rs`, `integrity.rs` (novo) | Criar / Modificar | Preflight `PRAGMA quick_check`, rotina `repair_database` segura e diagnóstico claro de `SQLITE_CORRUPT (11)`. |
| `src-tauri/crates/opendesk-recipes/src/store/recipes.rs`, `src-tauri/src/api/recipes.rs` | Ler / Modificar | Auditar e corrigir criação/salvamento de receitas, garantir escrita atômica e erro acionável. |
| `src/components/layout/Sidebar.tsx`, `MobileNav.tsx`, `CommandPalette.tsx`, `src/App.tsx`, `src/lib/links.ts` | Modificar | Remover linhas redundantes (`Worktrees` e `Arquivos` do menu), adicionar redirects `/worktrees` → `/runs?tab=worktrees` e `/files` → `/search?tab=files`. |
| `src/pages/RunsPage.tsx`, `src/pages/WorktreesPage.tsx` | Modificar | Transformar `Worktrees` em aba dentro de `RunsPage` (`Execuções` \| `Worktrees`). |
| `src/pages/ChatPage.tsx`, `src/pages/chat/MetaBuilder.tsx` | Modificar | Transformar `Chat` e `Criador` (MetaBuilder) em abas no topo da página em vez de empilhados. |
| `src/pages/SearchPage.tsx`, `src/pages/FilesPage.tsx` | Modificar | Unificar `Buscar` e `Arquivos` em abas (padrão `Buscar`). |
| `src/pages/KanbanPage.tsx`, `src/components/kanban/KanbanBoard.tsx` | Modificar | Fazer todas as colunas (inclusive vazias) ocuparem 100% da altura vertical disponível. |
| `src/pages/RecipesPage.tsx`, `src/pages/recipes/YamlPanel.tsx`, `RecipeFlowCard.tsx`, `src/components/recipes/editors/{RecipeStepper,StepEditor,HitlEditor,SinkEditor}.tsx` | Modificar | YAML fechado por padrão, passos visualmente distintos com colapso individual + recolher/expandir todos. |
| `src/components/ui/icon-tag.tsx` (novo), `src/components/kanban/TaskCard.tsx`, `src/components/routines/RoutineCard.tsx`, `src/pages/recipes/RecipeListCard.tsx`, `src/pages/RunsPage.tsx` | Criar / Modificar | Manter horário visível; substituir tags verbosas (`prompt`, `melhor custo-benefício`, origem, tipo de gatilho) por ícones com `Tooltip`. |
| `src-tauri/src/chat.rs`, `src-tauri/src/api/chat.rs`, `src/lib/types/chat.ts`, `src/pages/chat/{Transcript,ChatComposer}.tsx` | Modificar | Persistir anexos enviados e recebidos em `metadata_json` e renderizar chips clicáveis que abrem o arquivo no `FileViewer`. |
| `src/components/viewer/{FileViewer,FilePreviewBody,preview-helpers}.tsx`, `ImageZoomPane.tsx` (novo), `JsonTreeView.tsx` (novo), `CsvTableView.tsx` (novo), `CodeTextView.tsx` (novo) | Criar / Modificar | Zoom + drag/pan real em imagens; árvore/busca/cópia em JSON; TOC/cópia/busca em MD; linhas/wrap/busca em código; tabela ordenável em CSV. |
| `src-tauri/src/api/{personal_context,memory,prompt_templates,starter_packs,workspaces_bundle}.rs`, `src/components/learning/{PersonalContextCard,StructuredMemoryCard}.tsx`, `src/components/settings/PromptTemplatesCard.tsx`, `src/pages/packs/PacksPageContent.tsx`, `src/pages/settings/WorkspacesCard.tsx` | Criar / Modificar | Garantir simetria 100% Export ↔ Import (Contexto Pessoal, Memória Estruturada, Prompts, Pacotes individuais/todos, Workspaces todos). |
| `src-tauri/src/backup.rs`, `src-tauri/src/api/backups.rs`, `src/components/settings/BackupsCard.tsx` | Modificar | Backup completo da workspace (arquivos + snapshot relacional completo do SQLite) e restore exato (in-place com backup de segurança ou como nova workspace). |
| `src-tauri/src/voice.rs`, `src-tauri/src/api/voice.rs`, `src/lib/use-voice-input.ts` (novo), `src/components/ui/voice-dictation-button.tsx` (novo), `src/components/settings/VoiceCard.tsx`, `src/components/kanban/task-form-modal.tsx`, `src/pages/CapturePage.tsx`, `src/pages/chat/ChatComposer.tsx`, `src/components/runs/RunLogComposer.tsx` | Criar / Modificar | Corrigir TTS (retorno de áudio WAV/OGG tocado no `<audio>`) e STT (gravação + transcrição); adicionar microfone reutilizável em criação de tarefas e compositores. |
| `src-tauri/crates/opendesk-storage/migrations/0002_script_tools.sql` (novo), `src-tauri/crates/opendesk-storage/src/repos/script_tools.rs` (novo), `src-tauri/src/script_tools.rs` (novo), `src-tauri/src/api/script_tools.rs` (novo), `src-tauri/src/mcp_scripts.rs` (novo), `src/components/integrations/ScriptToolsCard.tsx` (novo), `src/pages/IntegrationsPage.tsx` | Criar / Modificar | Gestão completa de Ferramentas (Scripts-como-MCP) com editor de script, schema de inputs, prompt do agente, teste inline, export/import e ponte MCP `opendesk mcp-scripts`. |
| `src-tauri/src/self_test.rs` (novo), `src-tauri/src/api/self_test.rs` (novo), `src/components/settings/SelfTestCard.tsx` (novo), `src/pages/SettingsPage.tsx` | Criar / Modificar | Botão "Testar tudo" nas Configurações cobrindo DB, motor, tarefa, rotina, receita, script-MCP e voz, com limpeza automática garantida. |

---

## 3. Contratos, Schemas e Invariantes

### 3.1 Integridade e Reparo do SQLite (`P0.1`)

```rust
#[derive(Serialize)]
pub struct DbIntegrityReport {
    pub ok: bool,
    pub journal_mode: String,
    pub errors: Vec<String>,
}

/// Executa `PRAGMA quick_check(5)`. Se falhar, não abre o app em estado corrompido:
/// gera backup `<db>.corrupt-<ts>`, tenta recuperação limpa para `<db>.repaired`
/// (via `VACUUM INTO` / dump de tabelas legíveis) e valida `PRAGMA integrity_check`
/// antes da substituição atômica.
pub fn check_and_repair_database(db_path: &Path) -> StorageResult<DbIntegrityReport>;
```

* **Invariante fail-closed:** Nunca sobrescrever o `.db` original sem antes copiar `.db`, `.db-wal` e `.db-shm` para `<db>.corrupt-<timestamp>`. Se o banco recuperado não passar em `PRAGMA integrity_check == "ok"`, abortar e exibir erro acionável com o caminho do backup.

### 3.2 Contrato de Anexos no Chat (`P1.2`)

```typescript
export interface ChatAttachmentMeta {
  name: string;
  path: string;          // caminho relativo à workspace (validado contra path traversal)
  mime?: string | null;
  size?: number | null;
  direction: "sent" | "received";
  source?: "ui" | "agent" | "telegram" | "signal" | "email";
}
```

* Persistido em `ChatMessage.metadata_json` sob a chave `"attachments": ChatAttachmentMeta[]`.
* Ao clicar no chip, o frontend abre `LazyFileViewer` com `workspaceId` + `path` (reaproveitando o endpoint seguro `/api/files/content` / `/api/files/raw` que já aplica boundary da workspace).

### 3.3 Contrato Universal de Export / Import (`P1.4`)

Todo bundle JSON exportado segue envelope versionado único:

```json
{
  "schema_version": 1,
  "kind": "personal_context | structured_memory | prompt_templates | pack | packs_bundle | workspaces_bundle | script_tools",
  "exported_at": "2026-11-25T12:00:00Z",
  "source_workspace": "optional-name",
  "items": []
}
```

* **Parâmetro obrigatório na importação:** `mode: "merge" | "replace"`.
* **Invariante fail-closed:** Validar `schema_version`, `kind` e todos os itens em memória antes de abrir transação SQLite. Qualquer item inválido aborta 100% da importação (HTTP 400 com lista de erros por índice). Nenhum segredo (token/senha/API key) entra no export — apenas `secret_ref`.

### 3.4 Contrato de Backup Completo da Workspace (`P1.5`)

Arquivo `.zip` contendo:

1. `manifest.json`: `{ version: 2, workspace_id, workspace_name, created_at, app_version, file_count, state_sha256, files: [{ path, sha256, size }] }`.
2. `state/workspace-state.json`: dump completo de todas as tabelas da workspace (tasks, runs, artifacts metadata, schedules, recipes, recipe_versions, conversations, messages, personal_context, memory_namespaces/notes/acl, router_rules/models/budget, governance_profiles/goals/delegations, starter_packs_installed, calendar_sources/events, inbound_sources/outbound_sinks sem segredos, script_tools).
3. `files/...`: espelho integral do diretório da workspace (incluindo `.opencode/`, `input/`, `output/`).

* **Restore (`POST /api/backups/{id}/restore`):** aceita `{ mode: "replace_in_place" | "new_workspace", target_path?: string, new_name?: string }`. Antes de `replace_in_place`, cria automaticamente um backup de segurança `pre-restore-<ts>` e executa a restauração do SQLite em transação única após validar 100% dos SHA-256 do manifesto.

### 3.5 Schema e Contrato de Ferramentas / Scripts como MCP (`P2.2`)

```sql
-- src-tauri/crates/opendesk-storage/migrations/0002_script_tools.sql
CREATE TABLE IF NOT EXISTS script_tools (
  id TEXT PRIMARY KEY,
  workspace_id TEXT REFERENCES workspaces(id) ON DELETE CASCADE, -- NULL = global
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  agent_prompt TEXT NOT NULL DEFAULT '',     -- instruções gerenciáveis de quando/como usar
  runtime TEXT NOT NULL,                     -- 'bash' | 'python3' | 'node' | 'uv' | 'custom'
  custom_command TEXT,
  script_body TEXT NOT NULL,
  input_schema_json TEXT NOT NULL DEFAULT '{"type":"object","properties":{}}',
  env_secret_refs_json TEXT NOT NULL DEFAULT '{}', -- mapa ENV_VAR -> secret_ref (nunca plaintext)
  timeout_secs INTEGER NOT NULL DEFAULT 30,
  require_approval INTEGER NOT NULL DEFAULT 0,
  enabled INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(workspace_id, slug)
) STRICT;
```

* **Protocolo de Execução do Script:** recebe os argumentos validados contra `input_schema_json` via `stdin` (JSON) e via `OPENDESK_INPUT_JSON` (para scripts simples), com `cwd` na raiz da workspace, `kill_on_drop(true)` + process group + timeout rígido (`timeout_secs`). Retorna `stdout` (limitado a 1 MiB) como resultado MCP; exit code `!= 0` retorna `isError: true` com `stderr` sanitizado.
* **Exposição MCP:** subcomando `opendesk mcp-scripts` (stdio JSON-RPC 2.0) registrado automaticamente em `mcp.servers.opendesk-scripts` (igual ao `opendesk-control`), listando todas as `script_tools` habilitadas (globais + workspace) como `script_<slug>` com `description` combinando `description` + `agent_prompt`.

### 3.6 Contrato de Voz STT / TTS (`P2.1`)

* `POST /api/voice/tts` `{ text, voice?, speed? }` → retorna `audio/wav` (ou `audio/ogg`) com header `X-Voice-Provider` (nunca `200 OK` vazio; se nenhum backend de TTS estiver instalado/configurado, retorna `503` com `error` e `install_hint` acionável).
* `POST /api/voice/stt` (`multipart/form-data` ou JSON `{ audio_base64, mime, language? }`) → retorna `{ text, language, duration_ms, provider }`.
* **No Frontend:** `<audio>` reproduz o `Blob` retornado pelo `POST /api/voice/tts` (garantindo áudio real no Tauri Linux/Windows/macOS e no PWA).

### 3.7 Contrato do "Testar Tudo" (`P2.3`)

* `POST /api/diagnostics/self-test` `{ workspace_id?, mode: "fast" | "full_ai" }` → retorna:

```typescript
export interface SelfTestStep {
  id: "db" | "engine" | "task" | "routine" | "recipe" | "script_tool" | "voice" | "cleanup";
  label: string;
  status: "pass" | "warn" | "fail" | "skipped";
  duration_ms: number;
  detail: string;
}
export interface SelfTestReport {
  ok: boolean;
  mode: "fast" | "full_ai";
  total_ms: number;
  steps: SelfTestStep[];
}
```

* **Invariante:** A etapa `cleanup` roda **sempre** (mesmo se `task`/`routine`/`recipe` falharem) apagando todos os registros prefixados com `__selftest_<run_uuid>__`.

---

## 4. Fases Atômicas de Implementação (Microspecs)

### Fase 1 (`P0.1`) — Diagnóstico/Reparo do SQLite `(code: 11)` e Correção da Criação de Receitas

* **Arquivos da fase:**
* Ler: `src-tauri/crates/opendesk-storage/src/db.rs`, `pool.rs`, `migrations/0001_v2_baseline.sql`, `src-tauri/crates/opendesk-recipes/src/store/recipes.rs`, `src-tauri/src/api/recipes.rs`, `src-tauri/src/main.rs`.
* Criar: `src-tauri/crates/opendesk-storage/src/integrity.rs`.
* Modificar: `src-tauri/crates/opendesk-storage/src/db.rs`, `lib.rs`, `src-tauri/crates/opendesk-recipes/src/store/recipes.rs`, `src-tauri/src/api/recipes.rs`, `src-tauri/src/main.rs`.

* **Passos determinísticos:**
1. Em `opendesk-storage/src/integrity.rs`, implementar `check_database_integrity(db_path)` executando `PRAGMA quick_check(5)` em conexão direta SQLite e `repair_database(db_path)` que:
* Copia `opendesk.db` (+ `-wal` e `-shm` se existirem) para `opendesk.db.corrupt-<YYYYMMDD-HHMMSS>`.
* Tenta `REINDEX;` + `VACUUM INTO '<temp_repaired>'` em uma cópia de trabalho; se `VACUUM INTO` falhar por página corrompida, extrai linhas íntegras tabela a tabela (ou invoca `sqlite3 <corrupt> ".recover"` se disponível no PATH) para um banco novo criado com `0001_v2_baseline.sql`.
* Executa `PRAGMA integrity_check;` no banco reparado; só substitui o original atomicamente se retornar `"ok"`.

2. No startup (`src-tauri/src/main.rs` e `opendesk-daemon/src/main.rs`), rodar `check_database_integrity` logo após a verificação V1/V2. Se corrompido: no modo desktop mostrar diálogo nativo ("Banco corrompido detectado (code 11). Deseja reparar com backup automático?"); via CLI expor `opendesk repair-db [--db <path>]`.
3. Auditar `src-tauri/src/api/recipes.rs` e `opendesk-recipes/src/store/recipes.rs` no fluxo `POST /api/recipes` e `PUT /api/recipes/{id}`:
* Verificar se a gravação do YAML em disco + indexação no SQLite está protegida contra escrita concorrente com o `watcher` (que pode disparar `upsert` simultâneo no mesmo `recipe_id` durante a criação).
* Garantir que a criação escreva o arquivo atomicamente (`.tmp` → rename) e faça `INSERT ... ON CONFLICT DO UPDATE` dentro de transação explícita no writer pool (`opendesk-storage`).
* Traduzir erro SQLite `code: 11` na camada de erro (`AppError`) para mensagem acionável orientando `opendesk repair-db` / diálogo de reparo em vez de erro cru.

4. Escrever testes unitários em `integrity.rs` (simulando corrupção em bytes do `.db` e provando detecção + backup + falha fechada) e teste de integração de criação de receita em `src-tauri/src/api/tests/recipes_runtime.rs`.

* **Refatoração/Simplificação/Limpeza acoplada:** Garantir que `opendesk doctor` também reporte `PRAGMA integrity_check` detalhado (não só `quick_check`).
* **Edge cases obrigatórios:**
* Banco inexistente (primeiro boot) deve passar pelo check sem erro.
* Nunca deixar arquivos `-wal`/`-shm` antigos ao substituir um `.db` reparado (eles corrompem o banco novo na primeira abertura!).

* **Validação:**
* `cd src-tauri && cargo test -p opendesk-storage integrity && cargo test recipes` → **PASS**.

---

### Fase 2 (`P0.2`) — Navegação Consolidada (Abas) + Kanban 100% Altura Vertical

* **Arquivos da fase:**
* Ler e modificar: `src/App.tsx`, `src/components/layout/Sidebar.tsx`, `src/components/layout/MobileNav.tsx`, `src/components/layout/CommandPalette.tsx`, `src/lib/links.ts`, `src/pages/RunsPage.tsx`, `src/pages/WorktreesPage.tsx`, `src/pages/ChatPage.tsx`, `src/pages/SearchPage.tsx`, `src/pages/FilesPage.tsx`, `src/pages/KanbanPage.tsx`, `src/components/kanban/KanbanBoard.tsx`.

* **Passos determinísticos:**
1. **Execuções + Worktrees:**
* Extrair o conteúdo interno de `WorktreesPage.tsx` (sem `PageHeader` duplicado) como componente `WorktreesTabContent` (mantendo o arquivo `<= 500` linhas).
* Em `RunsPage.tsx`, adicionar `SectionTabs` no header/topo com abas `Execuções` (`runs`) e `Worktrees` (`worktrees`), sincronizadas com `?tab=worktrees`.
* Remover `Worktrees` da `Sidebar.tsx` e `MobileNav.tsx`; em `App.tsx`, redirecionar `/worktrees` → `/runs?tab=worktrees` e atualizar `links.ts` e `CommandPalette.tsx`.

2. **Chat + Criador (MetaBuilder):**
* Em `ChatPage.tsx`, substituir o layout empilhado (Chat + MetaBuilder um embaixo do outro) por `SectionTabs` com `Chat` (`chat`, padrão) e `Criador` (`builder`), sincronizado com `?tab=builder`.

3. **Buscar + Arquivos:**
* Extrair o conteúdo interno de `FilesPage.tsx` como `FilesTabContent`.
* Em `SearchPage.tsx`, adicionar `SectionTabs` com `Buscar` (`search`, sempre padrão) e `Arquivos` (`files`), sincronizado com `?tab=files`.
* Remover `Arquivos` da `Sidebar.tsx` e `MobileNav.tsx`; redirecionar `/files` → `/search?tab=files` preservando query params (`?path=...`).

4. **Kanban altura total:**
* Em `KanbanPage.tsx`, usar `PageBodyFill` com `flex flex-col h-full min-h-0`.
* Em `KanbanBoard.tsx`, garantir que o wrapper horizontal do grid/flex tenha `h-full min-h-0 flex-1 items-stretch` e cada coluna tenha `flex h-full min-h-full flex-col` com `CardContent` `flex-1 overflow-y-auto`, preenchendo até a base da viewport mesmo com 0 cards.

* **Refatoração/Simplificação/Limpeza acoplada:** Atualizar testes E2E afetados (`worktrees-ui.spec.ts`, `files-search-ui.spec.ts`, `files-viewers.spec.ts`, `scroll.spec.ts`) para confirmar que tanto as novas abas quanto os redirects legados funcionam.
* **Edge cases obrigatórios:**
* Deep link `/files?path=output/relatorio.md` deve abrir `SearchPage` já na aba `Arquivos` com o arquivo selecionado.
* Em `KanbanBoard`, não pode criar scroll vertical duplo na página (apenas dentro de cada coluna quando cheia).

* **Validação:**
* `npm run check && npm run lint && npm run test:ui` → **PASS**.
* `python3 scripts/check-frontend-limits.py` → **PASS**.

---

### Fase 3 (`P1.1`) — UX de Receitas (YAML Fechado + Passos Colapsáveis) + Ícones com Tooltip nos Cards

* **Arquivos da fase:**
* Criar: `src/components/ui/icon-tag.tsx`, `src/components/ui/icon-tag.test.tsx`.
* Ler e modificar: `src/pages/RecipesPage.tsx`, `src/pages/recipes/YamlPanel.tsx`, `src/pages/recipes/RecipeFlowCard.tsx`, `src/pages/recipes/RecipeListCard.tsx`, `src/components/recipes/editors/{RecipeStepper,StepEditor,HitlEditor,SinkEditor}.tsx`, `src/components/routines/RoutineCard.tsx`, `src/components/kanban/TaskCard.tsx`, `src/pages/RunsPage.tsx`.

* **Passos determinísticos:**
1. **Editor de Receitas:**
* Em `RecipesPage.tsx` / `YamlPanel.tsx`, inicializar `yamlOpen = false` (colapsado por padrão) com botão claro `"Mostrar YAML"` / `"Ocultar YAML"`.
* Em `RecipeStepper.tsx`, adicionar estado `collapsedIds: Set<string>` com botões no topo `"Recolher todos"` e `"Expandir todos"`.
* Em `StepEditor.tsx`, `HitlEditor.tsx` e `SinkEditor.tsx`, padronizar cabeçalho de cada passo com: número sequencial (`#1`, `#2`...), borda lateral colorida por tipo (Step = azul, HITL = âmbar, Sink = esmeralda), título/ID + resumo curto quando minimizado, botão chevron de minimizar/expandir e linha conectora vertical (`border-l`) entre passos consecutivos.

2. **Componente `<IconTag/>` com Tooltip:**
* Criar `src/components/ui/icon-tag.tsx` que renderiza um ícone compacto (`14px`) dentro de `<Tooltip content="{label}">` com `aria-label={label}` e `tabIndex={0}` (acessível por teclado).
* Mapear ícones canônicos em `icon-tag.tsx`:
* Política de router: `melhor custo-benefício` (`Scale`), `máxima qualidade` (`Sparkles`), `mais rápido/barato` (`Zap`), `manual` (`SlidersHorizontal`).
* Origem / Tipo: `prompt` (`Terminal`), `receita` (`Workflow`), `rotina` (`CalendarClock`), `manual` (`User`), `io_bus` (`Webhook`), `delegação` (`GitBranch`).
* Prioridade / Care level: ícones distintos com tooltip explícito.

* Substituir as tags verbosas por `<IconTag/>` em `RoutineCard.tsx`, `TaskCard.tsx`, `RecipeListCard.tsx` e `RunsPage.tsx`, **mantendo sempre visível em texto o horário de execução** (`next_run_at` / `last_run_at` / duração).

* **Refatoração/Simplificação/Limpeza acoplada:** Centralizar o mapeamento de ícones em `icon-tag.tsx` para não espalhar `switch` de ícones pelas páginas.
* **Edge cases obrigatórios:**
* Acessibilidade WCAG: nenhum ícone pode ficar sem `aria-label` e tooltip (verificar com `a11y.spec.ts`).
* Ao adicionar um novo passo em uma receita, o passo recém-criado deve abrir **expandido** automaticamente mesmo se os outros estiverem recolhidos.

* **Validação:**
* `npm run check && npm run lint && npm run test:ui` → **PASS**.
* `python3 scripts/check-frontend-limits.py` → **PASS**.

---

### Fase 4 (`P1.2`) — Anexos e Arquivos Clicáveis no Chat (Enviados e Recebidos)

* **Arquivos da fase:**
* Ler e modificar: `src-tauri/src/chat.rs`, `src-tauri/src/api/chat.rs`, `src-tauri/src/api/attachments.rs`, `src/lib/types/chat.ts`, `src/pages/chat/ChatComposer.tsx`, `src/pages/chat/Transcript.tsx`, `src/pages/ChatPage.tsx`.

* **Passos determinísticos:**
1. No backend (`api/chat.rs` e `chat.rs`), aceitar `attachments: Vec<ChatAttachmentMeta>` no `POST /api/chat/threads/{id}/messages` e persistir em `metadata_json` da mensagem do usuário com `direction: "sent"`.
2. Quando o turno do assistente concluir (ou quando uma mensagem vier de Telegram/canal com arquivo ou quando o agente criar/referenciar arquivos na workspace durante o turno), extrair os arquivos produzidos/anexados da run (`artifacts` ligados ao `run_id` ou caminhos explícitos dentro da workspace) e gravar em `metadata_json.attachments` da mensagem do assistente com `direction: "received"`.
3. Em `src/pages/chat/Transcript.tsx`, parsear `metadata_json.attachments` (com fallback seguro se JSON inválido) e renderizar chips compactos de arquivo (`Paperclip` + ícone de direção `↑ Enviado` / `↓ Recebido` + nome do arquivo + tamanho formatado).
4. Clicar em qualquer chip abre um modal `LazyFileViewer` diretamente sobre o chat (ou botão secundário `"Abrir em Arquivos"` que navega para `/search?tab=files&path=...`).
5. Em `ChatComposer.tsx`, exibir preview dos arquivos anexados antes do envio e garantir que o envio passe `attachments` no payload.

* **Refatoração/Simplificação/Limpeza acoplada:** Validar no backend que todo `attachment.path` está contido na raiz da workspace ativa (rejeitar `..` e paths absolutos externos — fail-closed).
* **Edge cases obrigatórios:**
* Se o arquivo anexado tiver sido deletado posteriormente do disco, clicar no chip deve mostrar erro amigável no viewer ("Arquivo não encontrado no disco") sem travar o chat.
* Mensagens antigas sem `metadata_json` ou sem chave `attachments` devem renderizar normalmente.

* **Validação:**
* `cd src-tauri && cargo test chat` → **PASS**.
* `npm run check && npm run test:ui` → **PASS**.

---

### Fase 5 (`P1.3`) — Visualizador Rico de Arquivos (Zoom/Pan em Imagens + JSON, MD, Código, CSV)

* **Arquivos da fase:**
* Criar: `src/components/viewer/ImageZoomPane.tsx`, `src/components/viewer/JsonTreeView.tsx`, `src/components/viewer/CsvTableView.tsx`, `src/components/viewer/CodeTextView.tsx`, `src/components/viewer/ImageZoomPane.test.tsx`.
* Ler e modificar: `src/components/viewer/FileViewer.tsx`, `src/components/viewer/FilePreviewBody.tsx`, `src/components/viewer/preview-helpers.ts`, `src/components/viewer/FileViewer.test.tsx`.

* **Passos determinísticos:**
1. **`ImageZoomPane.tsx` (PNG, JPG, WebP, GIF, SVG):**
* Estado controlado de `scale` (`0.25` a `6.0`), `offset: { x, y }` e `fitMode: boolean`.
* Corrigir o bug de zoom: aplicar `transform: translate(${offset.x}px, ${offset.y}px) scale(${scale})` com `transformOrigin: "center center"` sobre container `overflow-hidden select-none`.
* Suportar: botões `−` / `+` / `% atual` / `Ajustar` / `100%`, zoom via `onWheel` (preventDefault), **pan/arrastar com pointer events** (`onPointerDown`/`Move`/`Up` + `setPointerCapture`, cursor `grab`/`grabbing`), duplo clique para alternar `Ajustar` ↔ `200%`, e reset automático ao trocar de arquivo.

2. **`JsonTreeView.tsx` (`.json`, `.jsonl`):**
* Visualização em árvore colapsável (níveis expand/collapse, botão "Expandir tudo" / "Recolher tudo"), contador de chaves/itens, busca de chave/valor com filtro/destaque, botão copiar nó/JSON formatado, e toggle `Árvore` | `Código`.

3. **Markdown aprimorado (`FilePreviewBody.tsx`):**
* Extrair headings (`h1`–`h3`) em barra lateral/popover de **Sumário (TOC)** clicável quando houver `>= 3` títulos; adicionar botão copiar em cada bloco `<pre><code>`, botão copiar markdown bruto, busca de texto e toggle `Visualização` | `Fonte`.

4. **`CodeTextView.tsx` (código, `.yaml`, `.toml`, `.rs`, `.ts`, `.py`, `.log`, `.txt`):**
* Números de linha alinhados, barra de busca interna com contador de ocorrências (`1/12`) e botões anterior/próximo, toggle de quebra de linha (`Wrap`), e botão copiar conteúdo.

5. **`CsvTableView.tsx` (`.csv`, `.tsv`):**
* Parser seguro respeitando aspas/escapes, tabela com cabeçalho fixo (`sticky`), ordenação por coluna clicável, filtro rápido por texto e toggle `Tabela` | `Bruto`.

6. Integrar todos em `FilePreviewBody.tsx` mantendo cada arquivo `<= 500` linhas.

* **Refatoração/Simplificação/Limpeza acoplada:** Extrair helpers de detecção de tipo (imagem/json/csv/md/html/code) para `preview-helpers.ts` sem duplicação.
* **Edge cases obrigatórios:**
* Arquivos grandes (`> 500 KB` ou truncados pelo backend) não devem congelar o DOM: para JSON/CSV muito grandes (`> 5.000` nós/linhas), exibir banner de aviso e limitar renderização inicial com botão "Carregar mais" ou abrir em modo texto.
* Em `ImageZoomPane`, prevenir drag nativo fantasma da tag `<img>` (`draggable={false}`).

* **Validação:**
* `npm run check && npm run lint && npm run test:ui` → **PASS**.
* `python3 scripts/check-frontend-limits.py` → **PASS**.

---

### Fase 6 (`P1.4`) — Simetria Universal de Importação e Exportação

* **Arquivos da fase:**
* Ler e modificar (backend): `src-tauri/src/api/personal_context.rs`, `src-tauri/src/api/memory.rs`, `src-tauri/src/api/prompt_templates.rs`, `src-tauri/src/api/starter_packs.rs`, `src-tauri/src/api/mod.rs`.
* Criar (backend): `src-tauri/src/api/bundles.rs` (export/import em lote de pacotes customizados/internos e de todos os workspaces).
* Ler e modificar (frontend): `src/components/learning/PersonalContextCard.tsx`, `src/components/learning/StructuredMemoryCard.tsx`, `src/components/settings/PromptTemplatesCard.tsx`, `src/pages/packs/PacksPageContent.tsx`, `src/components/packs/PacksToolbar.tsx`, `src/pages/settings/WorkspacesCard.tsx`.
* Criar (frontend): `src/lib/import-export.ts` (helper único de download JSON + file picker + validação de envelope).

* **Passos determinísticos:**
1. Criar `src/lib/import-export.ts` com `exportJsonBundle(kind, items, filename)` e `pickJsonBundle(expectedKind)` (valida `schema_version` e `kind` fail-closed).
2. **Contexto Pessoal (`PersonalContextCard`):**
* Backend: `GET /api/personal-context/export?workspace_id=` e `POST /api/personal-context/import` (`mode: "merge" | "replace"`).
* Frontend: adicionar botões **Exportar JSON** e **Importar JSON** (além do botão criar regra).

3. **Memória Estruturada (`StructuredMemoryCard`):**
* Backend: adicionar `POST /api/memory/import` (espelhando o export já existente, recriando namespaces, ACLs e notas em transação).
* Frontend: adicionar botão **Importar JSON** ao lado de Exportar JSON.

4. **Prompts (`PromptTemplatesCard`):**
* Backend: adicionar `POST /api/prompt-templates/import` validando que todas as chaves pertencem aos IDs conhecidos de `PromptTemplateField` (rejeita chaves desconhecidas com 400).
* Frontend: adicionar botão **Importar JSON** ao lado de Exportar.

5. **Pacotes e Workspaces em lote (`api/bundles.rs`):**
* `GET /api/starter-packs/{id}/export` e `POST /api/starter-packs/import` (exportar/importar um pacote individual como JSON/ZIP).
* `GET /api/bundles/packs/export` e `POST /api/bundles/packs/import` (todos os pacotes + itens customizados da workspace).
* `GET /api/bundles/workspaces/export` e `POST /api/bundles/workspaces/import` (exportar/importar definições, configs, receitas, rotinas, regras e contextos de **todos os workspaces**, com opção `merge` ou `replace`).
* Expor botões simétricos em `PacksToolbar.tsx` / `PacksPageContent.tsx` e `WorkspacesCard.tsx`.

6. Auditar com `rg -i "export" src/` qualquer outra tela que tenha Export sem Import (ou vice-versa) e completar a simetria.

* **Refatoração/Simplificação/Limpeza acoplada:** Reaproveitar um único diálogo de confirmação para modo `Mesclar` vs `Substituir` na importação.
* **Edge cases obrigatórios:**
* Importar um arquivo de `prompt_templates` dentro de `structured_memory` deve falhar imediatamente com erro claro (`kind` incompatível).
* Modo `replace` deve sempre exigir confirmação explícita (`confirmAction`).

* **Validação:**
* `cd src-tauri && cargo test personal_context && cargo test memory && cargo test prompt_templates && cargo test bundles` → **PASS**.
* `npm run check && npm run lint && npm run test:ui` → **PASS**.

---

### Fase 7 (`P1.5`) — Backup e Restore Completo da Workspace (Arquivos + Estado SQLite)

* **Arquivos da fase:**
* Ler e modificar: `src-tauri/src/backup.rs`, `src-tauri/src/api/backups.rs`, `src-tauri/src/api/tests/backups.rs`, `src/components/settings/BackupsCard.tsx`, `src/lib/types/backup.ts`.

* **Passos determinísticos:**
1. Em `src-tauri/src/backup.rs`, evoluir o formato do backup para `version: 2`:
* Além de arquivar todos os arquivos da workspace (`files/...` com SHA-256 no `manifest.json`), serializar `state/workspace-state.json` com **todas as tabelas ligadas ao `workspace_id`** (workspace metadata, tasks, runs, artifacts, schedules, recipes, recipe_versions, conversations, messages, personal_context, memory, router_rules/models/budget, governance, starter_packs, calendar, io_bus sem segredos, script_tools).
* Calcular `state_sha256` e gravar no `manifest.json`.
* Garantir que backups `version: 1` antigos continuem legíveis apenas para restauração de arquivos (sem crash).

2. Implementar `restore_workspace` com 2 modos:
* `replace_in_place`: (a) valida todos os SHA-256 do ZIP em memória/staging antes de tocar em nada; (b) cria backup automático de segurança da workspace atual; (c) restaura arquivos para diretório temporário irmão e faz troca segura; (d) em transação SQLite única, apaga registros antigos daquela `workspace_id` e reinserta integralmente `state/workspace-state.json`.
* `new_workspace`: cria novo `workspace_id`, remapeia todas as FKs em `workspace-state.json` para o novo ID/path e grava arquivos + estado no destino escolhido.

3. Atualizar `BackupsCard.tsx` para permitir escolher o modo de restauração (`Substituir workspace exatamente como estava` vs `Restaurar como nova workspace`), exibir contagem de arquivos + registros de banco incluídos no backup, e adicionar botão **Importar arquivo `.zip` de backup** (simetria Export/Download ↔ Import/Upload).
4. Escrever teste round-trip em `api/tests/backups.rs` provando: cria workspace com arquivo + tarefa + rotina + receita + contexto pessoal → faz backup → altera arquivo e deleta tarefa/receita → restaura `replace_in_place` → confirma que arquivo, tarefa, rotina, receita e contexto voltaram **idênticos**.

* **Refatoração/Simplificação/Limpeza acoplada:** Se `backup.rs` passar de 450 linhas, dividir em `src-tauri/src/backup/{mod,export_state,restore_state,archive}.rs` (<= 500 linhas cada).
* **Edge cases obrigatórios:**
* ZIP adulterado (SHA-256 divergente ou path traversal `../`) deve abortar antes de modificar qualquer arquivo ou linha no banco.
* Nunca exportar segredos em texto plano no `workspace-state.json`.

* **Validação:**
* `cd src-tauri && cargo test backups` → **PASS**.
* `python3 scripts/check-rust-limits.py --root src-tauri/src` → **PASS**.

---

### Fase 8 (`P2.1`) — Correção Real de STT/TTS e Microfone Universal em Tarefas

* **Arquivos da fase:**
* Ler e modificar: `src-tauri/src/voice.rs`, `src-tauri/src/api/voice.rs`, `src-tauri/src/api/tests/voice.rs`, `src/components/settings/VoiceCard.tsx`, `src/pages/chat/ChatComposer.tsx`, `src/components/kanban/task-form-modal.tsx`, `src/pages/CapturePage.tsx`, `src/components/runs/RunLogComposer.tsx`.
* Criar: `src/lib/use-voice-input.ts`, `src/lib/use-voice-output.ts`, `src/components/ui/voice-dictation-button.tsx`, `src/components/ui/voice-dictation-button.test.tsx`.
* Remover: `src/pages/chat/useVoiceInput.ts` (substituído pelo hook compartilhado).

* **Passos determinísticos:**
1. **Corrigir TTS (`src-tauri/src/voice.rs` e `api/voice.rs`):**
* Garantir suporte funcional a provedores reais em ordem configurável: (a) Piper local (`piper` CLI gerando WAV em temp file lido e removido), (b) `espeak-ng` / `espeak` (gerando WAV via `--stdout`), (c) endpoint OpenAI-compatible `/v1/audio/speech` (usando `secret_ref`).
* `POST /api/voice/tts` retorna os bytes de áudio (`Content-Type: audio/wav` ou `audio/mpeg`) diretamente na resposta HTTP — nunca tentar tocar áudio cegamente no servidor sem feedback.
* Se nenhum provedor estiver disponível, retornar `503` com `error` e `install_hint` exato (ex.: `sudo apt install espeak-ng` ou instalar `piper` em Ferramentas).

2. **Corrigir STT (`src-tauri/src/voice.rs` e `api/voice.rs`):**
* Suportar conversão automática de `audio/webm`/`audio/ogg` (formato padrão do `MediaRecorder`) para WAV 16 kHz mono via `ffmpeg` quando necessário para `whisper-cli`/`whisper.cpp`, ou envio direto para endpoint STT OpenAI-compatible (`/v1/audio/transcriptions`).
* Retornar erro claro se microfone vier vazio (`< 1 KB`) ou se `ffmpeg`/binário whisper faltar.

3. **Frontend (`use-voice-output.ts`, `use-voice-input.ts`, `VoiceDictationButton.tsx`):**
* Criar `useVoiceOutput` usando `new Audio(URL.createObjectURL(blob))` com controle de `play`/`stop`, revogando Object URL no `onended`/`onerror` (zero memory leak).
* Extrair e melhorar `useVoiceInput` (gravação via `MediaRecorder`, tempo decorrido, envio para `/api/voice/stt`, inserção do texto transcrito no cursor ou anexado ao campo).
* Criar `<VoiceDictationButton onTranscript="{(text)"> ...} />` (com tooltip, estado gravando pulsando, estado transcrevendo com spinner e tratamento de erro de permissão de microfone).
* Integrar `<VoiceDictationButton/>` em: `TaskFormModal` (descrição e título), `CapturePage`, `ChatComposer` e `RunLogComposer`.
* Em `VoiceCard.tsx`, adicionar botões **"Testar fala (TTS)"** e **"Testar transcrição (STT)"** exibindo status real do backend detectado.

* **Refatoração/Simplificação/Limpeza acoplada:** Deletar `src/pages/chat/useVoiceInput.ts` após migrar `ChatPage`/`ChatComposer` para `src/lib/use-voice-input.ts`.
* **Edge cases obrigatórios:**
* Liberar todas as tracks do `MediaStream` (`stream.getTracks().forEach(t => t.stop())`) no encerramento/cancelamento da gravação para não manter o microfone preso no SO.
* Todos os processos `piper`/`espeak-ng`/`whisper`/`ffmpeg` no backend devem usar `kill_on_drop(true)` + timeout de 60s.

* **Validação:**
* `cd src-tauri && cargo test voice` → **PASS**.
* `npm run check && npm run lint && npm run test:ui` → **PASS**.

---

### Fase 9 (`P2.2`) — Gestão de Ferramentas (Scripts como MCP Local)

* **Arquivos da fase:**
* Criar (backend):
* `src-tauri/crates/opendesk-storage/migrations/0002_script_tools.sql`
* `src-tauri/crates/opendesk-storage/src/repos/script_tools.rs`
* `src-tauri/src/script_tools.rs` (executor seguro com timeout, stdin JSON, secret_ref, `kill_on_drop`)
* `src-tauri/src/api/script_tools.rs` (CRUD, `/test`, `/export`, `/import`)
* `src-tauri/src/mcp_scripts.rs` (servidor MCP stdio `opendesk mcp-scripts`)
* `src-tauri/src/api/tests/script_tools.rs`

* Modificar (backend): `src-tauri/crates/opendesk-storage/src/repos/mod.rs`, `src-tauri/src/api/mod.rs`, `src-tauri/src/proposals.rs` (registrar `opendesk-scripts` junto ao `opendesk-control`), `src-tauri/src/main.rs` (subcomando `mcp-scripts`), `src-tauri/src/api/prompt_layer.rs` (incluir resumo dos prompts das `script_tools` ativas no preview/contexto).
* Criar (frontend): `src/components/integrations/ScriptToolsCard.tsx`, `src/components/integrations/ScriptToolModal.tsx`.
* Modificar (frontend): `src/pages/IntegrationsPage.tsx` (adicionar seção **"Ferramentas (Scripts MCP)"**).

* **Passos determinísticos:**
1. Criar tabela `script_tools` (Seção 3.5) e repositório CRUD completo em `opendesk-storage/src/repos/script_tools.rs`.
2. Implementar executor em `src-tauri/src/script_tools.rs`:
* Validar `slug` (`^[a-z0-9][a-z0-9_-]{1,48}$`), validar `input` contra `input_schema_json` (usando `jsonschema` já presente no workspace ou validador estrito de tipos/required), resolver `env_secret_refs_json` via `SecretStore` (rejeitando qualquer valor em texto plano), executar script em arquivo temporário `0700` (ou `-c` conforme runtime) com `cwd = workspace_root`, `kill_on_drop(true)`, process group e `timeout_secs`.
* Retornar `{ ok, exit_code, stdout, stderr, duration_ms }` truncando stdout/stderr em 1 MiB.

3. Implementar `src-tauri/src/mcp_scripts.rs` expondo via stdio JSON-RPC 2.0:
* `tools/list`: retorna todas as `script_tools` com `enabled = 1` (globais + workspace atual) nomeadas `script_<slug>`, usando `description` + `agent_prompt` na descrição da tool e `input_schema_json` como `inputSchema`.
* `tools/call`: valida e executa a tool correspondente via `script_tools::execute` (se `require_approval = 1`, roteia por HITL antes de rodar).

4. Registrar `opendesk-scripts` automaticamente nos workspaces em `proposals::ensure_workspace_bridge` (apontando para `stable_executable()` + `mcp-scripts`).
5. Criar endpoints REST em `api/script_tools.rs`: `GET/POST /api/script-tools`, `PUT/DELETE /api/script-tools/{id}`, `POST /api/script-tools/{id}/test`, `GET /api/script-tools/export`, `POST /api/script-tools/import`.
6. Construir `ScriptToolsCard.tsx` + `ScriptToolModal.tsx` em `IntegrationsPage.tsx` com:
* Lista de scripts (escopo Global/Workspace, runtime, toggle ativar/desativar, botão Testar, Editar, Excluir).
* Editor com abas/seções limpas: **Script** (runtime + código), **Parâmetros** (construtor simples de campos JSON Schema), **Prompt para o Agente** (campo dedicado editável para orientar quando/como o agente deve usar a ferramenta), **Segredos & Limites** (somente `secret_ref`, timeout, exigir aprovação HITL).
* Painel inline **"Testar script"** com JSON de entrada e saída `stdout`/`stderr`/exit code em tempo real.
* Botões simétricos **Exportar JSON** e **Importar JSON**.

* **Refatoração/Simplificação/Limpeza acoplada:** Reutilizar `stable_executable()` de `api/google.rs` para que o AppImage continue funcionando com `opendesk mcp-scripts` após reinicialização.

* **Edge cases obrigatórios:**
* Se `script_body` estiver vazio ou `runtime` for inválido, retornar `400`.
* Timeout de script deve matar todo o process group (incluindo subprocessos criados pelo script).
* `env_secret_refs_json` nunca pode aceitar segredos diretos; validar com `validate_no_plaintext_secrets`.

* **Validação:**
* `cd src-tauri && cargo test script_tools` → **PASS** (incluindo teste real de `tools/list` e `tools/call` em `mcp_scripts`).
* `npm run check && npm run lint && npm run test:ui` → **PASS**.

---

### Fase 10 (`P2.3`) — Botão "Testar Tudo" (Self-Test nas Configurações) & Gate Final

* **Arquivos da fase:**
* Criar: `src-tauri/src/self_test.rs`, `src-tauri/src/api/self_test.rs`, `src-tauri/src/api/tests/self_test.rs`, `src/components/settings/SelfTestCard.tsx`.
* Modificar: `src-tauri/src/api/mod.rs`, `src-tauri/src/lib.rs`, `src/pages/SettingsPage.tsx`.

* **Passos determinísticos:**
1. Implementar `src-tauri/src/self_test.rs` com `run_self_test(state, workspace_id, mode)`:
* Gera `tag = format!("__selftest_{}__", Uuid::new_v4().simple())`.
* Roda em sequência, registrando `duration_ms`, `status` e `detail` de cada etapa:
1. **`db`:** `PRAGMA quick_check(1)` + `journal_mode == "wal"`.
2. **`engine`:** verifica status do daemon OpenCode (online/versão). Em modo `fast`, se offline marca `warn`; em modo `full_ai`, marca `fail` se offline.
3. **`task`:** cria tarefa `"{tag} task"`, valida persistência e transição; em `full_ai`, despacha execução mínima (`Responda apenas OK`) com timeout de 45s e verifica status `SUCCESS`.
4. **`routine`:** cria rotina inativa `"{tag} routine"`, valida cálculo de `next_run_at` e executa `dry_run`.
5. **`recipe`:** cria receita YAML mínima de 1 passo `"{tag}-recipe"`, valida parser + criação no banco (exercitando o fluxo da Fase 1) + `dry_run` (e execução real mínima em `full_ai`).
6. **`script_tool`:** registra temporariamente um script `echo '{"ok":true}'` (`bash`), executa via `script_tools::execute` e valida saída + exit code `0`.
7. **`voice`:** executa probe de disponibilidade dos backends STT/TTS configurados (marca `warn` se opcional não instalado, `pass` se pronto).
8. **`cleanup` (SEMPRE executado via RAII/guard mesmo em erro):** remove tarefa, rotina, receita (arquivo + banco) e script temporário criados com `{tag}`.

2. Expor `POST /api/diagnostics/self-test` em `api/self_test.rs` protegido por Bearer token e rate limit de rota sensível.
3. Criar `src/components/settings/SelfTestCard.tsx` e integrá-lo em `SettingsPage.tsx` (junto de Motor/Diagnóstico) com:
* Botão primário **"Testar tudo (rápido)"** (`mode: "fast"`).
* Botão secundário **"Testar tudo com IA"** (`mode: "full_ai"`, com tooltip avisando que consome poucos tokens).
* Checklist visual de cada etapa (`pass` verde, `warn` âmbar, `fail` vermelho), tempo individual/total e botão copiar relatório JSON.

4. Escrever teste em `api/tests/self_test.rs` validando que `mode: "fast"` passa offline e deixa **zero** registros `__selftest_*` remanescentes no banco e no disco após terminar.

* **Edge cases obrigatórios:**
* Impedir duas execuções simultâneas de `self-test` com um `AtomicBool` (retornar `409 Conflict` se já estiver rodando).
* Garantir limpeza mesmo se o cliente cancelar a requisição HTTP no meio.

* **Validação Final Completa (Gate do Repositório):**
* `python3 scripts/check-rust-limits.py --root src-tauri/crates && python3 scripts/check-rust-limits.py --root src-tauri/src && python3 scripts/check-frontend-limits.py` → **PASS** (zero arquivos > 500 linhas, zero `unwrap`/`expect` fora de testes).
* `cd src-tauri && cargo clippy --workspace --all-targets --all-features -- -D warnings` → **PASS**.
* `cd src-tauri && cargo test --workspace --all-features` → **PASS**.
* `npm run check && npm run lint && npm run test:ui && npm run build` → **PASS**.
* `./scripts/gate.sh` → **PASS**.

---

## 5. Prompt de Execução para o OpenCode

```markdown
Você vai implementar a especificação de melhorias do OpenDesk (Fases 1 a 10).

## Regras Obrigatórias de Execução
1. **Ordem estrita:** Execute EXATAMENTE uma fase por vez (Fase 1 -> Fase 2 -> ... -> Fase 10). Só avance para a próxima fase quando TODOS os comandos de validação da fase atual retornarem PASS.
2. **Leitura dirigida:** No início de cada fase, leia APENAS os arquivos listados em "Arquivos da fase" e use `rg` pontual para localizar referências/callers necessários.
3. **Limites rígidos do repositório (NÃO violar):**
   - NENHUM arquivo `.rs`, `.ts` ou `.tsx` de produção pode ultrapassar **500 linhas**. Se um arquivo crescer acima de 450 linhas, extraia um submódulo coeso imediatamente.
   - ZERO `.unwrap()` ou `.expect()` em código Rust de produção (somente dentro de `#[cfg(test)]`).
   - ZERO uso de `include!(`.
   - NUNCA adicione entradas em `scripts/rust-limits-baseline.txt` ou `scripts/shell-limits-baseline.txt`.
4. **Fail-closed e compatibilidade:**
   - Nunca silencie erros nem degrade silenciosamente em integridade do SQLite, importações JSON, restore de backup ou execução de scripts.
   - Nunca grave tokens, senhas ou chaves de API em texto plano no SQLite ou em exports JSON — use exclusivamente `secret_ref`.
   - Mantenha redirects das rotas antigas (`/worktrees` -> `/runs?tab=worktrees`, `/files` -> `/search?tab=files`) e preserve todos os testes existentes (Rust, Vitest e Playwright).
5. **Reporte ao final de cada fase:**
   - Liste arquivos criados/modificados/removidos.
   - Mostre o resultado exato dos comandos de validação.
   - Só então prossiga para a próxima fase.

Comece agora pela **Fase 1 (`P0.1`) — Diagnóstico/Reparo do SQLite `(code: 11)` e Correção da Criação de Receitas**. Leia primeiro os arquivos listados na Fase 1.
```
