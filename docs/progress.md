# Histórico de progresso

## 2026-11-06 — Fundação (Fase 1)

### Entregue

- Scaffold Tauri v2 + React 19 + TypeScript + Vite 8 + Tailwind CSS 4.
- Migração SQLite completa e corrigida em relação à spec:
  ordem de foreign keys, índices, `app_settings`, `integration_deliveries`,
  `config_change_log`, `permission_audit` e colunas de fase/telemetria.
- Repositórios de workspaces e settings; seed de budget; workspace ativo em
  `app_settings`.
- API local Axum com token Bearer, CORS restrito, `/ws` e eventos tipados.
- Daemon manager OpenCode V2:
  - spawn de `opencode serve --hostname 127.0.0.1 --port 3100` (ou próxima
    porta livre);
  - captura da senha efêmera no stdout/stderr;
  - Basic auth;
  - healthcheck de 5 s;
  - restart e reattach por `daemon.json` 0600;
  - shutdown do filho no exit do app.
- Bandeja do sistema com status, fila, orçamento, abrir dashboard, reiniciar
  daemon e sair.
- Frontend:
  - shell com sidebar, topbar, workspace switcher, orçamento, fila, status do
    daemon e command bar (Cmd/Ctrl+K);
  - dashboard com KPIs, agentes reais via `/api/agent`, engine e workspace;
  - 9 views roteadas com estados de fundação para Kanban, Chat, Artefatos,
    Arquivos, Rotinas, Studio, Roteador e Integrações;
  - Zustand com snapshot REST + WebSocket com reconexão e `ResyncRequired`.
- Criação de workspace cria `input/`, `output/` e `.opencode/`; o último é
  necessário para o OpenCode reconhecer o location e retornar agentes/modelos
  scoped.

### Verificado

- `cargo check` — passou (warnings de dead code aceitos para APIs de fases
  futuras).
- `npm run check` — passou.
- `npm run build` — passou.
- App iniciado com `npm run tauri dev`.
- Daemon subiu em `127.0.0.1:3101` porque a 3100 estava ocupada pelo harness
  Paperclip; `/api/health` reportou `online: true`, versão `2.2.5`.
- `GET /api/daemon/status` sem token retornou `401`.
- Criação de workspace via API criou `input/`, `output/`; depois validei que
  `.opencode/` é necessário para location-scoped agents/models.
- `GET /api/agents` e `/api/models` retornaram dados quando o workspace tinha
  `.opencode/` (75 modelos, 7 agentes built-in no location).
- `GET /api/dashboard` retornou zeros para workspace novo.
- `GET /api/settings/active_workspace_id` persistiu o workspace ativo.

### Observações de integração

- `/api/agent`, `/api/model` e `/api/config` usam
  `?location[directory]=<path>` (deepObject). Sem um location reconhecido,
  retornam `LocationNotFoundError` ou lista vazia.
- `GET /api/config` retorna a lista de documentos/diretórios carregados, não um
  objeto único.
- A senha do daemon é efêmera e aparece no stdout como
  `server password <valor>`; deve ser removida de qualquer log.
- O harness Paperclip ocupa a porta 3100; o fallback para 3101 é essencial.

### Próximo

1. Fase 2 — Studio SSOT:
   - `jsonc_editor` com CST + escrita atômica + hash de concorrência;
   - parsers/serializers de agentes, skills, comandos e MCP V2;
   - watcher `notify` com debounce;
   - UI do Studio e testes de paridade <500 ms.
2. Adicionar testes de integração Rust para a criação de workspace e layout.
3. Revalidar o app com a criação automática de `.opencode/`.

## 2026-11-06 — Studio SSOT, settings de workspace, agentes customizáveis e releases

### Entregue

- **Studio SSOT (Fase 2)**:
  - `jsonc_editor` com CST (`jsonc-parser`) preservando comentários, escrita
    atômica, backup e hash de concorrência (`409` em edição externa).
  - CRUD de agentes, skills, comandos e servidores MCP direto nos arquivos
    nativos do OpenCode V2.
  - Watcher `notify` com debounce de 150 ms e eventos `FsSyncTriggered`; a UI
    recarrega o Studio quando um arquivo muda por fora.
  - UI do OpenCode Studio com abas Agentes, Skills, Comandos, MCP e JSONC bruto,
    escopo Workspace/Global e editor de permissões V2.
- **Agentes customizáveis**:
  - `name` (nome de exibição), `avatar` (emoji/2 caracteres) e `color` no
    frontmatter; o OpenCode ignora `name`/`avatar` e usa `color` nativamente.
  - Dashboard e lista do Studio exibem avatar, nome e cor.
- **Workspaces**:
  - Configuração de pasta padrão (`workspaces_root`) na tela Configurações.
  - Diálogo com modos **Criar novo** (pasta pai + slug do nome) e **Adicionar
    existente** (qualquer pasta do disco).
  - Folder picker nativo via `tauri-plugin-dialog`; caminho sempre editável.
  - Criação automática de `input/`, `output/` e `.opencode/`.
- **Modo headless**:
  - `opendesk --headless` sobe a mesma API Axum/SQLite/watcher sem Tauri, com
    `OPENDESK_API_PORT`, `OPENDESK_DATA_DIR`, `OPENDESK_SKIP_DAEMON` e
    `OPENDESK_CONFIG_DIR` para testes isolados.
- **Testes**:
  - 12 testes Rust (workspaces, JSONC/comentários, agentes, skills, comandos,
    MCP e watcher).
  - 2 testes Playwright end-to-end cobrindo criação de workspace, configuração
    da pasta padrão, agente customizado e sincronização de edição externa.
  - Correção de um bug real encontrado pelo e2e: o botão “Novo workspace” estava
    coberto pelo `<select>` do workspace switcher por falta de `shrink-0`.
- **Releases locais**:
  - `releases/` com no máximo 3 versões, `manifest.json`, `SHA256SUMS` e
    `LATEST`; binários ignorados pelo git.
  - `scripts/release.sh` compila, empacota, poda versões antigas, cria commit/tag
    e faz push se houver remoto. Sem CI do git.

### Verificado

- `npm run check` — ok.
- `npm run build` — ok.
- `cargo test` — 12 passed.
- `npm run test:e2e` — 2 passed (Playwright Chromium; `npx playwright install chromium`).
- `npm run tauri build` — gerou `.deb` (6,96 MiB), `.rpm` (6,97 MiB) e
  `.AppImage` (91,44 MiB).

### Observações

- `name`/`avatar` são metadados de apresentação do OpenDesk. O OpenCode V2
  retorna o `name` derivado do ID e ignora esses campos; `color` é nativo.
- O headless usa `tauri::async_runtime` para spawn, o que funciona sem janela.
- A pasta padrão inicial é `~/Documents/OpenDesk Workspaces` (ou
  `~/OpenDesk Workspaces` quando `Documents` não existe).
- Nenhum remoto git estava configurado no ambiente; commit/tag são locais e o
  script faz push automaticamente quando um remoto for adicionado.

### Próximo

1. Fase 3 — roteador DSH, fila, budget guard, sessões/prompt/SSE, permissões e
   proveniência.
