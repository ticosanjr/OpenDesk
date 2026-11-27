# Plano de melhorias v3 — layout, UX, arquivos, bots multi-workspace e memória

**TL;DR:** Especificação atômica em **7 microspecs (`P0`–`P2`)** pronta para execução no OpenCode (DeepSeek v4), cobrindo: (1) layout 100% full-width/full-height com padding uniforme pequeno, headers sem layout shift entre abas, Studio mais compacto e fix de padding global nos botões; (2) fix do dropdown `/` / `@` no Chat e sistema visual único de spinner circular + ícones de estado em toda a UI; (3) consolidação em subabas (Buscar+Arquivos+Artefatos, Governança, Aprendizado e Roteador/Budget Guard) + Integrações logo abaixo de Mix de Modelos no Dashboard; (4) regra unificada de salvar/perguntar destino em todos os Importar/Exportar; (5) bots de chat independentes **por workspace** com **contexto isolado por grupo/conversa**; e (6) escopo de memória `normal | chat | both` editável na UI e controlável pelo MCP local.

### Alertas Técnicos Antes de Executar (Pontos Cegos)

* **Goldens Visuais do Playwright:** O layout atual trava conteúdo em `max-w-[1440px]` com gutter de `20–28px`. Remover o teto de largura e compactar o header vai alterar propositalmente os screenshots de `tests/visual/` — ao fim da Fase 3, é obrigatório rodar `npm run test:visual:update` e validar `scroll.spec.ts` + `a11y.spec.ts`.
* **Numeração da Migration SQL:** Verifique `ls src-tauri/crates/opendesk-storage/migrations/` antes de criar novas migrations. Use o próximo número sequencial disponível (`0002_` ou `0003_`, dependendo se `0002_script_tools.sql` já foi aplicada).
* **Limite de 500 Linhas / Zero `unwrap()`:** O gate bloqueia qualquer `.rs`, `.ts` ou `.tsx` de produção com mais de 500 linhas, `.unwrap()`/`.expect(` fora de testes ou `include!(`. Extraia submódulos sempre que um arquivo passar de 450 linhas.

---

## 1. Objetivo Final e Escopo

### Objetivo Final

Entregar um OpenDesk com aproveitamento integral de tela (horizontal e vertical), estabilidade visual absoluta entre abas, linguagem única de status animado, política determinística de salvar/abrir arquivos em todo fluxo de Import/Export, bots de chat multi-workspace com isolamento de contexto por grupo e separação rigorosa de memórias entre execução normal e chat (UI + MCP local).

### Fora de Escopo (NÃO Alterar)

* Arquitetura dos 8 crates Rust do workspace.
* Esquema de cores/tokens base de `DESIGN.md` (superfícies `#09090b`/`#0f0f12`, fontes Inter/JetBrains Mono).
* Remoção de rotas legadas sem redirect (`/artifacts`, `/files`, `/worktrees` devem sempre redirecionar preservando query params).
* Persistência de tokens de bot ou chaves em texto plano no SQLite (sempre `secret_ref` via `SecretStore`).

---

## 2. Mapa de Arquivos

| Arquivo | Ação | Motivo |
| --- | --- | --- |
| `src/components/layout/PageHeader.tsx`, `src/styles.css`, `DESIGN.md` | Modificar | Remover `max-w-[1440px]`, padronizar padding horizontal pequeno (`px-3 sm:px-4`) e altura estável do cabeçalho entre abas |
| `src/components/ui/button.tsx`, `src/pages/chat/MetaBuilder.tsx` | Modificar | Garantir padding mínimo em todas as variantes de botão e corrigir botão "Criar arquivo" no Criador |
| `src/components/ui/slash-textarea.tsx` | Modificar | Abrir dropdown de `/` e `@` para cima quando próximo da base da tela, com `max-h-60 overflow-y-auto` |
| `src/components/ui/working-indicator.tsx`, `src/components/ui/run-status.tsx`, `src/components/ui/workflow-status.tsx` | Modificar | Trocar rodinha dentada (`Cog`) por spinner circular (`Loader2 animate-spin`) e padronizar ícones de espera/falha/sucesso |
| `src/pages/DashboardPage.tsx`, `src/pages/dashboard/RunningRunsCard.tsx`, `src/components/dashboard/ActivityFeed.tsx`, `src/components/kanban/TaskCard.tsx`, `src/components/queue/QueueController.tsx`, `src/components/calendar/calendar-item.tsx` | Modificar | Subir `IntegrationFootprint` para logo abaixo de `CostMixCard`; aplicar ícones de execução/fila/falha em todos os cards e listas |
| `src/pages/StudioPage.tsx` | Modificar | Compactar verticalmente o cabeçalho do OpenCode Studio |
| `src/pages/SearchPage.tsx`, `src/pages/FilesPage.tsx`, `src/pages/ArtifactsPage.tsx`, `src/components/layout/Sidebar.tsx`, `src/components/layout/MobileNav.tsx`, `src/components/layout/CommandPalette.tsx`, `src/App.tsx`, `src/lib/links.ts` | Modificar | Unificar `Buscar` \| `Arquivos` \| `Artefatos`, fazer `Arquivos` ocupar 100% da altura vertical e redirecionar `/artifacts` |
| `src/pages/GovernancePage.tsx`, `src/pages/LearningPage.tsx`, `src/pages/RouterPage.tsx` | Modificar | Organizar conteúdo em subabas + full-width horizontal |
| `src/pages/CalendarPage.tsx`, `src/pages/RunsPage.tsx`, `src/pages/ChatPage.tsx`, `src/pages/PacksPage.tsx`, `src/pages/ObservabilityPage.tsx`, `src/pages/IntegrationsPage.tsx`, `src/pages/SettingsPage.tsx` | Modificar | Aplicar container full-width/full-height com padding horizontal uniforme |
| `src/lib/file-preferences.ts`, `src/lib/save-file.ts`, `src/lib/download.ts`, `src/components/settings/FilePreferencesCard.tsx` | Modificar | Adicionar `importExportAsk` (default `true`) e rotear todo Import/Export pela política de salvar arquivo |
| `src-tauri/crates/opendesk-storage/migrations/000N_channel_bots_and_memory_scope.sql` | Criar | Criar tabelas `channel_bots`, `channel_chat_contexts` e coluna `usage_scope` nas memórias e contexto pessoal |
| `src-tauri/crates/opendesk-storage/src/repos/channel_bots.rs` | Criar | Repositório CRUD de bots por workspace e contextos por grupo/conversa |
| `src-tauri/src/integrations/bot_supervisor.rs`, `src-tauri/src/integrations/telegram.rs`, `src-tauri/src/integrations/signal.rs`, `src-tauri/src/api/channel_bots.rs` | Criar / Modificar | Supervisor multi-bot por workspace e isolamento de sessão/contexto por `(bot_id, external_chat_id)` |
| `src/components/settings/TelegramCard.tsx`, `src/components/settings/SignalCard.tsx`, `src/components/integrations/ChannelsCard.tsx`, `src/components/integrations/ChannelBotContextsModal.tsx` | Criar / Modificar | UI de gestão de bots por workspace + contextos por grupo/conversa |
| `src-tauri/crates/opendesk-storage/src/repos/memory/`, `src-tauri/crates/opendesk-engine/src/personal_context.rs`, `src-tauri/crates/opendesk-engine/src/dsh_router/context_packer.rs`, `src-tauri/src/chat.rs`, `src-tauri/src/api/prompt_preview.rs` | Modificar | Filtrar memórias por `usage_scope` (`normal` vs `chat` vs `both`) conforme origem da execução |
| `src-tauri/src/mcp_control/tools.rs`, `src-tauri/src/mcp_control/payload.rs`, `src/components/learning/PersonalContextCard.tsx`, `src/components/learning/StructuredMemoryCard.tsx` | Modificar | Expor edição/filtro de `usage_scope` na UI e ferramentas de leitura/alteração de escopo no MCP local `opendesk-control` |