2. Ampliar e2e para Kanban/fila quando os endpoints existirem.
3. Adicionar teste de contrato opt-in contra `opencode serve` real.

### Verificação extra do binário 0.2.0

- `releases/0.2.0/opendesk-linux-x86_64 --headless` respondeu
  `{"ok":true,"version":"0.2.0"}`.
- O binário de release abriu a janela Tauri e renderizou o dashboard com o menu
  de Configurações; SIGTERM encerrou o app e o daemon filho.
- Checksums e manifesto conferidos em `releases/0.2.0/`.

## 2026-11-06 — Fase 3: roteador DSH, fila, execução e proveniência

### Entregue

- **DSH Router**:
  - políticas `MANUAL`, `CHEAPEST`, `COST_BENEFIT`, `BEST_QUALITY` e
    `CUSTOM_RULES`;
  - regras IF-THEN com `all`/`any`/`not`, campos `tags`, `title`, `prompt`,
    `priority`, `estimated_complexity`, `budget_daily_pct`, `workspace_id` e
    `execution_mode`;
  - classificação de complexidade, escolha de fase Plan/Build, agente, modelo e
    effort;
  - resolução de modelo contra `/api/model` com fallback quando o modelo
    preferido não existe na instância;
  - `Context Packer` com regra obrigatória de output, arquivos de `input/`,
    plano aprovado e critérios de aceite.
- **Fila persistente**:
  - worker em Tokio com prioridade, pausa/retomada, cancelamento, retry,
    recuperação de jobs `RUNNING` após reinício e eventos `QueueUpdated`;
  - controle da fila no Kanban e endpoints REST.
- **Budget Guard**:
  - hard/soft cap diário, mensal e janela de 5h do opencode-go;
  - hard cap pausa a fila e emite `BudgetAlert`; soft cap notifica;
  - UI de configuração em Roteador & Custos.
- **Execução no OpenCode V2**:
  - criação de sessão com `location.directory`, agente e `Model.Ref`;
  - prompt assíncrono, assinatura do SSE global `/api/event`, filtragem por
    sessão, streaming de texto/raciocínio/tools e logs JSONL;
  - espera de conclusão por `session.idle`/`session.execution.*`, fallback
    `session/:id/wait`, timeout e interrupção;
  - broker de permissões: em runs autônomos responde `once` e grava
    `permission_audit`;
  - telemetria de tokens/custo/latência e resumo final.
- **Proveniência**:
  - diff autoritativo via `/api/session/:id/diff`;
  - fallback por tool calls `write`/`edit`/`patch` quando o diff vem vazio;
  - registro de arquivo, agente, motivo, tamanho, sha256, ação e patch em
    `artifacts_provenance`, com evento `ArtifactCreated`.
- **Kanban & Chat**:
  - Kanban com 5 colunas, drag-and-drop nativo, criação de tarefa, dispatch,
    aprovação de plano, logs por run e controlador da fila;
  - Chat & Criador com tarefa rápida que passa pelo router/fila;
  - Meta-Builder por linguagem natural para agentes, skills e comandos.
- **Auditoria automática**:
  - na política `BEST_QUALITY`, após Build bem-sucedido o OpenDesk enfileira um
    run `audit` read-only com o agente `explore`.
- **UI**:
  - Artefatos com filtros, preview, diff e ações;
  - Dashboard com runs reais, uso por modelo e feed de artefatos/runs;
  - Roteador & Custos com política, regras e budget editáveis.

### Verificado

- `cargo test` — **24 passed**.
- `npm run check` — ok.
- `npm run build` — ok.
- `npm run test:e2e` — **3 passed** (workspace, Studio, edição externa,
  Kanban/fila).
- Teste live com `deepseek/deepseek-flash`:
  - job `COMPLETED`, tarefa em `REVIEW`/`build_complete`;
  - tokens 13.816 input / 295 output, custo ~US$ 0,0023;
  - `hello.txt` criado em `output/2026-11/crie-hello-artifacts/`;
  - artefato registrado com ação `CREATED`, agente `build`, tamanho 11 bytes e
    diff.
- `crouton-alpha-free` falhou por restrição regional do provedor (403), não por
  bug do OpenDesk; o erro foi persistido corretamente no run e no job.

### Observações

- O broker de permissões aprova automaticamente (`once`) e audita. Uma política
  por workspace/UI pode ser adicionada na Fase 4/5.
- O diff do OpenCode pode vir vazio em workspaces sem snapshot; o fallback por
  tool calls cobre `write`/`edit`/`patch`.
- O evento `session.execution.failed` é tratado como falha do run, com a
  mensagem estruturada do provedor.
- A fila executa um job por vez; concorrência configurável fica para a próxima
  iteração.

### Próximo

1. Fase 4 — cron, pastas determinísticas por rotina, SMTP e entregas
   pós-processamento.
2. Concorrência configurável e política de permissões por workspace.
3. Testes de contrato opt-in contra `opencode serve` real.
4. Release 0.4.0 com cron/SMTP/entregas da Fase 4.

### Release 0.3.0

- `releases/0.3.0/` gerado localmente com `.deb` 7,27 MiB, `.rpm` 7,27 MiB,
  `.AppImage` 91,70 MiB e binário portátil.
- `releases/LATEST` = `0.3.0`; `v0.3.0` tagueado.
- Retenção mantém `0.2.0` e `0.3.0` (máximo 3).
- `/api/meta` agora reporta fase `execution` e os recursos das fases 1–3.

## 2026-11-06 — Fase 4: cron, pastas determinísticas e SMTP

### Entregue

- **Motor cron**:
  - tick de 15 s lendo `schedules` ativas do SQLite;
  - cálculo de próxima ocorrência com `croner` + `chrono-tz`;
  - `output/{YYYY}-{MM}/{schedule_slug}` criado antes do enqueue;
  - idempotência por índice único `(schedule_id, scheduled_for)`;
  - `run now`, pausa/retomada e `POST /api/schedules/tick` para testes;
  - execução de comandos `/nome args` expandindo templates do Studio.
- **SMTP**:
  - configuração por workspace com host, porta, usuário, remetente e TLS;
  - segredo AES-256-GCM em `~/.local/share/opendesk/secrets/` com 0600 (ADR 0005);
  - teste de conexão e envio de e-mail de teste;
  - entrega pós-run com resumo HTML, tabela de artefatos e anexos (até 5);
  - idempotência por `smtp:{run_id}:{recipient}` em `integration_deliveries`;
  - UI de Integrações com histórico de entregas.
- **UI de Rotinas**:
  - criação com cron, timezone, agente, comando/prompt, política, template de
    pasta e destinatários;
  - próxima execução, última execução/status, pausa, execução manual e exclusão.
- **Migração 0002**:
  - `execution_queue.output_dir`;
  - índice único de slot de schedule;
  - índice de schedules devidas.
- **Eventos**:
  - `ScheduleUpdated` para atualizar a UI em tempo real.

### Verificado

- `cargo test` — **28 passed**.
- `npm run check` — ok.
- `npm run build` — ok.
- `npm run test:e2e` — **4 passed** (workspace, Studio, Kanban/fila, rotina +
  SMTP).
- Teste headless:
  - schedule criado com `America/Sao_Paulo` e próxima execução 08:00 local;
  - `run now` criou `output/2026-11/relatorio-diario` e job `schedule`;
  - tick forçado enfileirou um slot vencido;
  - SMTP salvo com `password_set=true` e arquivo `0600`;
  - teste SMTP sem configuração retorna erro claro; com configuração tenta
    conexão real.

### Observações

- Entregas SMTP falhas ficam registradas como `FAILED` com `last_error`; retry
  automático fica para a próxima iteração.
- O segredo é vinculado ao machine-id; trocar de máquina exige recadastrar.
- O cron usa o slot agendado como chave de idempotência; `run now` usa o
  timestamp atual.
- Google Workspace e MCP bridge continuam Fase 5.

### Próximo

1. Fase 5 — OAuth2 Google, Drive/Docs/Calendar/Gmail/Tasks e MCP bridge.
2. Retry automático de entregas e política de permissões por workspace.
3. Concorrência configurável da fila.
4. Release 0.6.0 com endurecimento e testes de contrato.

### Release 0.4.0

- `releases/0.4.0/` com `.deb` 7,74 MiB, `.rpm` 7,74 MiB, `.AppImage` 92,08 MiB
  e binário portátil.
- `releases/LATEST` = `0.4.0`; `v0.4.0` tagueado.
- Retenção: `0.2.0`, `0.3.0` e `0.4.0` (máximo 3).
- `/api/meta` reporta fase `automation` e os recursos de cron/SMTP.

## 2026-11-06 — Fase 5: Google Workspace e MCP bridge

### Entregue

- **OAuth2 Google**:
  - Authorization Code + PKCE com callback loopback fixo
    `http://127.0.0.1:53682/callback`;
  - `client_id` em `app_settings`, `client_secret` e refresh token no secret
    store AES-256-GCM;
  - cache de access token com expiração e refresh automático;
  - UI para salvar credenciais, conectar, revogar e registrar MCP;
  - escopos de Docs, Drive (`drive.file`), Calendar, Gmail e Tasks.
- **Google API client**:
  - Drive: criar pasta, upload, busca;
  - Docs: criar a partir de Markdown via HTML, ler texto e append;
  - Gmail: enviar, criar rascunho e buscar threads;
  - Calendar: listar e criar eventos;
  - Tasks: listar e criar tarefas (Keep enterprise retorna erro explicativo).
- **MCP bridge**:
  - `opendesk mcp-google` via stdio, JSON-RPC 2.0, stdout reservado para frames;
  - ferramentas de Gmail/Docs/Drive/Calendar/Tasks/Keep;
  - registro automático em `mcp.servers.opendesk-google` com env de workspace;
  - validado manualmente com `initialize`, `tools/list` e `tools/call`.
- **Pós-run**:
  - export do artefato `.md` principal para Google Docs quando
    `export_to_gdocs`/`sync_to_google_docs` está ativo;
  - upload dos artefatos para a pasta do Drive do workspace;
  - links salvos em `artifacts_provenance` e entregas registradas como
    `GDOCS`/`DRIVE`.

### Verificado

- `cargo test` — **30 passed**.
- `npm run check` — ok.
- `npm run build` — ok.
- `npm run test:e2e` — **4 passed** (inclui salvar credenciais Google).
- Headless:
  - `GET/PUT /api/integrations/google/config` funciona;
  - `POST /oauth/start` gera URL com PKCE e callback;
  - `POST /mcp/register` grava `mcp.servers.opendesk-google` no
    `opencode.jsonc`;
  - bridge MCP responde `initialize`, lista 15 ferramentas e devolve erro claro
    quando a conta não está conectada.
- OAuth real e chamadas Google não foram executados por falta de credenciais de
  uma conta Google neste ambiente.

### Observações

- O usuário precisa criar um OAuth client Desktop no Google Cloud e cadastrar o
  callback exibido na UI.
- Google Keep permanece enterprise; `gtasks_*` é o caminho suportado.
- A exportação Google é best-effort e não falha o run; erros viram
  `integration_deliveries` com `FAILED`.
- O bridge MCP usa o mesmo store de segredos do app.

### Próximo

1. Endurecimento e testes com conta Google real.
2. Concorrência configurável da fila e retry de entregas.
3. Testes de contrato opt-in contra `opencode serve` real.
4. Release 0.5.0.

### Release 0.5.0

- `releases/0.5.0/` com `.deb` 7,97 MiB, `.rpm` 7,97 MiB, `.AppImage` 92,28 MiB
  e binário portátil.
- `releases/LATEST` = `0.5.0`; `v0.5.0` tagueado.
- Retenção: `0.3.0`, `0.4.0` e `0.5.0` (0.2.0 removida).
- Repositório privado publicado: https://github.com/ticosanjr/OpenDesk
- `/api/meta` reporta fase `integration` e os recursos de Google/MCP.
- Bridge MCP 0.5.0 responde initialize e lista 15 ferramentas.

## 2026-11-06 — Fase 6: resiliência do motor e atualizações (0.6.0)

### Causa raiz corrigida

O AppImage do OpenDesk herda um `PATH` que não inclui `~/.opencode/bin` e
executava `Command::new("opencode")`. O binário existia (`v2.2.5`), mas o spawn
falhava com `ENOENT`, o `daemon.json` nunca era criado e o app ficava offline.
Reiniciar o OpenCode não resolvia porque o problema era a descoberta do binário.

### Entregue

- **Resolver de binário** (`opencode_client/binary.rs`):
  - ordem: caminho salvo → `OPENCODE_BIN`/`OPENDESK_OPENCODE_BIN` → `PATH` →
    caminhos conhecidos (`~/.opencode/bin`, `~/.local/bin`, `~/.bun/bin`,
    `/usr/local/bin`, `/usr/bin`, Homebrew e equivalentes no Windows);
  - validação com `opencode --version` e exigência de V2;
  - detecção do método de instalação (`curl`, `npm`, `bun`, `brew`);
  - caminho escolhido persistido em `app_settings.opencode_bin_path`.
- **Fallback de serviço compartilhado**:
  - leitura de `~/.local/state/opencode/service.json` e
    `~/.config/opencode/service.json`;
  - attach com Basic auth, validação de `/api/info` e versão 2.x;
  - status reporta `mode: dedicated | shared | offline`.
- **Supervisor**:
  - daemon dedicado com `PATH` enriquecido;
  - lock file reutilizado quando o daemon anterior está saudável;
  - backoff, healthcheck e restauração automática.
- **Mensagens acionáveis**:
  - estados `starting`, `restarting`, `offline`, `online`;
  - banner global com motivo, “Configurar binário” e “Tentar novamente”;
  - card OpenCode Engine em Configurações com caminho, versão, porta, modo,
    caminhos testados, detecção, escolha de binário e reinício.
- **Preflight**:
  - dispatch, aprovação de plano e “executar agora” retornam 503 explicativo
    quando o motor está offline, em vez de enfileirar trabalho condenado.
- **Logs e diagnóstico**:
  - log rotativo diário em `~/.local/share/opendesk/logs/opendesk.log.*`;
  - `GET /api/engine/diagnostics` com daemon, binário, caminhos testados,
    arquivos de serviço compartilhado (sem senha) e tail do log;
  - botão “Copiar diagnóstico” e “Abrir logs”.