---

## 3. Contratos, Schemas e Invariantes

### 3.1 Contrato de Layout e Header Estável (`P0.1`)

* **Container Universal (`PageBody` e `PageBodyFill`):**
  * Remover `max-w-[1440px] mx-auto` de `PageBody`.
  * Aplicar padding horizontal único e estreito em todas as páginas: `px-3 sm:px-4` (`12px` mobile / `16px` desktop), `w-full min-w-0`.
  * `PageBodyFill` (usado por Kanban, Studio, Chat/Criador, Buscar/Arquivos/Artefatos, Calendário): `flex flex-1 flex-col w-full h-full min-h-0 overflow-hidden px-3 sm:px-4 pb-3`.

* **Header Sem Layout Shift (`PageHeader`):**
  * Altura estrutural fixa entre abas: `min-h-[68px]` (compacto no Studio: `min-h-[52px] py-2`).
  * Subtítulo obrigatoriamente de linha única (`truncate` com `title={description}`), nunca quebrando em 2 linhas numa aba e 1 linha em outra.
  * Quando a página possuir `SectionTabs`, renderizar o `PageHeader` **fora** do conteúdo condicional de cada aba (apenas o painel inferior troca).

### 3.2 Contrato Visual de Status Animado (`P0.2`)

Fonte única de verdade (`run-status.tsx`, `workflow-status.tsx`, `working-indicator.tsx`):

| Estado Canônico | Ícone (`lucide-react`) | Animação / Cor | Uso Obrigatório |
| --- | --- | --- | --- |
| `RUNNING` / `EXECUTING` | `Loader2` | `animate-spin text-blue-400` | Execuções (múltiplas simultâneas), Dashboard, Fila, Kanban, Calendário, Chat |
| `QUEUED` / `PENDING` / `WAITING` | `Clock3` | estático `text-amber-300` | Fila, tarefas aguardando worker, itens pendentes no Dashboard |
| `WAITING_HITL` / `NEEDS_APPROVAL` | `MessageCircleQuestion` / `ShieldAlert` | estático `text-amber-400` | Aprovações e perguntas HITL |
| `BLOCKED` / `HELD` / `PAUSED` | `PauseCircle` | estático `text-orange-400` | Tarefas retidas ou fila pausada |
| `SUCCESS` / `DONE` | `CheckCircle2` | estático `text-emerald-400` | Concluído com sucesso |
| `FAILED` / `ERROR` | `XCircle` | estático `text-red-400` | Falha de run, rotina ou receita |
| `CANCELLED` | `Ban` | estático `text-zinc-500` | Cancelado |

* **Invariante:** Remover qualquer referência a `Cog` girando (`.opendesk-gear` / `.opendesk-gear-reverse`). Em `prefers-reduced-motion: reduce`, manter o `Loader2` com rotação lenta (`animation-duration: 3s`), nunca invisível.

### 3.3 Contrato de Preferências de Import/Export (`P1.2`)

```typescript
export interface FilePreferences {
  downloadDir: string;
  downloadAsk: boolean;
  importExportAsk: boolean; // default: true (sempre perguntar onde salvar em importar/exportar)
  htmlAllowScripts: boolean;
}

export async function saveExportedData(options: {
  filename: string;
  content: string | Blob;
  mime: string;
  filters?: Array<{ name: string; extensions: string[] }>;
}): Promise<{ saved: boolean; path?: string }>;

export async function pickImportFile(options: {
  title: string;
  extensions: string[];
}): Promise<{ name: string; text: string } | null>;
```

* **Invariante:** Se `importExportAsk === true` (default) ou `downloadDir` estiver vazio, abrir o diálogo nativo `save()` em Tauri; se `importExportAsk === false` e `downloadDir` existir, salvar diretamente em `joinPath(downloadDir, filename)` via endpoint backend allowlisted; em modo browser/PWA, acionar download nativo do navegador.

### 3.4 Schema SQL — Multi-Bot por Workspace, Contextos por Grupo e Escopo de Memória (`P1.3` + `P2.1`)

```sql
-- 1. Bots de chat configurados por workspace (ex.: 1 bot Telegram/Signal/WhatsApp por workspace)
CREATE TABLE IF NOT EXISTS channel_bots (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  channel TEXT NOT NULL CHECK(channel IN ('telegram', 'signal', 'whatsapp', 'matrix')),
  name TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 0,
  secret_ref TEXT,                            -- NUNCA token em texto plano
  config_json TEXT NOT NULL DEFAULT '{}',     -- account, cli_path, phone_number_id, etc.
  allowlist_json TEXT NOT NULL DEFAULT '[]',  -- chat_ids / números autorizados
  default_agent TEXT NOT NULL DEFAULT 'build',
  status TEXT NOT NULL DEFAULT 'stopped',     -- 'stopped' | 'running' | 'error'
  last_error TEXT,
  last_checked_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(workspace_id, channel, name)
) STRICT;
CREATE INDEX IF NOT EXISTS idx_channel_bots_ws ON channel_bots(workspace_id, channel);

-- 2. Contexto e sessão isolados por conversa/grupo dentro do mesmo bot
CREATE TABLE IF NOT EXISTS channel_chat_contexts (
  id TEXT PRIMARY KEY,
  bot_id TEXT NOT NULL REFERENCES channel_bots(id) ON DELETE CASCADE,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  channel TEXT NOT NULL,
  external_chat_id TEXT NOT NULL,             -- ex.: "-1001234567890" (grupo Telegram) ou DM id
  chat_type TEXT NOT NULL DEFAULT 'group',    -- 'private' | 'group' | 'supergroup' | 'channel'
  title TEXT NOT NULL,                        -- ex.: "Trabalho", "Hobbies", "Religião"
  system_context TEXT NOT NULL DEFAULT '',    -- instruções/contexto exclusivos desta conversa
  agent_override TEXT,                        -- agente opcional só para este grupo
  memory_scope TEXT NOT NULL DEFAULT 'chat' CHECK(memory_scope IN ('chat', 'normal', 'both', 'none')),
  opencode_session_id TEXT,                   -- sessão OpenCode exclusiva deste grupo
  enabled INTEGER NOT NULL DEFAULT 1,
  last_message_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(bot_id, external_chat_id)
) STRICT;
CREATE INDEX IF NOT EXISTS idx_channel_contexts_bot ON channel_chat_contexts(bot_id, enabled);

-- 3. Distinção de escopo de memória (execução normal vs chat)
-- Verificar nomes exatos das tabelas em 0001_v2_baseline.sql antes de aplicar:
ALTER TABLE personal_context ADD COLUMN usage_scope TEXT NOT NULL DEFAULT 'both'
  CHECK(usage_scope IN ('normal', 'chat', 'both'));

ALTER TABLE memory_notes ADD COLUMN usage_scope TEXT NOT NULL DEFAULT 'both'
  CHECK(usage_scope IN ('normal', 'chat', 'both'));
```

* **Invariante de Isolamento:**
  * Chave de sessão/thread para mensagens recebidas de bots: `(bot_id, external_chat_id)`. Mensagens do Grupo A jamais leem ou escrevem na `opencode_session_id` ou no `ChatThread` do Grupo B, mesmo compartilhando o mesmo `bot_id` e a mesma `workspace_id`.
  * Execuções `Normal` (Kanban, Rotinas, Receitas) injetam **somente** `personal_context` e `memory_notes` com `usage_scope IN ('normal', 'both')`.
  * Execuções `Chat` (Chat UI, Telegram, Signal, WhatsApp) injetam **somente** `usage_scope IN ('chat', 'both')` (ou `none` se o `channel_chat_contexts.memory_scope` for `'none'`), somado ao `channel_chat_contexts.system_context` do grupo específico.

### 3.5 Contrato MCP Local para Escopo de Memória (`P2.1`)

Adicionar/estender em `opendesk-control` (`src-tauri/src/mcp_control/tools.rs`):

* `memory_list` / `memory_search`: parâmetro opcional `usage_scope?: "normal" | "chat" | "both"`.
* `memory_create` / `memory_update`: parâmetro `usage_scope?: "normal" | "chat" | "both"` (rejeita qualquer outro valor com erro JSON-RPC `-32602`).
* `memory_set_scope`: `{ id: string, usage_scope: "normal" | "chat" | "both" }`.
* `personal_context_list` / `personal_context_create` / `personal_context_update` / `personal_context_set_scope`: mesmas regras de validação fail-closed para `usage_scope`.

---

## 4. Fases Atômicas de Implementação (Microspecs)

### Fase 1 (`P0.1`) — Layout Full-Width/Full-Height Uniforme, Headers Estáveis, Studio Compacto, Dashboard e Botões

* **Arquivos da fase:**
  * Ler e modificar: `src/components/layout/PageHeader.tsx`, `src/styles.css`, `src/components/ui/button.tsx`, `src/pages/chat/MetaBuilder.tsx`, `src/pages/StudioPage.tsx`, `src/pages/DashboardPage.tsx`, `src/pages/CalendarPage.tsx`, `src/pages/RunsPage.tsx`, `src/pages/ChatPage.tsx`, `src/pages/PacksPage.tsx`, `src/pages/ObservabilityPage.tsx`, `src/pages/IntegrationsPage.tsx`, `src/pages/SettingsPage.tsx`, `DESIGN.md`.

* **Passos determinísticos:**
1. Em `src/components/layout/PageHeader.tsx`:
   * Remover `max-w-[1440px] mx-auto` de `PageBody`; aplicar `w-full min-w-0 px-3 sm:px-4 py-3` em `PageBody` e `flex flex-1 flex-col w-full h-full min-h-0 px-3 sm:px-4 pb-3` em `PageBodyFill`.
   * Padronizar o padding horizontal de `PageHeader` para o mesmo `px-3 sm:px-4`.
   * Adicionar prop `compact?: boolean` em `PageHeader` (normal: `min-h-[64px] py-2.5`; compact: `min-h-[48px] py-1.5`) e garantir que a linha de descrição use `truncate` (altura fixa de 1 linha) para eliminar layout shift entre abas.