- **MCP estável no AppImage**:
  - o comando registrado usa `$APPIMAGE` ou `/usr/bin/opendesk` em vez do
    caminho efêmero `/tmp/.mount_*`.
- **Atualização do OpenCode**:
  - checagem de releases estáveis V2 no GitHub (não usa `/releases/latest`,
    que aponta para V1);
  - exibição de versão atual, última, notas e data;
  - atualização pela UI com `opencode upgrade --method <método> <versão>`,
    pausa da fila, recusa se houver job RUNNING, reinício e validação.
- **Atualização do OpenDesk**:
  - release privada no GitHub com `.deb`, `.rpm`, `.AppImage`, binário portátil,
    `SHA256SUMS` e `latest.json`;
  - token fine-grained no secret store (fallback para `gh auth token`);
  - verificação SHA256 antes de instalar;
  - AppImage/binário portátil: helper destacado espera o processo sair,
    substitui o arquivo, mantém `.bak` e relança;
  - `.deb`: download para `~/Downloads` com instrução `sudo dpkg -i`;
  - build de desenvolvimento apenas notifica.
- **Release script**:
  - gera `latest.json` com SHA256 de cada asset;
  - cria/atualiza GitHub Release e envia assets via `gh`;
  - mantém retenção local de 3 releases.

### Verificado

- `cargo test` — **55 passed** (resolver, updater, self-update, fila, budget,
  cron, JSONC, secrets, MCP).
- `npm run check` — ok.
- `npm run build` — ok.
- `npm run test:e2e` — **4 passed** (inclui card do motor e status offline).
- Headless com daemon real:
  - binário encontrado em `/home/rafael/.opencode/bin/opencode` (v2.2.5);
  - daemon dedicado online na porta 3101;
  - `mode: dedicated`, `binary_path` correto.
- Headless com `HOME` isolado e sem `opencode` no `PATH`:
  - spawn falhou como esperado;
  - fallback anexou ao serviço compartilhado em `127.0.0.1:49374`;
  - `mode: shared`, versão 2.2.5.

### Próximo

1. Testar atualização real do OpenCode pela UI.
2. Publicar a release 0.6.0 com `latest.json` e validar o auto-update.
3. Ampliar cobertura para ≥80% nos módulos de lógica.
4. Retry automático de entregas e concorrência configurável da fila.

## 2026-11-06 — Release 0.6.1

- Helper de auto-atualização agora copia para `<binário>.new`, dá `chmod` e usa
  `mv -f` (atômico no mesmo filesystem) antes de relançar.
- Mensagem de atualização indica encerrar o app também pela bandeja.
- `cargo test` — **48 passed** (testes adicionais do supervisor de daemon e do
  manifesto/SHA256 de atualização).

## 2026-11-06 — Release 0.6.2

- Backup de auto-atualização agora preserva o nome (`OpenDesk_*.AppImage.bak`).
- O helper de atualização remove o próprio script depois de relançar.
- Testes adicionais para tasks, schedules, router rules e settings.
- `cargo test` — **55 passed**.
- Atualização real validada de 0.6.0 → 0.6.1:
  - `GET /api/updates/status` detectou `update_available: true`;
  - download privado, SHA256 confere, binário substituído e `.bak` criado;
  - hash final idêntico ao asset 0.6.1.

## 2026-11-06 — Fase 7: usabilidade básica (0.7.0)

### Corrigido

- **Erros de tarefa acionáveis**:
  - `task_runs.error_code`, `error_message`, `error_hint` (migration 0003);
  - `QueueJobDto` agora expõe `error_message`, `retry_count`, `max_retries` e `finished_at`;
  - classificação automática de falhas (engine offline, auth, modelo/agente, timeout,
    cancelamento, budget, binário ausente);
  - `GET /api/tasks/{id}/diagnostics` com tarefa, jobs, runs, engine e tail do log;
  - modal da tarefa mostra “O que falhou”, “Como resolver”, copiar erro e últimas execuções;
  - LogDrawer mostra status colorido, erro, dica e resumo do run;
  - card do Kanban e itens da fila ficam vermelhos em falha.

- **Edição de tarefa**:
  - modal com título, descrição, modo, prioridade, agente, modelo, effort, tags,
    e-mail e exportação Google;
  - “Salvar” e “Salvar e reexecutar”;
  - `PATCH` permite limpar agente/modelo/plano (`clear_agent`, `clear_model`, `clear_plan`);
  - bloqueio de edição durante execução, com orientação para cancelar.

- **Kanban funcional**:
  - drag-and-drop com `dataTransfer` e fallback por menu “mover…”;
  - endpoint `POST /api/tasks/{id}/transition`:
    - PLAN inicia plano;
    - BUILD inicia build (aproveitando plano existente);
    - REVIEW inicia auditoria;
    - DONE conclui sem run;
    - BACKLOG cancela jobs ativos e limpa a fase;
  - confirmação antes de transições que consomem budget;
  - atualização otimista com rollback em erro;
  - deep link `/kanban?task=...&run=...` abre tarefa e log.

- **Agentes**:
  - ID gerado automaticamente do nome (ex.: “Revisor Fiscal” → `revisor-fiscal`);
  - ID manual fica opcional e não bloqueia a criação;
  - 20 presets de avatar + emoji livre;
  - 8 presets de cor + cor personalizada;
  - validação de ID duplicado.

- **Dashboard**:
  - atividades recentes unificadas (runs + artefatos) e ordenadas por data;
  - clique abre a tarefa/run no Kanban ou o artefato;
  - cores por status: falha vermelha, sucesso verde, execução azul, cancelado amarelo;
  - falha mostra a mensagem do erro na própria atividade.

- **Sobre & diagnóstico**:
  - `GET /api/about` com versões (OpenDesk, OpenCode, Tauri, Rust, OS/kernel),
    banco/schema, caminhos, engine, updates e logs;
  - card em Configurações com modos **simples** e **avançado**;
  - copiar resumo/JSON, abrir logs e abrir pasta de dados;
  - link direto para o diagnóstico do motor.

- **Base da 0.6.3 incorporada**:
  - concorrência configurável da fila (1–4) com API e seletor no Kanban;
  - retry inline de SMTP/Google com backoff;
  - `opendesk doctor`;
  - gate local de release e cobertura (frontend lógica ~90%, Rust ~41%);
  - clippy sem warnings.

### Verificado

- `cargo test` — **74 passed**.
- `npm run check` — ok.
- `npm run test:ui` — 16 passed; cobertura de lógica frontend 90,3% linhas.
- Gate local: `scripts/gate.sh`.
- E2E cobre: agente com slug automático e avatar preset, card Sobre, `/api/about`,
  diagnóstico de tarefa, transição para DONE e concorrência da fila.

### Próximo

1. Release 0.7.0 e validação visual no AppImage.
2. Chat real com qualquer agente.
3. Signal bridge via `signal-cli`.
4. Retry durável, rollback e assinatura de atualização.

## 2026-11-06 — Fase 8: Chat real e Signal (0.8.0)

### Chat