2. Em `src/pages/StudioPage.tsx`, passar `compact` ao `PageHeader` e enxugar espaçamentos excessivos da barra de abas/escopo (`py-1.5` / `gap-2`) para maximizar a área útil dos editores.
3. Em `src/pages/DashboardPage.tsx`, mover `<IntegrationFootprint .../>` para renderizar **imediatamente abaixo** de `<CostMixCard .../>` (antes das seções inferiores).
4. Em `src/pages/ChatPage.tsx` e `src/pages/chat/MetaBuilder.tsx`:
   * Garantir que a aba `Chat` ocupe `w-full h-full min-h-0` (sem `max-w-*` limitando o painel do transcript/composer).
   * Garantir que a aba `Criador` (`MetaBuilder`) use `flex flex-1 flex-col w-full h-full min-h-0`, expandindo o formulário e o preview para ocupar tanto a largura quanto a altura disponíveis.
5. Auditar `src/components/ui/button.tsx` e todos os botões com `rg "<Button" src/`:
   * Garantir `inline-flex items-center justify-center gap-1.5 whitespace-nowrap shrink-0` e padding horizontal mínimo explícito em todas as variantes (`xs: h-7 px-2.5`, `sm: h-8 px-3`, `md: h-9 px-3.5`, `lg: h-10 px-4`, `icon: h-8 w-8 p-0`).
   * Corrigir o botão **"Criar arquivo"** em `MetaBuilder.tsx` (e qualquer outro botão com `px-0`/`w-*` apertado encontrado na auditoria) removendo overrides que cortavam ícone/texto.
6. Auditar e remover `max-w-*` restritivos em `CalendarPage`, `RunsPage`, `PacksPage`, `ObservabilityPage`, `IntegrationsPage` e `SettingsPage` para que todas ocupem 100% do espaço horizontal respeitando o mesmo padding `px-3 sm:px-4`.
7. Atualizar a tabela de Layout em `DESIGN.md` refletindo o layout full-width com gutter uniforme `12px / 16px`.

* **Refatoração/Simplificação/Limpeza acoplada:** Padronizar containers de Settings/Integrations com grid responsivo (`grid-cols-[200px_minmax(0,1fr)]`) onde o painel da direita se estende até a borda direita sem limite artificial.
* **Edge cases obrigatórios:**
  * Nenhum overflow horizontal em `1024×768` (validar com `tests/e2e/scroll.spec.ts` e `tests/e2e/ui-audit.spec.ts`).
  * Botões `icon`-only continuam quadrados (`w-8 h-8`) com `aria-label` e `Tooltip`.

* **Validação:**
  * `npm run check && npm run lint && npm run test:ui` → **PASS**.
  * `python3 scripts/check-frontend-limits.py` → **PASS**.

---

### Fase 2 (`P0.2`) — Fix do Dropdown `/` / `@` no Chat + Sistema Visual Unificado de Status (Spinner Circular)

* **Arquivos da fase:**
  * Ler e modificar: `src/components/ui/slash-textarea.tsx`, `src/components/ui/slash-textarea.test.tsx`, `src/components/ui/working-indicator.tsx`, `src/components/ui/run-status.tsx`, `src/components/ui/run-status.test.tsx`, `src/components/ui/workflow-status.tsx`, `src/styles.css`, `src/pages/dashboard/RunningRunsCard.tsx`, `src/components/dashboard/ActivityFeed.tsx`, `src/components/kanban/TaskCard.tsx`, `src/components/queue/QueueController.tsx`, `src/pages/RunsPage.tsx`, `src/components/calendar/calendar-item.tsx`.

* **Passos determinísticos:**
1. **Corrigir `src/components/ui/slash-textarea.tsx`:**
   * Calcular espaço disponível abaixo vs acima via `containerRef.current.getBoundingClientRect()` ao abrir o menu de sugestões.
   * Se `window.innerHeight - rect.bottom < 260` (caso padrão no composer do rodapé do chat), renderizar o dropdown **para cima** (`bottom-full mb-1.5`) usando Portal (`createPortal` em `document.body` com coordenadas `fixed` e `z-50`) para que nenhum `overflow-hidden` pai corte a lista.
   * Aplicar `max-h-60 overflow-y-auto overscroll-contain` no dropdown e chamar `activeItemRef.scrollIntoView({ block: "nearest" })` ao navegar com setas `ArrowUp`/`ArrowDown`.
   * Adicionar teste em `slash-textarea.test.tsx` garantindo que o menu posiciona para cima quando perto da borda inferior e permanece rolável.
2. **Substituir Engrenagem pelo Spinner Circular (`working-indicator.tsx` & `styles.css`):**
   * Em `working-indicator.tsx`, trocar `Cog` por `Loader2` (`className="animate-spin text-blue-400 shrink-0"`).
   * Remover `.opendesk-gear`, `.opendesk-gear-reverse` e `@keyframes opendesk-gear-spin` de `src/styles.css`.
3. **Unificar Ícones de Estado (`run-status.tsx` e `workflow-status.tsx`):**
   * Garantir que `runStatusMeta` e `WorkflowStatusBadge` implementem exatamente a tabela da Seção 3.2: `RUNNING` = `Loader2 animate-spin` azul; `QUEUED`/`PENDING` = `Clock3` âmbar; `FAILED` = `XCircle` vermelho; `SUCCESS`/`DONE` = `CheckCircle2` verde; `BLOCKED`/`HELD`/`PAUSED` = `PauseCircle` laranja.
4. **Aplicar em todos os locais de execução:**
   * `RunsPage.tsx`: exibir `RunStatusIcon` animado em **todas** as linhas/cards em `RUNNING` simultaneamente (e `Clock3` / `XCircle` / `CheckCircle2` nos demais).
   * `DashboardPage` (`RunningRunsCard.tsx`, `ActivityFeed.tsx`): toda tarefa/run ativa ou listada exibe o ícone de estado correto (`Loader2` girando quando rodando, `Clock3` quando na fila, `XCircle` quando falhou).
   * `QueueController.tsx`, `TaskCard.tsx`, `calendar-item.tsx`: remover ícones hardcoded antigos e consumir `RunStatusIcon` / `WorkflowStatusBadge`.

* **Refatoração/Simplificação/Limpeza acoplada:** Checar com `rg "opendesk-gear|<Cog" src/` para garantir zero restos da rodinha dentada.
* **Edge cases obrigatórios:**
  * Clicar num item do dropdown no portal (`onMouseDown` com `preventDefault()`) não pode disparar blur prematuro e fechar antes de inserir o comando.
  * Em `prefers-reduced-motion: reduce`, `animate-spin` continua visível em velocidade reduzida.

* **Validação:**
  * `rg "opendesk-gear" src/` → **PASS** (saída vazia).
  * `npm run check && npm run lint && npm run test:ui` → **PASS**.

---

### Fase 3 (`P1.1`) — Subabas Estruturais (Buscar/Arquivos/Artefatos, Governança, Aprendizado, Roteador) + Altura Vertical em Arquivos

* **Arquivos da fase:**
  * Ler e modificar: `src/App.tsx`, `src/lib/links.ts`, `src/components/layout/Sidebar.tsx`, `src/components/layout/MobileNav.tsx`, `src/components/layout/CommandPalette.tsx`, `src/pages/SearchPage.tsx`, `src/pages/FilesPage.tsx`, `src/pages/ArtifactsPage.tsx`, `src/pages/GovernancePage.tsx`, `src/pages/LearningPage.tsx`, `src/pages/RouterPage.tsx`.

* **Passos determinísticos:**
1. **Buscar + Arquivos + Artefatos (`SearchPage.tsx`):**
   * Extrair o corpo de `ArtifactsPage.tsx` como `ArtifactsTabContent` (sem `PageHeader` duplicado).
   * Em `SearchPage.tsx`, usar um **único `PageHeader` estável** acima de `SectionTabs` com 3 abas: `Buscar` (`search`, padrão), `Arquivos` (`files`) e `Artefatos` (`artifacts`), sincronizadas em `?tab=...`.
   * Em `FilesTabContent` (`FilesPage.tsx`), aplicar `flex flex-1 min-h-0 h-full` no grid de árvore + preview (`FileListCard` e `FilePreviewCard`) para ocupar **100% da altura vertical** da viewport com scroll interno independente em cada painel.
   * Remover `Artefatos` (e confirmar remoção de `Arquivos`/`Worktrees`) da `Sidebar.tsx` e `MobileNav.tsx`.
   * Em `App.tsx`, redirecionar `/artifacts` → `/search?tab=artifacts` preservando query params (`?id=...`), e atualizar `links.ts` e `CommandPalette.tsx`.
2. **Agentes & Governança (`GovernancePage.tsx`):**
   * Manter `PageHeader` estável + KPI strip compacto no topo e dividir o corpo em `SectionTabs` (`?tab=...`): `Agentes` (`agents` — OrgChart), `Metas` (`goals`), `Delegações` (`delegations`), `Handoffs` (`handoffs`) e `Performance` (`performance`).
   * Remover `max-w-*` para uso total da largura.
3. **Aprendizado (`LearningPage.tsx`):**
   * Manter `PageHeader` estável e dividir em `SectionTabs` (`?tab=...`): `Lições & Loop` (`lessons`), `Contexto Pessoal` (`context`), `Memória Estruturada` (`memory`) e `Avaliação de Skills` (`eval` — `SkillEvalCard` + `LearningProposalsSection`).
4. **Roteador & Custos (`RouterPage.tsx`):**
   * Manter `PageHeader` estável e dividir em `SectionTabs` (`?tab=...`): `Políticas & Modelos` (`models`), `Regras` (`rules`), `Budget Guard` (`budget`) e `Análise de Custos` (`analysis`).

* **Refatoração/Simplificação/Limpeza acoplada:** Verificar que nenhuma página ultrapassa 500 linhas após a reorganização em abas; se passar, extrair componentes de aba em subpastas (`src/pages/governance/`, `src/pages/router/`, etc.).
* **Edge cases obrigatórios:**
  * Alternar entre qualquer par de abas (`Buscar` ↔ `Arquivos` ↔ `Artefatos`, etc.) não pode deslocar a posição vertical dos botões ou abas em mais de `0px`.
  * Deep links existentes (`/artifacts?id=abc`, `/files?path=x.md`) devem abrir exatamente o item alvo na aba correta.

* **Validação:**
  * `npm run check && npm run lint && npm run test:ui` → **PASS**.
  * `python3 scripts/check-frontend-limits.py` → **PASS**.
  * Rodar `npx playwright test tests/e2e/scroll.spec.ts tests/e2e/files-search-ui.spec.ts tests/e2e/artifacts-ui.spec.ts tests/e2e/governance-ui.spec.ts tests/e2e/learning-ui.spec.ts tests/e2e/router-ui.spec.ts` (e atualizar seletores de navegação nos specs se necessário, sem remover asserts funcionais) → **PASS**.

---

### Fase 4 (`P1.2`) — Política Unificada de Salvar/Abrir Arquivo em Todos os Importar e Exportar

* **Arquivos da fase:**
  * Ler e modificar: `src/lib/file-preferences.ts`, `src/lib/file-preferences.test.ts`, `src/lib/save-file.ts`, `src/lib/save-file.test.ts`, `src/lib/download.ts`, `src/lib/download.test.ts`, `src/components/settings/FilePreferencesCard.tsx`, `src-tauri/src/api/files.rs` (verificar endpoint de gravação de arquivo salvo pelo usuário).
  * Criar: `src/lib/import-export.ts`, `src/lib/import-export.test.ts`.
  * Auditar e atualizar callers: `PersonalContextCard.tsx`, `StructuredMemoryCard.tsx`, `PromptTemplatesCard.tsx`, `BackupsCard.tsx`, `PacksPageContent.tsx`, `WorkspacesCard.tsx`, `RunLogHeader.tsx`, `ScriptToolsCard.tsx` (se existir).