- Migration 0004 com `chat_threads` e `chat_messages`.
- Threads com agente, modelo, sessão OpenCode, origem e id externo.
- `GET/POST /api/chat/threads`, `GET/PATCH/DELETE /api/chat/threads/{id}` e
  `POST /api/chat/threads/{id}/messages`.
- Serviço de chat reutiliza a sessão do OpenCode, consulta budget, aguarda a
  resposta e devolve tokens/custo.
- UI real: lista de conversas, escolha de qualquer agente, modelo opcional,
  histórico, envio com Enter, exclusão e selo de origem Signal.
- Thread criada automaticamente para mensagens Signal.

### Signal

- Configuração em Integrações: conta, caminho do `signal-cli`, allowlist,
  workspace e agente padrão.
- Vínculo por `signal-cli link` com QR code renderizado no app.
- Bridge `signal-cli daemon --json` com allowlist obrigatória.
- Mensagens recebidas viram thread de chat e a resposta é enviada pelo Signal.
- Start/stop manual e desabilitado por padrão.
- Testado com `signal-cli` falso: mensagem recebida → chat real no OpenCode →
  resposta enviada pelo `signal-cli send`.

### Verificado

- `cargo test` — 82 passed.
- Chat real com OpenCode: pergunta “Responda apenas: OK” retornou “OK”,
  10.148 tokens de entrada e custo registrado.
- Signal bridge: envelope fake recebido, thread `source=signal` criada e
  resposta `SINAL OK` enviada ao destinatário.
- E2E: thread de chat criada/listada e card Signal presente em Integrações.

## 2026-11-06 — Release 0.8.1

- `SignalConfig` detecta automaticamente `~/.local/bin/signal-cli` quando o
  caminho não foi informado.
- O bridge, vínculo e envio do Signal agora enriquecem o `PATH` com Java
  recente (`/opt/java/current/bin`, SDKMAN), necessário para o signal-cli 0.14
  dentro do AppImage/minimal PATH.
- `probe` do OpenCode tolera `ETXTBSY` durante substituição do binário.

## 2026-11-06 — Fase 10: gestão completa do Studio (0.10.0)

### Studio

- Visão **Efetivo**: global + workspace, com origem, override e estado.
- Busca, filtro por tipo/estado, contadores e ações rápidas.
- CRUD continua disponível por tipo e escopo.
- `duplicate`, `copy` e `move` entre qualquer workspace e global, com novo ID e
  opção de sobrescrever.
- Ativação/desativação:
  - agentes/MCPs via `disabled` nativo;
  - skills/comandos movidos para `.opencode/disabled/...`.
- Backups por item com restauração pela UI.
- Instalação:
  - catálogo curado de MCPs (filesystem, git, fetch, memory, sequential
    thinking, Playwright);
  - GitHub (`owner/repo`), URL, pasta/arquivo local, JSON de MCP e npm;
  - preview item a item, destino workspace/global, conflito e avisos;
  - limites de 10 MB, validação de caminho e nenhuma execução de scripts.
- Editor de MCP mantém JSON bruto e agora participa do toggle/effective.

### Tarefas

- `PATCH /api/tasks/{id}` testado para gravar agente, modelo e limpar campos.
- Modal de edição usa select de modelos disponíveis + opção personalizada.
- Feedback “Tarefa salva”.
- Erros de região/provider (`This model is not available in your country`) agora
  são classificados como `model_unavailable` com dica de trocar de modelo.

### Limpeza

- `scripts/clean.sh` remove `target`, `dist`, `coverage`, `test-results`,
  `playwright-report`, `.vite` e temporários.
- `scripts/release.sh` executa a limpeza no final.
- `npm run clean` e `npm run clean:deep`.
- `.gitignore` cobre artefatos de build e cobertura.

### Verificado

- `cargo test` — 87 passed.
- `cargo clippy --all-targets -- -D warnings` — limpo.
- `npm run check` — ok.
- Playwright — 6 testes, incluindo cópia/toggle/effective/instalação npm e
  persistência de modelo em tarefa.
- Cobertura Rust — 40,23% (ratchet local); lógica frontend — 90,3%.

## 2026-11-06 — 0.10.1: Studio global, tarefas e modelos

### Studio

- `mcp` V1 (`mcp.<nome>` + `enabled`) e V2 (`mcp.servers` + `disabled`) são
  detectados e preservados em list/upsert/delete/toggle.
- O MCP global `rea` aparece no Studio e não é duplicado ao editar.
- Visão **Efetivo** é o padrão e mistura global + workspace.
- Inventário runtime do OpenCode (agentes/modelos) visível no Studio.
- Filtros, contadores, cópia/movimento, toggle e backups continuam funcionando.
- Fixture E2E com `opencode.jsonc` V1 + agente/skill/comando globais garante que
  o Studio realmente lê o global.

### Tarefas e Kanban

- `agent_override`/`model_override` em `tasks` (migration 0006).
- Editar modelo/agente passa a vencer a política do router.
- `model_health` (migration 0005) coloca modelos com erro de região/auth em
  cooldown por 24h.
- Transições persistem a coluna antes do dispatch; falha mantém a coluna e grava
  `dispatch_failed`.
- Modal de edição com combobox de modelo filtrável por texto.
- Toast “Tarefa salva”.
- E2E cobre edição de modelo pela UI e transição PLAN offline mantendo a coluna.

### Verificado

- `cargo test` — 90 passed.
- Playwright — 7 passed.
- Task real no OpenCode:
  - modelo `deepseek/deepseek-flash`;
  - status `SUCCESS`;
  - resumo `TASK OK`;
  - artefato criado em `output/2026-11/teste-real/TASK_OK.txt`;
  - job `COMPLETED`, sem erro.

## 2026-11-06 — 0.10.2: importação fiel, retry e rollback

- Importação de agentes/comandos/skills preserva frontmatter, permissões, modelo,
  modo e arquivos extras.
- GitHub usa default branch, `@ref` e token para repositório privado.
- Retry durável de SMTP em SQLite (`next_retry_at`, `max_attempts`, payload) com
  worker pós-restart e backoff.
- Rollback de OpenDesk pelo `.bak` e de OpenCode pelo binário anterior.
- Testes: 92 Rust, 8 Playwright.
- E2E cobre importação local com permissões/modelo e asset de skill.

## 2026-11-06 — 0.11.0: runtime inventory e execução longa

### Studio

- Runtime inventory via `/api/skill`, `/api/command`, `/api/mcp`, `/api/agent`.
- Efetivo mescla runtime + global + workspace + ancestrais.
- Itens runtime read-only com badges, contadores e status de MCP.
- Descoberta de `.opencode`/`opencode.json(c)` em diretórios ancestrais.
- Verificação real: 67 skills, 3 comandos e 1 MCP `rea` no ambiente do usuário.

### Execução

- Cliente SSE separado do cliente REST: sem timeout total de 120s.
- `Accept: text/event-stream` + `Accept-Encoding: identity`.
- Fallback para `wait_session`/`messages` quando o stream cai.
- Sucesso parcial preservado quando há artefatos.
- Task real: SUCCESS, `REVIEW/build_complete`, 2 artefatos.
- Daemons órfãos do OpenDesk são encerrados no boot.