* **Passos determinísticos:**
1. Em `src/lib/file-preferences.ts`, adicionar `importExportAsk: boolean` com **valor padrão `true`**, persistido na setting `files.import_export_ask` (mesmo padrão de `files.download_ask` e `files.download_dir`).
2. Em `src/components/settings/FilePreferencesCard.tsx`, adicionar o switch **"Perguntar sempre onde salvar ao importar/exportar"** (ligado por padrão) com texto claro explicando que, quando desligado, usa automaticamente a pasta de downloads configurada.
3. Verificar/adicionar no backend (`src-tauri/src/api/files.rs`) suporte em `POST /api/files/save-bytes` (ou estender o endpoint usado por `saveLocalFile`) para gravar texto/bytes exportados num caminho escolhido pelo usuário via diálogo nativo ou na `downloadDir` configurada (rejeitando paths fora de `$HOME` / `$TEMP` se inválido, fail-closed).
4. Criar `src/lib/import-export.ts` implementando `saveExportedData` e `pickImportFile` conforme Seção 3.3:
   * Em Tauri: se `prefs.importExportAsk || !prefs.downloadDir`, abre `save({ defaultPath, filters })` do `@tauri-apps/plugin-dialog` e grava no caminho escolhido (retorna `{ saved: false }` se usuário cancelar); caso contrário grava automaticamente em `joinPath(prefs.downloadDir, filename)` e exibe toast com caminho salvo.
   * Em browser/PWA: usa `downloadBlob`.
   * Para importar: em Tauri usa `open({ filters })` + leitura segura (ou `<input type="file">` quando em browser/PWA).
5. Atualizar `downloadRun` (`src/lib/download.ts`) e **todos** os botões de Exportar/Importar encontrados com `rg -i "export|import|downloadBlob" src/` para passarem exclusivamente por `saveExportedData` / `pickImportFile`.

* **Refatoração/Simplificação/Limpeza acoplada:** Remover chamadas soltas a `document.createElement("a")` / `URL.createObjectURL` espalhadas pelos componentes de exportação.
* **Edge cases obrigatórios:**
  * Cancelar o diálogo nativo de salvar não pode exibir toast de erro nem toast falso de sucesso.
  * Se `downloadDir` configurado não existir mais no disco e `importExportAsk === false`, fazer fallback abrindo o diálogo de salvar e avisando o usuário (fail-safe explícito, sem perda de dados).

* **Validação:**
  * `cd src-tauri && cargo test files_save` → **PASS**.
  * `npm run check && npm run lint && npm run test:ui` → **PASS**.

---

### Fase 5 (`P1.3`) — Backend Multi-Bot por Workspace + Contexto Isolado por Conversa/Grupo

* **Arquivos da fase:**
  * Criar:
    * `src-tauri/crates/opendesk-storage/migrations/000N_channel_bots_and_memory_scope.sql` (usar próximo número sequencial real em `migrations/`)
    * `src-tauri/crates/opendesk-storage/src/repos/channel_bots.rs`
    * `src-tauri/src/integrations/bot_supervisor.rs`
    * `src-tauri/src/api/channel_bots.rs`
    * `src-tauri/src/api/tests/channel_bots.rs`
  * Ler e modificar:
    * `src-tauri/crates/opendesk-storage/src/repos/mod.rs`
    * `src-tauri/src/integrations/telegram.rs`, `src-tauri/src/integrations/signal.rs`
    * `src-tauri/src/api/mod.rs`, `src-tauri/src/app_state.rs`, `src-tauri/src/chat.rs`

* **Passos determinísticos:**
1. Criar a migration SQL da Seção 3.4 (`channel_bots`, `channel_chat_contexts` e já incluir os `ALTER TABLE` de `usage_scope` da Fase 7 para não proliferar migrations).
2. Implementar `repos/channel_bots.rs` com CRUD de `channel_bots` (filtrado por `workspace_id`), CRUD de `channel_chat_contexts` (filtrado por `bot_id`), e `upsert_seen_chat_context(bot_id, external_chat_id, chat_type, title)` para auto-registrar grupos/DMs quando o bot recebe a primeira mensagem.
3. Implementar `bot_supervisor.rs` (armazenado no `AppState`):
   * Mantém um `HashMap<String, BotHandle>` (`bot_id` → `CancellationToken` + `JoinHandle` + status live).
   * No startup do daemon, carrega todos os `channel_bots` com `enabled = 1` de **todas as workspaces** e inicia cada bridge isoladamente (Telegram long-polling com seu próprio `secret_ref`, Signal com sua conta/config).
   * Garante migração suave (auto-seed no primeiro boot): se existir configuração legada global de Telegram/Signal em `app_settings` e `channel_bots` estiver vazio, cria automaticamente o registro correspondente em `channel_bots` vinculado à workspace configurada, sem perder o bot atual do usuário.
   * No shutdown, cancela todos os tokens e drena tasks com timeout de 3s.
4. Atualizar roteamento de mensagens recebidas em `telegram.rs` / `signal.rs` e `chat.rs`:
   * Resolver o contexto específico em `channel_chat_contexts` pela chave única `(bot_id, external_chat_id)`.
   * Criar/buscar a thread local vinculada a `external_id = format!("{channel}:{bot_id}:{external_chat_id}")` na `workspace_id` do bot.
   * Usar `context.opencode_session_id` (isolada por grupo!), `context.agent_override.unwrap_or(bot.default_agent)` e injetar `context.system_context` no prompt do turno (junto com o `memory_scope` daquele grupo).
   * Se `context.enabled == 0`, ignorar silenciosamente a mensagem (com log debug) sem executar o agente.
5. Expor endpoints REST em `api/channel_bots.rs`:
   * `GET /api/channels/bots?workspace_id=&channel=`
   * `POST /api/channels/bots` (salva token exclusivamente via `SecretStore`, gera `secret_ref`, nunca retorna o token no JSON)
   * `PUT /api/channels/bots/{id}`, `DELETE /api/channels/bots/{id}` (para bridge e remove segredo do `SecretStore`)
   * `POST /api/channels/bots/{id}/start`, `POST /api/channels/bots/{id}/stop`, `POST /api/channels/bots/{id}/test`
   * `GET /api/channels/bots/{id}/contexts`, `POST /api/channels/bots/{id}/contexts`, `PUT /api/channels/bots/{id}/contexts/{ctx_id}`, `DELETE /api/channels/bots/{id}/contexts/{ctx_id}`
   * Manter rotas legadas `/api/telegram/*` e `/api/signal/*` operando sobre o bot padrão da workspace ativa para não quebrar testes existentes (`api/tests/telegram.rs`).
6. Escrever testes em `api/tests/channel_bots.rs` provando: (a) 2 workspaces com 2 bots Telegram independentes mantêm estados/tokens separados; (b) 2 grupos diferentes (`-1001` Trabalho e `-1002` Hobbies) no mesmo bot geram 2 threads/sessões completamente isoladas com `system_context` distinto; (c) token nunca aparece no JSON de resposta nem no SQLite.

* **Refatoração/Simplificação/Limpeza acoplada:** Remover campos globais soltos duplicados onde o `BotSupervisor` já centraliza estado, mantendo arquivos `<= 500` linhas.
* **Edge cases obrigatórios:**
  * Se 1 bot falhar por token inválido/rede, apenas o `status` daquele `bot_id` muda para `'error'` com `last_error` persistido — nunca derrubar bots de outras workspaces.
  * Impedir dois bots com o mesmo token rodando long-polling simultaneamente (detectar colisão ou documentar erro HTTP 409 do Telegram no `last_error` com backoff exponencial).

* **Validação:**
  * `cd src-tauri && cargo test channel_bots && cargo test telegram` → **PASS**.
  * `python3 scripts/check-rust-limits.py --root src-tauri/crates && python3 scripts/check-rust-limits.py --root src-tauri/src` → **PASS**.

---

### Fase 6 (`P1.4`) — UI de Bots por Workspace e Gestão de Contextos por Grupo/Conversa

* **Arquivos da fase:**
  * Criar: `src/components/integrations/ChannelBotContextsModal.tsx`, `src/components/integrations/ChannelBotsPanel.tsx`.
  * Ler e modificar: `src/components/settings/TelegramCard.tsx`, `src/components/settings/SignalCard.tsx`, `src/components/integrations/ChannelsCard.tsx`, `src/lib/types/schedules.ts`.

* **Passos determinísticos:**
1. Adicionar os tipos TypeScript `ChannelBot` e `ChannelChatContext` em `src/lib/types/schedules.ts`.
2. Criar `ChannelBotsPanel.tsx` reutilizável (recebendo `channel: "telegram" | "signal" | "whatsapp"` e `workspaceId`) exibindo:
   * Cabeçalho indicando claramente a workspace ativa (ex.: `"Bots de Telegram — Workspace <nome>"`) e resumo dos bots configurados em outras workspaces (badge de status online/erro/parado por workspace).
   * Lista de bots da workspace ativa com: nome, status ao vivo (`StatusDot` + `Loader2` quando conectando), botões `Iniciar`/`Parar`, `Testar`, `Contextos (N)`, `Editar` e `Excluir`.
   * Formulário de criação/edição (token nunca pré-preenchido em texto plano — apenas indicador `"Token configurado"` + campo opcional para substituir, allowlist, agente padrão).
3. Criar `ChannelBotContextsModal.tsx` permitindo gerenciar os contextos por conversa/grupo de um bot:
   * Listar grupos/chats já detectados automaticamente pelo bot + botão **"Adicionar grupo/chat manualmente"** (`external_chat_id`, título amigável como `"Trabalho"`, `"Hobbies"`, `"Religião"`).
   * Editor por contexto: campo `system_context` (instruções exclusivas daquele grupo), `agent_override` (opcional), seletor `memory_scope` (`Chat` \| `Normal` \| `Ambos` \| `Nenhuma`), botão `"Reiniciar sessão do grupo"` (limpa `opencode_session_id` só daquele grupo) e toggle ativar/desativar.
4. Integrar `ChannelBotsPanel` em `TelegramCard.tsx`, `SignalCard.tsx` e `ChannelsCard.tsx` preservando compatibilidade com os testes E2E existentes (`integrations-ui.spec.ts`).

* **Refatoração/Simplificação/Limpeza acoplada:** Centralizar lógica de CRUD de bots num único `ChannelBotsPanel` em vez de duplicar estado em cada card de canal.
* **Edge cases obrigatórios:**
  * Ao trocar de workspace no switcher da topbar, a lista de bots e seus status devem recarregar imediatamente para a nova workspace (sem vazar estado da anterior).

* **Validação:**
  * `npm run check && npm run lint && npm run test:ui` → **PASS**.
  * `python3 scripts/check-frontend-limits.py` → **PASS**.

---

### Fase 7 (`P2.1`) — Escopo de Memória (`normal` vs `chat` vs `both`) no Motor, UI e MCP Local + Gate Final

* **Arquivos da fase:**
  * Ler e modificar (backend):
    * `src-tauri/crates/opendesk-domain/src/lib.rs` (enum `MemoryUsageScope { Normal, Chat, Both }`)
    * `src-tauri/crates/opendesk-storage/src/repos/memory/` (queries + CRUD com `usage_scope`)
    * `src-tauri/crates/opendesk-engine/src/personal_context.rs`
    * `src-tauri/crates/opendesk-engine/src/dsh_router/context_packer.rs`
    * `src-tauri/src/chat.rs`, `src-tauri/src/api/personal_context.rs`, `src-tauri/src/api/memory.rs`, `src-tauri/src/api/prompt_preview.rs`
    * `src-tauri/src/mcp_control/tools.rs`, `src-tauri/src/mcp_control/payload.rs`
    * `src-tauri/src/api/tests/memory.rs`, `src-tauri/src/api/tests/phase2.rs`
  * Ler e modificar (frontend):
    * `src/lib/types/learning.ts`
    * `src/components/learning/PersonalContextCard.tsx`
    * `src/components/learning/StructuredMemoryCard.tsx`