### Testes

- `cargo test` — 95 passed.
- Playwright — 8 passed.
- `scripts/verify-real.sh` — REAL VERIFY OK.
- Clippy limpo.

## 2026-11-06 — 0.11.1: runtime location, fallback e verificação longa

- Studio ganhou seletor de **runtime location** (diretório do TUI/projeto).
- Importação de skill/agente runtime para disco (workspace/global).
- Model health visível em Configurações e aviso no combobox de modelo.
- Fallback automático para modelo saudável quando o modelo escolhido falha por
  região/auth.
- `opendesk doctor --fix-daemons` limpa daemons órfãos marcados.
- Teste E2E de drag-and-drop do Kanban.
- Testes de backup/restore e importação ZIP com assets.
- `scripts/verify-real.sh` aceita modelo/prompt e imprime duração.
- Verificação longa: `deepseek-v4.1-flash`, **148s**, SUCCESS, REVIEW,
  7 artefatos.

## 2026-11-06 — 0.12.0: rotinas intuitivas, notificações e edição

### Rotinas

- Formulário com recorrência estruturada (uma vez, horária, diária, semanal,
  dias úteis, mensal e cron custom) e prévia dos próximos disparos.
- `POST /api/schedules/preview` valida e devolve cron, descrição humana e as 5
  próximas ocorrências.
- `recurrence_json` é a fonte de verdade; `cron_expression` é regenerado no
  create/update e continua sendo o contrato do motor.
- Timezone padrão é o do sistema (`iana-time-zone`) e permanece editável.
- Rotinas de uma vez são desativadas após enfileirar; datas passadas são
  rejeitadas; `end_date` desativa a rotina quando expira.
- Disparo com motor offline é marcado `SKIPPED_OFFLINE` e o próximo slot é
  preservado.
- Rotinas continuam por workspace: a UI mostra o workspace ativo e orienta a
  trocar de workspace para gerenciar outro conjunto.
- Criar e editar usam o mesmo modal; a lista mostra recorrência humana, uma vez,
  Signal e “só novidades”.

### Notificações e agente

- `notify_signal` + destinatários (vazio usa a allowlist do Signal).
- `notify_only_on_change` compara SHA-256 do artefato principal/resumo com
  `last_output_hash`; sem novidade, exporta Google mas omite e-mail/Signal.
- Agente é um combobox opcional com busca que filtra por ID/nome; escolher um
  agente herda a política do roteador até edição manual.
- Prompt abre menu `/` com comandos e skills do runtime.
- Template da pasta de saída tem default e fica abaixo do prompt com descrição.

### Testes

- `cargo test` — 103 passed.
- `vitest` — 31 passed (inclui `src/lib/recurrence.test.ts`).
- Playwright — 10 passed, incluindo recorrência, busca de agente, menu `/`,
  notificações e edição de rotina.
- Modal ganhou rolagem vertical para formulários altos.
- `cargo clippy --all-targets -- -D warnings` limpo.

## 2026-11-06 — 0.13.0: explorador, feedback global e gestão de workspace

### Exploração e workspace

- Página Arquivos real com árvore navegável, busca local, breadcrumb, tamanho,
  data, prévia textual e ações de abrir/copiar caminho.
- API `/api/files/tree` e `/api/files/content` com validação de caminho,
  bloqueio de symlink/`..`, limite de 512 KB e omissão de pastas pesadas.
- Workspace editável (nome, cor, política, timezone e e-mail padrão) e
  removível sem apagar a pasta no disco.
- Switcher de workspace com busca, cor, caminho e atalhos para criar/gerenciar.

### Feedback e acessibilidade

- `Toaster` global com variantes, ações e erros persistentes.
- `ConfirmDialog` substitui os 11 `window.confirm`.
- `ErrorBoundary` por aplicação com recarregar/tentar novamente.
- Alerta de budget e progresso de atualização aparecem no shell.
- Notificações nativas para runs concluídos/falhosos.
- Modal com focus trap, restauração de foco e `aria-label` no fechamento.
- Tray: pausar/retomar fila realmente funciona e atualiza o rótulo.

### Formulários e produtividade

- `AgentCombobox` compartilhado (global + workspace + runtime + built-ins +
  ID manual), `ModelCombobox` com filtro/health e `SlashTextarea` com `/`.
- Kanban: board horizontal, busca, filtro por agente/prioridade e formulário
  único de criar/editar com agente global, modelo pesquisável, e-mail e Google.
- Chat: busca de conversas, renomear inline, pickers compartilhados e `/`.
- Rotinas: duplicar, testar notificação e histórico por rotina.
- Command palette busca tarefas, rotinas, artefatos e agentes.
- Atalhos: `Ctrl/Cmd+K` (busca), `Ctrl/Cmd+N` (nova tarefa), `Ctrl/Cmd+Shift+R`
  (atualizar dados).
- Página de Configurações e Integrações ganharam navegação por seções.
- Dashboard usa agentes do disco quando o engine está offline e separa gasto
  diário do gasto acumulado.

### Performance e robustez

- Rotas com `React.lazy`: chunk inicial caiu de ~642 KB para ~366 KB
  (~115 KB gzip).
- Filtro de artefatos com debounce de 250 ms.
- Logs de runs em andamento atualizam a cada 3 s.
- `task_runs.schedule_id` + índice para histórico de rotinas.
- Backup rotativo do SQLite antes de migrações.

### Testes

- `cargo test` — 107 passed.
- `vitest` — 41 passed; cobertura de linhas 92,4%.
- Playwright — 12 passed, incluindo explorador de arquivos e edição/remoção de
  workspace.
- `cargo clippy --all-targets -- -D warnings` limpo.
- `scripts/gate.sh` passou.

## 2026-11-06 — 0.14.0: timeline de execução e referências navegáveis

### Timeline

- `GET /api/runs/{id}/timeline` estruturou o JSONL em:
  - raciocínio (thinking) com texto, início/fim e duração;
  - execução com ferramentas, input/output, status, timestamps e duração;
  - resultado não colapsado;
  - passos com agente/modelo/tokens/custo/finish.
- Layout no estilo OpenCode: raciocínio e executado colapsados, resultado
  aberto, modo bruto, copiar, atualizar e polling de runs em andamento.
- Drawer do Kanban reutiliza o mesmo `RunLogView`; nova página `/runs/:id`.
- Nova página `/runs` com busca e filtro por status.
- Sidebar reorganizada em Operação / Biblioteca / Governança, com “Execuções”.

### Navegação e notificações

- `links.ts` + `EntityLink` para tarefa, run, artefato, rotina, agente, arquivo,
  settings e integrações.
- Deep links com foco/destaque para rotina, arquivo, thread, agente e seções de
  settings/integrações.
- Toaster aceita `href` e `key`; o corpo do toast abre o destino.
- Runs concluídos/falhosos, artefatos criados, falhas de tarefa/rotina, Google,
  budget e engine agora linkam para onde apontam.
- Histórico de rotinas, execuções de tarefa, dashboard, artefatos e drawer têm
  links para `/runs/:id`.

### Testes