* **Passos determinísticos:**
1. Definir `MemoryUsageScope` (`"normal" | "chat" | "both"`) em `opendesk-domain` com parsing estrito (`FromStr` fail-closed retornando `DomainError::Validation` para qualquer valor inválido).
2. Atualizar os repositórios de `personal_context` e `memory_notes` para persistir, filtrar (`filter_for_normal` → `IN ('normal','both')`; `filter_for_chat` → `IN ('chat','both')`) e retornar `usage_scope`.
3. Atualizar montagem de prompts:
   * Em `context_packer.rs` (execuções normais: Kanban, Rotinas, Receitas) e `api/prompt_preview.rs` (preview de tarefa): carregar **somente** `usage_scope IN ('normal', 'both')`.
   * Em `chat.rs` (Chat UI e bots de chat) e `api/prompt_preview.rs` (preview de chat): carregar **somente** `usage_scope IN ('chat', 'both')` (ou nenhuma memória se `channel_chat_contexts.memory_scope == 'none'`).
4. Atualizar API HTTP:
   * `GET/POST/PUT /api/personal-context` e `GET/POST/PUT /api/memory/notes`: aceitar/retornar `usage_scope` (default `'normal'` quando criado pela UI/tarefa sem campo explícito, `'chat'` quando criado a partir de fluxo de chat, preservando `'both'` para registros legados).
   * Incluir `usage_scope` nos payloads de Export/Import JSON (Fase 4).
5. Atualizar **MCP Local (`src-tauri/src/mcp_control/tools.rs` e `payload.rs`)**:
   * Expor/atualizar as ferramentas `memory_list`, `memory_create`, `memory_update`, `memory_set_scope`, `personal_context_list`, `personal_context_create`, `personal_context_update`, `personal_context_set_scope` conforme Seção 3.5.
   * Garantir que a descrição de cada tool documente os valores exatos `"normal" | "chat" | "both"`.
6. Atualizar `PersonalContextCard.tsx` e `StructuredMemoryCard.tsx`:
   * Adicionar barra de filtro rápido (`Todos` \| `Normal` \| `Chat` \| `Ambos`).
   * Em cada item/regra de memória, exibir seletor inline ou badge clicável para alternar `Normal` / `Chat` / `Ambos` com salvamento imediato + feedback visual.
7. Escrever testes automatizados em `api/tests/memory.rs`, `api/tests/phase2.rs` e `mcp_control/payload.rs` provando:
   * Memória marcada `chat` **não aparece** no prompt de tarefa normal (`context_packer` / `prompt_preview`).
   * Memória marcada `normal` **não aparece** no prompt de chat (`chat_prompt_preview`).
   * Memória marcada `both` aparece em ambos.
   * Chamada MCP `memory_set_scope` / `personal_context_set_scope` altera o escopo e rejeita valores inválidos (`-32602`).

* **Edge cases obrigatórios:**
  * Registros antigos migrados recebem `usage_scope = 'both'` na migration para não quebrar workspaces existentes nem testes antigos.
  * Se o usuário tentar importar JSON legado sem `usage_scope`, assumir `'both'` de forma explícita.

* **Validação Final Completa (Gate):**
  * `python3 scripts/check-rust-limits.py --root src-tauri/crates && python3 scripts/check-rust-limits.py --root src-tauri/src && python3 scripts/check-frontend-limits.py` → **PASS**.
  * `cd src-tauri && cargo clippy --workspace --all-targets --all-features -- -D warnings` → **PASS**.
  * `cd src-tauri && cargo test --workspace --all-features` → **PASS**.
  * `npm run check && npm run lint && npm run test:ui && npm run build` → **PASS**.
  * Atualizar goldens visuais apenas após conferir visualmente o novo layout full-width (`npm run test:visual:update` se necessário) e rodar `./scripts/gate.sh` → **PASS**.

---

## 5. Prompt de Execução para o OpenCode

```markdown
Você vai implementar a especificação de melhorias de layout, UX, arquivos, bots multi-workspace e escopo de memória do OpenDesk (Fases 1 a 7).

## Regras Obrigatórias de Execução
1. **Ordem estrita:** Execute EXATAMENTE uma fase por vez (Fase 1 -> Fase 2 -> ... -> Fase 7). Só avance para a próxima fase quando TODOS os comandos de validação da fase atual retornarem PASS.
2. **Leitura dirigida:** No início de cada fase, leia APENAS os arquivos listados em "Arquivos da fase" e use `rg` pontual para localizar callers/imports/tabelas exatas (especialmente `0001_v2_baseline.sql` antes da Fase 5).
3. **Limites rígidos do repositório (NÃO violar):**
   - NENHUM arquivo `.rs`, `.ts` ou `.tsx` de produção pode ultrapassar **500 linhas**. Se crescer acima de 450 linhas, extraia um submódulo limpo imediatamente.
   - ZERO `.unwrap()` ou `.expect()` em código Rust de produção (somente em `#[cfg(test)]`).
   - ZERO uso de `include!(`.
   - NUNCA adicione entradas em `scripts/rust-limits-baseline.txt` ou `scripts/shell-limits-baseline.txt`.
4. **Fail-closed e preservação de contratos:**
   - Nunca silencie erros nem salve tokens/segredos de bots em texto plano no SQLite (use exclusivamente `SecretStore` + `secret_ref`).
   - Garanta 0px de layout shift no `PageHeader` ao alternar entre abas e zero overflow horizontal em 1024x768.
   - Preserve redirects de rotas legadas (`/artifacts` -> `/search?tab=artifacts`, `/files` -> `/search?tab=files`, `/worktrees` -> `/runs?tab=worktrees`) e mantenha todos os testes existentes verdes (atualize seletores E2E apenas se a navegação mudou para aba, nunca remova assertions).
5. **Reporte objetivo ao fim de cada fase:**
   - Liste arquivos criados/modificados/removidos.
   - Cole o resultado exato dos comandos de validação executados.
   - Confirme conformidade com o limite de 500 linhas antes de avançar.

Comece agora pela **Fase 1 (`P0.1`) — Layout Full-Width/Full-Height Uniforme, Headers Estáveis, Studio Compacto, Dashboard e Botões**. Leia primeiro os arquivos listados na Fase 1.
```