- `cargo test` — 110 passed (inclui parser de timeline com fixtures).
- `vitest` — 48 passed (inclui timeline renderizado, links e dedupe de toasts).
- Playwright — 13 passed.
- `cargo clippy --all-targets -- -D warnings` limpo.

## 2026-11-06 — 0.15.0: status Paperclip, execução conversacional e visualizadores

### Tarefas e Kanban

- `tasks.status` com sete estados Paperclip; `phase` separado do status.
- `attempt_number`, `session_id`, `last_run_status`, `last_run_at`,
  `blocked_reason` e `cancelled_at`.
- Build bem-sucedido vai para DONE; falha vira BLOCKED; cancelamento vira
  CANCELLED; interrupção por restart não é reenfileirada automaticamente.
- Removidos fallback automático de modelo, auditoria automática e retry
  automático.
- Kanban com sete colunas, scroll interno, engrenagens animadas, último run no
  card e ações Cancelar/Bloquear/Espera/Reabrir/Concluir.
- Dropdown “mover” removido; movimento apenas por drag-and-drop.

### Execução conversacional

- `task_messages` para respostas/comentários do usuário.
- Timeline intercalada: passo, pensamento, ferramenta, comentário e resultado.
- Modo conversa/faixas/bruto no visualizador.
- Rodapé de execução com agente, modelo, mensagem e “continuar execução”.
- `POST /api/tasks/{id}/messages` grava e enfileira nova tentativa.

### Arquivos, artefatos e busca

- `FileViewer` com Markdown, HTML sandbox, imagens, PDF, vídeo, áudio, código e
  binários.
- Endpoints raw autenticados com MIME e limite de 250 MB.
- `/search` com abas por tipologia e busca em tarefas, comentários, arquivos,
  artefatos, agentes, projetos, rotinas e execuções.
- Catálogo curado de repositórios no Studio e cards expansíveis.

### UI

- Piso global de tipografia (8–10px → 10–12px), mais peso e ícones.
- Tooltip acessível e indicador de execução com engrenagens.
- Scroll interno em colunas, logs, Studio, artefatos e modais.

### Testes

- `cargo test` — 110 passed.
- `vitest` — 48 passed.
- Playwright — 15 passed.
- Clippy limpo.

## 2026-11-07 — 0.15.1: versão no tray e contrato de scroll

- Menu da bandeja mostra `OpenDesk vX.Y.Z` no topo e no tooltip.
- `PageBodyFill` + regras de `min-h-0`/`overflow` para telas split-pane.
- `100dvh` no shell, modais, drawers e run detail.
- Kanban sem scroll aninhado nas colunas: board rola na horizontal e a página
  rola na vertical.
- Files/Artefatos com visualizador ocupando altura disponível e corpo rolável.
- Studio, ConfigPanel, LogDrawer e RunLogView com wrappers flex corrigidos.
- Tabelas e blocos de diagnóstico com scroll nos dois eixos.
- Auditoria Playwright de scroll em todas as rotas, 1100×700 e 1440×900.
- `cargo test` — 111 passed; Playwright — 16 passed.

## 2026-11-07 — 0.15.2: model refs e notificações

- Corrigido `variant: null` em `create_session`: modelos sem variante agora
  funcionam em tarefas, chat, rotinas e fallbacks.
- Parser único `ModelRefDto::parse`/`to_payload`; `create_session` omite `model`
  quando ausente e `variant` quando vazio.
- Validação de modelo em tarefas, chat, dispatch, mensagens, agentes, comandos e
  regras do roteador.
- Fechar a janela agora encerra o app em vez de esconder na bandeja.
- Notificações de tarefa apenas em `DONE`/`BLOCKED`/`CANCELLED`; runs de rotina
  uma vez por run.
- Artefatos agrupados por run; falhas de rotina só quando `last_run_at` muda.
- Dedupe central `markNotificationOnce`, native só com janela oculta, permissão
  solicitada uma vez e toasts limpos ao esconder/descarregar.
- `cargo test` — 114 passed; `vitest` — 52 passed.

## 2026-11-07 — 0.16.0: continuidade de sessão, preferências e Kanban em lista

- Follow-up de tarefa reutiliza `session_id` da sessão OpenCode.
- `execution_queue.session_id` + `task_runs.session_id` persistidos.
- Preferências de notificação: nativas, artefatos e fechar para bandeja.
- `close_to_tray` respeitado no `CloseRequested`.
- Botão Executar desabilitado com tooltip quando o engine está offline.
- Kanban com alternância Board/Lista, seleção múltipla e ações em lote.
- Paginação “carregar mais” em Artefatos e Execuções.
- `LazyFileViewer` para carregar o visualizador sob demanda.
- Ajuda de atalhos com `?`.
- `cargo test` — 114 passed; `vitest` — 52 passed.

## 2026-11-07 — 0.17.0: resiliência de fila, atenção, cron e entregáveis

- Lease/heartbeat de jobs (`lease_expires_at`, `last_heartbeat_at`), heartbeat
  de 5s e interrupção de lease expirado.
- Attention Card no Dashboard com aprovação de plano, tarefas bloqueadas e
  falhas das últimas 24h.
- Cron com `misfire_policy`, `safe_readonly_mode`, catch-up e dry-run.
- Rotinas com dry-run, histórico, duplicação e teste de notificação.
- Artefatos fixáveis (`pinned`) com estrela no explorador e no detalhe.
- `MEMORY.md` por workspace com botão “memória” na execução.
- Fork de tarefa (`forked_from_task_id`).
- Model mix/economia dos últimos 7 dias no Dashboard.
- `cargo test` — 115 passed; Playwright — 19 passed.

## 2026-11-07 — 0.18.0: permissões interativas, swarm e captura rápida

- Permissões interativas (`permission_requests`) com decisão once/always/reject
  direto no Attention Card e estados BLOCKED/WORKING/DONE/IDLE por agente.
- Medidor de contexto por tarefa e compactação manual/automática via API do
  engine.
- A/B swarm em 2–4 modelos com pastas isoladas e escolha do vencedor.
- Fork de tarefa a partir de um run, copiando os artefatos até aquele ponto.
- Quick Capture global (`Ctrl/Cmd+Shift+Space`) com texto, link e imagem colada.
- Rollback de artefato criado/editado em um clique.
- Blueprints de equipe ao criar workspace.
- Retry de falha em outro modelo pelo Attention Card.
- Entregáveis fixados no Dashboard e nos cards do Kanban.
- `cargo test` — 124 passed; `clippy` limpo.

## 2026-11-07 — 0.18.1: captura com subagente e comparação lado a lado

- Quick Capture aceita agente/subagente opcional e envia com override explícito.
- Swarm A/B mostra os entregáveis de cada modelo em duas colunas assim que os
  runs terminam.

## 2026-11-07 — 0.19.0: cache, mídia, aprovação remota e chat com anexos

- Prefixo estável no context packer para maximizar prompt caching do OpenCode Go.
- Cache hit (`tokens_cache_read` / input) exposto em `/api/cost/mix` e no card.
- Metadados de vídeo/áudio via ffprobe em `/api/files/media` e no explorador.
- Aprovação de permissão remota pelo Signal (`APROVAR`/`NEGAR` + prefixo).
- SecretStore aceita variáveis `OPENDESK_<NAMESPACE>_<KEY>` como fallback.
- Chat aceita imagens e arquivos por colagem/arraste, salvos em input/chat/.
- `cargo test` — 125 passed.

## 2026-11-07 — 0.20.0: bandeja, abas, timeline ao vivo e fila

- Janela principal abre maximizada; “X” esconde na bandeja por padrão e “Sair”
  no tray encerra. Quick Capture fecha normalmente.
- Configurações, Integrações e Kanban em abas reais; somente o painel ativo é
  renderizado.
- Timeline de execução atualiza por `RunStreamChunk` com debounce, mantém status
  RUNNING até evento terminal e mostra artefatos/pasta de resultados.
- Filtro de artefatos por `run_id` e links diretos para artefato, pasta e
  explorador.
- Controlador de fila em aba própria com lease, heartbeat, grupo de concorrência,
  prioridade, retry em lote e pasta de saída.
- Kanban com destaque de coluna de destino e auto-scroll no arraste.
- `cargo test` — 126 passed; Playwright — 20 passed.

## 2026-11-07 — 0.21.0: barramento de I/O, ferramentas externas e hardening

- Migração 0016 com sources, sinks e bindings.
- Core UEN com ingestão idempotente e sinks pós-run.
- Inbounds: dropzone/OCR/Whisper, IMAP, SQL poller, webhooks assinados,
  Telegram e Matrix.
- Sinks: compilador XLSX/PDF, FS atômico, S3/MinIO, Git, SQL, SMTP e chat.
- UI de Integrações: Entradas, Saídas, Histórico, MCP e segurança.
- Vínculo de sinks em tarefas e rotinas.
- Ferramentas externas com detecção/download em pasta fora do release.
- Permissão com timeout que libera o worker.
- Watcher com supressão de eco.
- Manutenção periódica do SQLite (checkpoint, backup, retenção).
- Quick Capture fecha ao perder foco e reutiliza a janela.
- Protocolo V2 documentado em `docs/protocol/opencode-v2.md`.
- `cargo test` — 133 passed; Playwright — 21 passed.

## 2026-11-07 — 0.22.0: receitas declarativas, HITL e Rsync

- Migração 0017 com recipes_index, recipe_runs e hitl_requests.
- Parser/validação YAML, sync por watcher, indexação por workspace.
- Motor de receitas com steps sequenciais, variáveis, checkpoint, retomada de
  estado e sinks via io_bus.
- HITL com token one-shot, timeout, Desktop UI, e-mail SMTP/IMAP e Signal.
- Modalidades binária, múltipla escolha, feedback, edição de artefato e proxy.
- Página Receitas com stepper visual e editor YAML Monaco.
- Attention Card lista checkpoints HITL pendentes.
- Rsync para pastas locais/remotas: sink e trigger, com chave SSH e `--delete`
  opt-in.
- rsync no catálogo de ferramentas externas.
- `cargo test` — 159 passed; Playwright — 22 passed.

## 2026-11-07 — 0.22.1: hotfix de inicialização e migração incremental

- Corrigida a inicialização do runtime de receitas no Tauri GUI: `tokio::spawn`
  agora roda dentro do runtime async, evitando panic no boot.
- Migração 0017 mantida intacta para compatibilidade de checksum; a coluna
  `options_json` passou para a migração 0018.
- Testado boot GUI com janela, boot headless e upgrade de um banco criado pela
  v0.22.0.

## 2026-11-07 — 0.23.0: auto-organização de agentes com aprovação

- Tabela `config_proposals` e API de propostas de configuração.
- MCP `opendesk-config` para agentes proporem agentes, skills, comandos,
  receitas, rotinas, MCPs e arquivos `.opencode`.
- Attention Card com Aprovar/Negar propostas; aplicação usa os mesmos writers
  do Studio/receitas/schedules.
- Workspaces padrão em `~/Documents/OpenDesk Workareas`; raízes temporárias
  recusadas fora do modo de teste/Rogue (`OPENDESK_ALLOW_TEMP_WORKSPACES=1`).
- `cargo test` — 162 passed; Playwright — 24 passed.

## 2026-11-08 — 0.23.1: receitas contínuas, gatilhos I/O e Signal

- Steps de receita reutilizam `session_id`, mantendo contexto entre etapas.
- Receitas podem ser acionadas por fontes de I/O via `config.recipe_id`
  (webhook, IMAP, Signal, SQL poller e dropzone).
- Runtime de receitas ganhou gatilho CRON com timezone e catch-up.
- Signal agora extrai quote, reação, anexos e tenta transcrever áudio de voz.
- Workspaces padrão em `OpenDesk Workareas`; `/tmp` só com
  `OPENDESK_ALLOW_TEMP_WORKSPACES=1` para testes/automação Rogue.
- Testes: 162 Rust, 55 Vitest, 24 Playwright.

## 2026-11-08 — UX por workspace, fluxo de receitas e rotinas por evento

- Tema sutil do workspace ativo em divisórias, foco, tabs, navegação e scroll.
- Attention Card reorganizado em seções, com aprovar/remover/dispensar e uma
  única chamada para permissões, HITL e propostas.
- Kanban sem Backlog; tarefas nascem em TODO, cards compactos e filtro de
  retenção para Done/Cancelled por período e quantidade, persistido por
  workspace.
- Controlador da fila compactado com contadores em pílula.
- Receitas: fluxo único step/checkpoint/saída reordenável, zero ou múltiplos
  gatilhos, nomes humanos, tooltips, comboboxes, seletor de pasta e editor YAML
  local (sem carregamento infinito).
- Rotinas: gatilhos CRON, pasta, Rsync, IMAP, Signal, webhook e SQL; execução
  de prompt ou receita; formulário em accordion.
- Arquivos: busca global no workspace; Roteador: modelo roteador e comparação
  de custos sob demanda; ferramentas externas com detecção, download e caminho
  manual.
- Pin de artefatos corrigido (`pinned` na lista de colunas).
- Testes: 167 Rust, 58 Vitest, 24 Playwright.

## 2026-11-08 — 0.24.0: pacotes iniciais e confiabilidade de rotinas

- 24 pacotes opcionais (24) e 6 templates de workspace embutidos no binário.
- Página Pacotes com catálogo, prévia, conflitos, instalação e remoção segura.
- Instalação grava agentes, skills, comandos, receitas, rotinas inativas e
  pastas; remoção preserva arquivos modificados e rotinas já executadas.
- Rotinas de prompt e de receita agora atualizam `last_run_status`
  (SUCCESS/FAILED/REJECTED/TIMED_OUT/CANCELLED) e o histórico de receita é
  vinculado ao `schedule_id`.
- Propostas MCP de rotina criam a fonte de I/O correspondente; delete remove.
- Boot sincroniza fontes de rotina antes de iniciar o I/O bus.
- Remover tarefa cancela jobs ativos; dismiss do Attention é versionado por
  `updated_at` e limpo após 90 dias.
- Editor YAML sincroniza números de linha com o scroll.
- Migração `0022`; ADR 0032; `docs/starter-packs.md`.
- Testes: 174 Rust; Vitest e Playwright executados no gate.
