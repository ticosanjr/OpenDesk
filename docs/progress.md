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

## 2026-11-08 — 0.24.1: P0 de pacotes e receitas

- Reinstalação de pacote não duplica rotinas; rotina preservada após remoção é
  reutilizada na reinstalação.
- Receitas com gatilhos IMAP, webhook e SQL criam/atualizam/removem fontes de
  I/O automaticamente; Signal também pode acionar receitas.
- Receita interrompe se um step termina FAILED/CANCELLED e marca a rotina.
- Pacotes reportam status `installed`, `modified` ou `partial`, com arquivos/
  rotinas faltando e ação de reparo.
- Modal de execução de receita aceita contexto para `{{trigger.context}}`.
- Packs corrigidos: caminhos de entrada por pacote e agente principal `primary`.
- Validação de frontmatter de agentes/skills/comandos e de rotinas no bundle.
- Versão 0.24.1 (sem release).

## 2026-11-08 — 0.24.1: P1 de cobertura e confiança

- `build_router` exposto para testes de contrato com `tower::ServiceExt::oneshot`.
- Testes de API para health/auth, pacotes, conflito com overwrite, busca de
  arquivos, dismiss versionado do Attention, fontes de receita/rotina e
  criação de workspace por template.
- Teste de upgrade de banco legado (0019 → 0022): Backlog vira TODO, colunas de
  trigger e `alert_dismissals` são criadas.
- Gate de cobertura global elevado para 44% e novo gate de core (46%) em
  `scripts/coverage-core.sh`, cobrindo db/api/recipes/queue/io_bus/config_sync/
  dsh_router/starter_packs.
- Engine interrompe receita em step falho (já coberto) e rotinas reutilizam
  fontes ao instalar pacotes.

## 2026-11-08 — 0.24.1: P2 de produto e performance

- Pacotes mostram atualização disponível (versão instalada x versão do bundle)
  e aceitam seleção por arquivo em conflitos; arquivos não selecionados são
  preservados.
- Instalação em lote: receitas são indexadas uma vez por pacote em vez de uma
  sincronização por receita.
- Detecção de ferramentas ganhou cache de 30s, `refresh=1` para verificação
  manual e execução fora do runtime com `spawn_blocking`.
- Editor YAML faz parse com debounce de 200ms.
- Busca de arquivos usa índice em memória por 15s, invalidado ao instalar ou
  remover pacote, e prioriza nome sobre caminho.
- Cards do Kanban e jobs da fila usam `content-visibility` para reduzir render
  fora da viewport.
- Roteador ganhou peso entrada/saída configurável e parecer textual local
  comparando menor/maior custo e spread.

## 2026-11-08 — 0.24.1: cobertura Playwright ampliada

- Novos cenários de UI: conflito por arquivo em pacotes, reparo de pacote,
  uninstall preservando edição, template de workspace, fluxo de receita com
  reordenação e múltiplos gatilhos, contexto de execução, dismiss/remoção no
  Attention, busca global de arquivos, tema do workspace + retenção do Kanban,
  accordion de rotina com receita e blend do roteador.
- Helpers Playwright com workspace isolado e retry para locks transitórios.
- Suíte E2E passa de 25 para 35 testes.

## 2026-11-08 — 0.24.1: auditoria visual e otimizações seguras

- Novo teste Playwright de auditoria de UI em 1440x900 e 1100x800, cobrindo
  todas as rotas principais e modais: overflow horizontal, elementos cobertos,
  botões sem nome acessível e erros não capturados.
- Corrigido o cabeçalho de Arquivos que fazia o seletor “Todo o workspace” ficar
  sob o painel de pré-visualização.
- Nome acessível em botões de histórico/exclusão do Studio e exclusão de rotina.
- Sidebar mais compacta: Configurações permanece visível em 1100x800.
- Kanban com badge de status sem quebra de linha.
- Receitas exibem `@agente` compacto no stepper, sem truncamento estranho.
- Título “Rotinas (Cron)” corrigido para “Rotinas & Gatilhos”.
- Otimizações: debounce de eventos no Attention Card, memoização de fila e tema
  do workspace, e índice de arquivos construído fora do lock.

## 2026-11-08 — 0.24.1: cobertura 90%+ e DRY

- Cobertura unitária da lógica fechou em 99,54% de linhas, 98,62% de
  statements, 99,25% de funções e 91,66% de branches; thresholds do Vitest
  elevados para 90% em todas as métricas.
- Novos testes para formatClock/formatDateTime, links de runs, notificações
  nativas, retenção (hoje/7d/storage inválido), recorrência, trigger types,
  apiPatch, toast clear, reconexão WebSocket e eventos parciais do app store.
- E2E: helper único de API/workspace substitui cópias em todas as specs;
  auditoria de UI cobre 14 rotas em 1440×900/1100×800 e 4 modais.
- DRY: classes de formulário centralizadas em `src/lib/ui-classes.ts`,
  `TemplateCard` extraído e cards do Kanban/pacotes/arquivos ajustados.

## 2026-11-08 — 0.24.1: Fábrica de Capacidades e ícone unificado

- Novo pack `meta-forge` com 4 agentes, 6 skills, 5 comandos, 6 receitas e
  rotina semanal de auditoria.
- Metodologia eval-driven: baseline, evals de capability/regression, referência,
  progressive disclosure, red team e refino por falhas.
- Workspace default `Fábrica de Capacidades` criada uma vez, com template
  reutilizável e sem roubar o foco de workspaces existentes.
- Ícones de tray, janela e bundle regenerados a partir de `public/opendesk.svg`;
  tray usa explicitamente o PNG 32×32.
- `OPENDESK_WORKSPACES_ROOT` adicionado para isolamento de testes.
- Teste E2E garante agentes/skills/comandos/receitas da fábrica.
- Criação interna da fábrica usa `create_trusted`, para que um
  `OPENDESK_WORKSPACES_ROOT` isolado funcione sem afrouxar a validação de
  workspaces criados pelo usuário.
- Smoke headless e GUI (Xvfb) confirmam criação única e ausência de panic.

## 2026-11-08 — pós-0.24.1: retry durável Google e estabilidade do gate

- Entregas `GDOCS` e `DRIVE` agora usam `mark_failed_retryable` com payload
  mínimo (artifact, título e pasta) e são retomadas pelo worker durável após
  restart, com backoff exponencial igual ao do e-mail.
- O retry recarrega o artefato do banco e do disco; se ele sumiu, a entrega vira
  `SKIPPED` em vez de falhar para sempre. `SMTP`, `GDOCS` e `DRIVE` compartilham
  o mesmo dispatcher por canal.
- Histórico de entregas ganhou o botão **Reenviar** para linhas `FAILED`, com
  `POST /api/integrations/deliveries/{id}/retry`. Só entregas `FAILED` com
  payload são reenviadas; `SUCCESS` nunca é replicada para evitar documento ou
  upload duplicado.
- `busy_timeout` do SQLite subiu de 5s para 15s para absorver rajadas de escrita
  dos workers em paralelo com a API.
- `model-ref.spec.ts` passou a criar workspace própria com `seedWorkspace`, sem
  disputar a workspace mais recente de outras specs.
- `scripts/release.sh` lê `docs/releases/<versão>.md` e usa o mesmo texto no
  GitHub Release e no `latest.json` do updater; notas da 0.24.1 retroativas
  adicionadas. `SHA256SUMS` passou a ser gerado fora do próprio hash.
- `docs/progress.md` sem a seção duplicada de cobertura; números finais do gate.
- Testes: 198 Rust (3 de retry durável Google + 2 do endpoint de reenvio),
  81 Vitest, 38 Playwright.

## 2026-11-08 — 0.24.2: cobertura Rust e confiabilidade de entregas

- Fase 6 iniciada com 26 testes de contrato da API cobrindo tarefas, rotinas,
  Studio (duplicar/copiar/backup/toggle), arquivos, busca global, fila,
  orçamento, custo, chat, roteador, propostas, artefatos, I/O bus, SMTP, Signal,
  engine/diagnóstico e receitas (sync + dry-run).
- Cobertura Rust subiu de 46,31% para **55,00% de linhas** e o core de 49,26%
  para **59,72%**. Ratchet do gate elevado para 55% global e 59% core.
- Testes Rust passaram de 198 para **213**.
- Retry durável de Google Docs/Drive, reenvio manual no histórico, timeout de
  SQLite em 15s e `model-ref.spec.ts` determinístico incluídos no release.
- `scripts/release.sh` usa notas de `docs/releases/<versão>.md` no GitHub e no
  `latest.json`; `SHA256SUMS` não inclui a si próprio.

## 2026-11-08 — pós-0.24.2: Fase 1 (toggles, modelo explícito e opções avançadas)

- Novo `Switch` acessível usado nas rotinas e receitas. Rotinas alternam pelo
  card; receitas ganharam `POST /api/recipes/{id}/enabled`, que altera apenas a
  linha `enabled:` do YAML preservando comentários e reindexa a receita.
- `ModelCombobox` agora tem dois estados explícitos: **Roteador automático**
  (`null`) ou **Modelo específico**. Valor vazio não existe mais; o padrão é o
  roteador.
- Roteador nunca troca um modelo escolhido pelo usuário: o classificador mantém
  o modelo manual mesmo se ele estiver indisponível ou em cooldown; o erro real
  do provider aparece em vez de um fallback silencioso.
- Tarefa: título e prompt ficam visíveis; modo, prioridade, agente, modelo,
  effort, tags, e-mail e Google Docs ficam em **Mais opções**, com resumo do que
  está configurado.
- `model-selection.ts` centraliza a regra “vazio ⇒ roteador”.
- Testes: 217 Rust, 84 Vitest e 39 Playwright; E2E da fase cobre toggles,
  persistência, modelo fixo e opções avançadas.

## 2026-11-08 — pós-0.24.2: Fase 3 (calendário local e agenda unificada)

- Migration `0023_calendar`: `calendar_sources`, `calendar_events` e
  `calendar_trigger_fires`.
- Cada workspace ganha um calendário local (“Meu calendário”) sob demanda.
- CRUD de eventos em `/api/calendar/events` com validação de título, intervalo,
  fuso e cor; `GET /api/calendar/events/{id}` para deep links.
- Nova agenda unificada `/api/calendar/agenda` reúne eventos, ocorrências
  futuras de rotinas CRON, receitas CRON, execuções (`task_runs` e
  `recipe_runs`) e jobs futuros da fila, com status `PAST/ACTIVE/FUTURE`.
- Página `/calendar` com grade mensal de 6 semanas, agenda do dia, filtros por
  rotinas/receitas/execuções/eventos e CRUD de eventos locais.
- Navegação, command palette, deep links (`/calendar?date=`,
  `/calendar?event=`) e suporte a `/recipes?recipe=` adicionados.
- Testes: 222 Rust, 89 Vitest e 40 Playwright; E2E cobre criar, editar,
  filtrar e excluir evento e visualizar uma rotina no calendário.

## 2026-11-08 — pós-0.24.2: Fase 4 (Google Calendar)

- Escopo OAuth ganhou `calendar.readonly`; contas já conectadas precisam
  reconectar para listar calendários.
- `GoogleApi` agora lista calendários (`calendarList`), pagina eventos com
  `syncToken`/`showDeleted`, cria em calendário específico, aplica `patch` e
  exclui eventos. Eventos de dia inteiro usam `date` no Google e são
  normalizados para datas UTC no cache local.
- Fontes Google podem ser importadas, ativadas/desativadas para exibição e
  sincronização, sincronizadas sob demanda e desconectadas. O calendário local
  não pode ser removido.
- Sync incremental a cada 5 minutos; token expirado (`410`) cai para full sync
  de −30/+180 dias. Eventos cancelados viram `CANCELLED` e somem da agenda.
- Criar, editar e excluir evento de fonte Google faz write-through e só então
  atualiza o cache local; falha do Google não altera o cache.
- Página do calendário ganhou botão Google/importação, sincronização manual e
  seletor de calendário ao criar evento. Fontes invisíveis não aparecem na
  agenda.
- Testes: 229 Rust, 89 Vitest e 40 Playwright; parser de evento Google, upsert
  idempotente, corrida do calendário local, fontes, importação sem OAuth e
  visibilidade cobertos.

## 2026-11-08 — pós-0.24.2: Fase 5 (eventos de calendário como trigger)

- Novo gatilho `CALENDAR_EVENT` para receitas, com configuração de calendário,
  filtro por título, antecedência (`lead_minutes`) e tolerância
  (`lookahead_minutes`).
- Runtime varre eventos do cache a cada 10s; Google usa o cache sincronizado a
  cada 5min, e a tolerância padrão de 10min cobre esse atraso.
- `calendar_trigger_fires` garante uma execução por receita/evento/ocorrência,
  inclusive após restart. Eventos cancelados não disparam.
- Contexto disponível no prompt como `{{trigger.calendar_event.title}}`,
  `start`, `end`, `description`, `location` e `id`.
- Editor de receitas ganhou campos do gatilho e carrega os calendários do
  workspace.
- Testes: 231 Rust, 89 Vitest e 40 Playwright; varredura idempotente e claim
  cobertos.

## 2026-11-08 — 0.25.0: calendário completo, gatilhos e cobertura

- Fase 1: switches de rotina/receita, modelo explícito (roteador x específico)
  e opções avançadas recolhidas na tarefa.
- Fase 3: calendário local, agenda unificada e CRUD de eventos.
- Fase 4: importação/sync do Google Calendar, gestão de fontes e write-through.
- Fase 5: `CALENDAR_EVENT` como gatilho idempotente de receitas.
- Fase 6: cobertura Rust elevada de 55,96% para **59,31% global** e de 61,40%
  para **65,20% core**. Ratchet do gate em 59% global e 64% core.
- Testes: 252 Rust, 89 Vitest e 40 Playwright.

## 2026-11-08 — 0.26.0: anexos, retry de trigger e simplificação

- Anexos genéricos via `POST/DELETE /api/attachments`: arquivos até 20 MB em
  `input/attachments/`, com caminho relativo seguro e remoção validada.
- Componente `PromptAttachments` adicionado a tarefas, rotinas, etapas de
  receita e contexto de execução; os caminhos entram no prompt/descrição.
- Trigger de calendário libera o claim quando `engine::start` falha, permitindo
  retry na próxima varredura em vez de perder a ocorrência.
- `CalendarPage` reduzido de 1.104 para 702 linhas (478 na primeira extração;
  cresceu com as novas visões); `EventModal` e `ImportGoogleModal` extraídos
  para `components/calendar/`.
- Cobertura Rust: **59,19% global** e **64,96% core**.
- Testes: 253 Rust, 89 Vitest e 40 Playwright naquele corte.

## 2026-11-08 — verificação proporcional e calendário por dia/semana

- Regra: sem release, `npm run check:changed` roda apenas fmt/Clippy/testes Rust
  e typecheck/Vitest quando os arquivos correspondentes mudam. Cobertura e
  Playwright ficam reservados ao gate de release.
- Calendário ganhou visões **Mês**, **Semana**, **7 dias** e **Dia**, com
  navegação proporcional à visão e agenda lateral apenas no mês.
- Receitas com trigger CRON sem `config` usam o padrão diário 08:00 no
  timezone do sistema (`iana-time-zone`, fallback UTC) no calendário e no
  runtime, em vez de desaparecerem.

## 2026-11-08 — calendário por visão, CRON vazio e limpeza

- Visões de calendário: mês, semana, 7 dias e dia; navegação adapta o passo e a
  agenda lateral fica no mês.
- Receita CRON com `config` vazio usa 08:00 diário no timezone do sistema
  (`Intl` no editor, `iana-time-zone` no backend) no calendário e no runtime; o
  editor inicializa o config ao escolher CRON.
- Trigger de calendário só consulta eventos quando existe receita habilitada
  com `CALENDAR_EVENT`, e usa a correspondência de calendário simplificada.
- Manutenção remove claims de trigger e eventos cancelados com mais de 90 dias.
- Cancelar tarefa/rotina/execução de receita remove os anexos enviados naquele
  formulário; o chat deixa explícito que “automático” usa o padrão do agente.
- Verificação proporcional via `npm run check:changed`; testes: 254 Rust e 91 Vitest.

## 2026-11-08 — cards consistentes em Kanban, rotinas e atenção

- Kanban: card extraído para `TaskCard` com altura fixa de 152px, título em até
  duas linhas, descrição em uma, metadados limitados a 3 + `+N` e rodapé fixo.
- Rotinas: card extraído para `RoutineCard` com 188px, cabeçalho com switch,
  metadados 4 + `+N`, prompt em duas linhas e ações secundárias no menu `⋯`.
- Atenção: linhas com 82px, detalhe em duas linhas e coluna de ações fixa.
- Inativas deixam de usar opacidade global; usam superfície mais escura + badge.
- Playwright: novo `cards.spec.ts` valida altura, overflow interno e menus em
  1440×900 e 1100×800. `ui-audit` e `opendesk` seguem verdes.

## 2026-11-08 — refatoração de duplicações e arquivos grandes

- MIME unificado em `media.rs`; `split_recipients` em `text.rs`; armazenamento
  base64 de anexos centralizado em `attachments::save_base64`, com `save_bytes`
  privado ao módulo.
- `basename` em `lib/path.ts`; `InfoRow`/`InfoTile` compartilhados; `STATUS_LABEL`
  movido para `lib/task-ui.ts`.
- `api/tests.rs` dividido em `tests/{calendar,tasks,schedules,studio,core,
  resources}.rs`, com helpers no módulo pai.
- `KanbanPage` caiu de 2.135 para 880 linhas com `TaskCard` e `task-modals`.
- `RecipesPage` caiu de 1.427 para 509 linhas com `recipe-editors`.
- `RoutinesPage` caiu de 1.267 para 424 linhas com `RoutineCard` e
  `RoutineFormModal`.
- `delivery.rs` dividido em `delivery/{mod,google,email,render,tests}.rs`.
- `studio_ops.rs` dividido com `studio_ops_install.rs` (709 linhas).
- Testes e Playwright direcionados seguem verdes.

## 2026-11-08 — segunda onda de refatoração

- `task-modals` dividido em `task-form-modal`, `task-details-modal` e
  `swarm-modal`; `task-modals.tsx` virou barrel de 3 linhas.
- `types.ts` dividido em `types/{core,schedules,runs,engine,studio,calendar,chat}.ts`,
  com `index.ts` como barrel de 7 linhas.
- `RunLogView` dividido em `RunLogView` (680 linhas) + `run-log-parts` (437).
- `delivery.rs` virou módulo com `mod/google/email/render/tests`.
- `studio_ops` separou instalação em `studio_ops_install.rs`.
- `api/tests` separado em domínios (`calendar`, `tasks`, `schedules`,
  `studio`, `resources`, `core`).
- MIME, destinatários, anexos base64, `basename`, `InfoRow` e `STATUS_LABEL`
  centralizados.

## 2026-11-08 — correções do code review

- CRON sem timezone usa `iana-time-zone` no backend e `Intl` no editor, em vez
  de `America/Sao_Paulo` fixo (ADR 0017); o fallback de expressão continua
  `0 8 * * *`.
- Linhas de atenção mantêm 82px, mas conteúdo e ações usam filhos roláveis
  (`min-h-0 overflow-y-auto`), removendo o clipping do ADR 0021.
- `/api/attachments`, `/api/capture` e mensagens de chat aceitam corpo de até
  32 MB, então arquivos de 20 MB em base64 passam pelo limite real; `save_bytes`
  ficou privado e capturas voltaram a reportar “imagem inválida/maior que 20 MB”.
- `check:changed` usa `git diff HEAD`, cobrindo mudanças staged e unstaged.
- `types/index.ts` virou barrel; definições foram para `core`, `schedules` e
  `runs`. `InfoTile` local de Sobre virou `AboutMetric`, sem colisão com o
  componente compartilhado.
- Testes: 256 Rust, 91 Vitest e 42 casos Playwright (21 arquivos); Playwright
  direcionado de cards e auditoria de UI verde.

## 2026-11-08 — cron unificado e componentes extraídos

- `recipes::cron::CronTrigger` resolve expressão, timezone do sistema e próxima
  ocorrência em um só lugar; calendário e runtime deixam de repetir o switch de
  `cron`/`cron_expression`/`schedule`. Testes cobrem os três nomes de chave,
  config vazio, timezone explícito e trigger não-CRON.
- Calendário: pill e linha de agenda saíram para
  `components/calendar/calendar-item.tsx`; `CalendarPage` caiu de 702 para 611
  linhas e a célula do dia ganhou `aria-label` próprio, evitando que o nome
  acessível concatenasse todos os itens.
- Receitas: `TriggerEditor`/`CronFields` foram para
  `recipe-trigger-editor.tsx` e `LabeledField`/`inputClass` para
  `recipe-fields.tsx`; `recipe-editors.tsx` caiu de 935 para 646 linhas.
- Rotinas: o seletor de receita/agente fecha por `relatedTarget` em vez de
  `setTimeout` no blur, removendo o fechamento durante o clique.
- Playwright direcionado: calendário, cards, receitas, rotinas, auditoria de UI
  e `opendesk` verdes (18 testes). Suíte atual: 260 Rust e 91 Vitest.

## 2026-11-08 — interface única para itens do Studio

- Novo `config_sync::studio_item` com `StudioKind`, `StudioItem` e
  `ItemSnapshot`: `exists`, `path`, `copy_to`, `delete`, `set_enabled` e
  `list` escondem o layout de agent, skill, command e MCP.
- `api/studio_ops.rs` deixou de repetir switches de kind em duplicate, copy,
  move, backup, restore, delete, toggle e effective; as operações HTTP agora
  montam um `StudioItem` e chamam a interface.
- `StudioItem::list` normaliza id, path, disabled e valor para os quatro kinds,
  então o merge de config efetiva usa um único laço.
- Testes novos: mover agente, conferir o merge efetivo e alternar MCP.
- Verificação: 261 testes Rust, 91 Vitest, Clippy limpo e 19 Playwright
  direcionados (`opendesk` + pacotes) verdes.

## 2026-11-08 — release 0.27.0

- Gate completo (typecheck, Vitest com cobertura, Clippy, testes Rust, build,
  cobertura Rust e Playwright inteiro) passou.
- Cobertura: **60,02% global** e **66,12% core** no Rust; Vitest com **97,54%
  de linhas**.
- Testes: **264 Rust**, **91 Vitest** e **41 Playwright**.
- Versão 0.27.0 publicada com artefatos locais, tag e GitHub Release.

## 2026-11-08 — release 0.28.0

- Fechamento de comboboxes centralizado em `shouldCloseOnBlur`, sem timers de
  blur em agente, modelo e receita.
- Instalação do Studio passou a usar `StudioKind`; testes novos cobrem CRUD,
  config e round-trip de `opencode.jsonc`.
- Busca global não sobrescreve mais o hash depois da navegação para um resultado.
- Gate completo passou: **60,23% global** e **66,37% core** no Rust, **97,55%**
  de linhas no Vitest, **265 Rust**, **93 Vitest** e **41 Playwright**.
- Versão 0.28.0 publicada com artefatos locais, tag e GitHub Release.

## 2026-11-09 — settlement, content installation e delivery

- `queue_scheduler::settlement` extraiu a liquidação de run do `execute_job`:
  status de fila/rotina, permissões, fechamento do run, task, eventos e
  classificação de erro. Testes cobrem plano/build, falha com bloqueio e os
  códigos de erro.
- `config_sync::content_installation` centraliza parsing e escrita de agent,
  skill, command e MCP; `install_apply` agora só monta `ContentDraft`.
- `delivery` ganhou `DeliveryChannel` e adapters explícitos para SMTP e Signal;
  retry e nomes de canal deixaram de ser strings soltas. O enum agora também
  cobre GMAIL, GCALENDAR e GTASKS do schema, marcando canais legados sem adapter
  como `SKIPPED` em vez de deixá-los presos em `FAILED`.
- Contratos: payloads Google (Calendar, Gmail raw, Docs) e autenticação/erros do
  OpenCode V2 cobertos.
- `StudioPaths::has_studio_files` evita varrer raízes ancestrais sem `.opencode`
  no merge efetivo do Studio.
- Cobertura: **61,78% global** e **68,07% core**; ratchet local elevado para
  **61% / 67%**. A meta de 80% segue como trilha incremental.
- O E2E de receita deixou de depender do toast e passou a aguardar o estado
  persistido, eliminando flake sob carga.
- Testes: **282 Rust**, **93 Vitest** e **41 Playwright**.

## 2026-11-09 — detalhes premium de navegação e feedback

- Erros de render agora ficam isolados na rota: o shell continua navegável e o
  boundary reseta ao trocar de rota, em vez de derrubar o app inteiro.
- A navegação volta ao topo do `main` ao trocar de página; mudanças de query não
  provocam salto durante a digitação.
- O título da janela passa a refletir a página e o workspace ativo.
- Movimento reduzido desativa spinners, skeletons, barras de loading e pulsos.
- `RunsPage` usa o mesmo `LoadingState` do restante do produto.
- Calendário lembra a última visão (Mês, Semana, 7 dias ou Dia) por workspace.
- Botões somente de ícone receberam nome acessível em Arquivos, Artefatos, Chat,
  Captura, Studio, integrações e editor de receitas.
- Empty states do Dashboard agora têm CTA para Studio e criação de tarefa;
  históricos e prévias usam `LoadingState` em vez de texto solto.
- Testes: **282 Rust**, **95 Vitest**; Playwright direcionado de calendário,
  auditoria de UI e fluxo principal verde (19 testes).

## 2026-11-09 — pacotes, catálogo externo e criação por objetivo

- Página Pacotes reorganizada em três camadas: **Workspaces prontos**,
  **Descobrir capacidades** e **Adicionar capacidades**.
- Novo catálogo externo curado com 18 fontes para skills, agentes, comandos,
  MCPs, workflows e prompts: skills.sh, officialskills.sh, Linkly, ClawHub,
  GitHub Topics, FlowGPT, PromptBase, Smithery, Glama, Anthropic MCP, PulseMCP,
  Awesome MCP, mcp.so, Composio, Docker MCP, Cursor Directory, Dify e n8n.
- Cada fonte tem foco, forma de consumo, tags e um atalho para criar o workspace
  recomendado quando há template adequado.
- Templates mostram quantidade de pacotes, pastas e composição resumida.
- Quando o onboarding está visível, a lista de workspaces prontos não duplica os
  mesmos cards; ela reaparece após dispensar o onboarding ou instalar um pacote.
- Testes: **97 Vitest** e E2E novo de filtro MCP + criação de workspace a partir
  de um recurso externo.

## 2026-11-09 — release 0.29.0

- Gate completo passou com **282 Rust**, **97 Vitest** e **42 Playwright**.
- Cobertura Rust: **61,62% global** e **67,82% core**.
- Versão 0.29.0 publicada com artefatos locais, tag e GitHub Release.

## 2026-11-09 — correção de REAL, boot resiliente e workspace unificado

- Corrigido o erro intermitente `f64 (REAL) vs INTEGER` no SQLite: somas de
  `estimated_cost_usd` agora usam `CAST(... AS REAL)` no budget, dashboard e
  custo por modelo. Isso acontecia quando todos os custos eram zero.
- O boot não depende mais do dashboard: daemon e workspaces são publicados
  primeiro, com `Promise.allSettled`; falha do dashboard não esconde a lista de
  workspaces.
- O diálogo do Topbar agora unifica **Em branco**, **Template** e **Existente**.
  Templates com pacotes/pastas podem ser criados sem passar pela página Pacotes.
- `RunExecution` extraiu `plan_run` e `record_artifacts` do `execute_job`, com
  teste do plano e da proveniência; `queue_manager` caiu de 941 para 786 linhas.
- `TaskState` centraliza running, aprovação, bloqueio, falha e terminal; cards,
  detalhes, formulário e notificações usam a mesma projeção.
- Propostas de agent/skill/command usam `StudioItem` para remoção.
- `PacksPage` caiu de 897 para 569 linhas com `PackInstallModal`,
  `WorkspaceTemplateModal` e tipos compartilhados em `pack-types`.
- A agenda do calendário foi separada em `calendar_agenda.rs`; `calendar.rs`
  caiu de 768 para 522 linhas e a coleta de eventos/rotinas/receitas/runs/fila
  ficou isolada.
- `record_artifacts` ganhou teste de deduplicação entre diffs e writes.
- O catálogo externo só mostra “Usar template recomendado” quando o template
  existe no backend, eliminando lookup silencioso.
- Testes: **286 Rust**, **101 Vitest**; Playwright direcionado de workspace e
  pacotes verde (6 testes).

## 2026-11-09 — catálogo por formatos operacionais

- Catálogo reorganizado em 8 categorias e **29 packs**: Base, Meta, Operações,
  Finanças, GTM, Conteúdo, Inteligência e Vida.
- Os 7 workspaces prontos agora refletem os formatos operacionais:
  `operacoes-backoffice`, `financas-fiscal`, `gtm-clientes`, `conteudo-midia`,
  `inteligencia-pesquisa`, `fabrica-capacidades` e `vida-casa`.
- Packs renomeados para a linguagem operacional: `vendas-pipeline`,
  `pos-vendas-cs`, `suporte-atendimento`, `lancamento-gtm`, `youtube-pipeline`,
  `blog-seo`, `wiki-segundo-cerebro`, `financas-casa`, `saude-treino` e
  `viagens-planner`.
- Novos packs: `sop-builder`, `compras-orcamentos`, `auditoria-financeira` e
  `agent-evaluator`, com agentes, skills, comandos, receitas e rotinas.
- Templates antigos foram substituídos pelas pastas operacionais propostas;
  o catálogo foi validado contra os diretórios e arquivos reais.
- Testes de starter packs, typecheck, Vitest e E2E de pacotes/workspace verdes.

## 2026-11-09 — pacotes por abas, itens individuais e novas verticais

- Página Pacotes ganhou abas **Visão geral**, **Workspaces**, **Pacotes** e
  **Capacidades externas**, com busca global nos três grupos.
- Cada pacote agora lista agentes, skills, comandos e receitas com detalhe
  expansível e instalação individual, sem marcar o pacote inteiro como instalado.
- Modal de workspace mostra composição de packs, pastas e README do template.
- Catálogo cresceu para **46 packs** e **11 workspaces**.
- Novas verticais: **Performance & Digital Commerce**, **Oratória, Relações &
  Ministério**, **Vida Pessoal, Família & Patrimônio** e **Educação, Pesquisa &
  Engenharia de Documentos**.
- 17 novos packs, incluindo dropshipping-radar, ecommerce-ops, afiliados-growth,
  ad-creative-studio, copywriting-direct-response, formula-lancamento,
  estudos-escrituras, discursos-religiosos, preparacao-discursos,
  negociacao-tatica, mente-clara-journaling, compras-dispensa,
  orcamento-domestico-avancado, colecionador-vault, escola-tutor-reverso,
  wiki-karpathy-pro e document-cloner-engine.
- Testes de starter packs: 15 verdes; E2E de pacotes: 5 verdes.

## 2026-11-09 — ação por workspace, menu, autocomplete e timezone

- Botões primários e switches ligados usam `--workspace-accent`; “Nova tarefa”,
  “Reiniciar”, criação, execução e toggles de rotinas/receitas deixam de ser
  azuis fixos.
- Menu de ações da rotina fecha ao clicar fora ou pressionar Escape.
- Prompt de etapa ganhou autocomplete com `@agente`, `/comando` e `#skill`, com
  dica visível e navegação por teclado.
- Timezone virou dropdown pesquisável por offset (`UTC-03:00 America/Sao_Paulo`),
  com o fuso atual marcado; aplicado em rotinas, receitas e configurações.
- Trigger CRON de receita agora usa frequência, horário, dia da semana/mês e
  timezone; expressão crua só aparece em “Personalizado (avançado)”.
- Testes: **104 Vitest**; E2E direcionado de workspace, rotinas e receitas verde.

## 2026-11-09 — loop de aprendizado leve

- Novo módulo `learning` extrai lições determinísticas de runs concluídos, sem
  embeddings, worker próprio ou dependências novas.
- Falhas são agrupadas por código classificado + agente + modelo + mensagem
  normalizada; sucessos são opcionais e agrupam agente + modelo.
- Captura idempotente por `learning_run_captures`: cada run é observado uma vez,
  então varreduras manuais não inflam ocorrências.
- Promover uma lição cria uma proposta `SKILL` no fluxo HITL existente; o arquivo
  só é gravado em `.opencode/skills` após aprovação humana.
- Memorizar grava um bloco curto e marcado em `.opencode/MEMORY.md` com escrita
  atômica, backup e marca idempotente por fingerprint.
- Evidência guarda apenas erro truncado, agente, modelo, duração e custo — nunca
  prompt completo ou segredo.
- Corrigido `db::runs`: as consultas de run agora selecionam código, mensagem e
  dica de erro; antes os campos voltavam vazios para a API e para o aprendizado.
- Corrigido o índice de receitas: a chave virou `(workspace_id, id)`. O mesmo
  pacote em dois workspaces não move mais receitas entre eles nem depende da
  ordem do sync.
- Reconciliação de receitas serializada por workspace, impedindo que o watcher
  apague linhas recém-instaladas por um pacote.
- `schedules.recipe_id` perdeu a FK de coluna única, incompatível com a nova
  chave composta; o vínculo continua validado pela rotina e pelo workspace.
- Sync de fontes de receita ficou resiliente a receitas removidas no meio do
  caminho: registra aviso, reconcilia o workspace e continua os demais.
- API: `GET /api/learning`, `PUT /api/learning/settings`,
  `POST /api/learning/scan`, `POST /api/learning/lessons/{id}/dismiss`,
  `POST /api/learning/lessons/{id}/promote` e
  `POST /api/learning/lessons/{id}/remember`.
- Nova página **Aprendizado** em Governança: estatísticas de 30 dias, filtros,
  captura ligável/desligável, mínimo de ocorrências, promoção automática opcional
  e aprovação/rejeição de propostas.
- ADRs 0034 (aprendizado) e 0035 (chave de receita por workspace) documentam as
  decisões.
- Testes: **296 Rust**, **109 Vitest** e **45 Playwright**.

## 2026-11-09 — fila, Kanban, dedupe, Studio e HITL

- Badge da fila conta apenas `QUEUED`/`RUNNING` do workspace ativo e abre a aba
  de fila; antes somava histórico de todos os status e podia mostrar jobs de
  outro workspace.
- Tarefas ganharam `origin`, `parent_task_id` e `dedupe_key`. Kanban separa
  **Trabalho** de **Automações**; o Attention Card ignora automações.
- Execuções de receita, I/O bus e captura continuam visíveis na aba de
  automações, nas receitas e em Execuções, com badge de origem.
- Deduplicação de tarefas e jobs com fingerprint SHA-256, índice único parcial,
  reutilização de tarefa ativa e carência configurável (padrão 5 min).
- Nova aba Configurações → Fila para ativar/desativar e ajustar a carência.
- Receitas e I/O bus respeitam tarefas deduplicadas e não criam runs/duplicatas.
- Studio: eventos de filesystem atualizam a lista, mas nunca sobrescrevem o
  rascunho; o texto digitado para de desaparecer.
- Skill nova gera slug automático a partir do nome e o botão passa a se chamar
  “Nova skill”.
- HITL persiste título, pergunta renderizada e contexto; o card mostra a
  pergunta, próximas consequências e ações por tipo.
- Aprovação de plano mostra trecho do plano; permissões explicam o efeito de
  cada decisão.
- ADRs 0036 (origem/dedupe) e 0037 (HITL) documentam as decisões.
- Testes: **303 Rust**, **116 Vitest** e **49 Playwright**.

## 2026-11-09 — governança de agentes

- Nova página **Agentes & Governança** com organograma, cargos, times, gestores
  e escopo global/workspace.
- Perfis operacionais em `agent_profiles` mantêm o arquivo do agente como SSOT;
  cargo, time, gestor, budget, quota e heartbeat ficam no SQLite.
- Budget mensal por agente derivado de `task_runs`, com bloqueio de dispatch e
  mensagem acionável quando esgotado.
- Quota de concorrência aplicada em `next_queued`; sem perfil, vale o limite do
  grupo.
- Metas em `agent_goals` com métrica, alvo, progresso, prazo e status.
- Delegações em `agent_delegations`: pedido PENDING, aprovação HITL cria tarefa
  `DELEGATION`, atribui ao destino e tenta despachar.
- Heartbeat opt-in com loop de 60s, intervalo configurável, check-in manual e
  tarefas `ROUTINE`; slot reservado antes da criação para evitar duplo disparo.
- Performance de 30 dias por agente: runs, sucesso, falhas, custo, duração e
  tokens.
- ADR 0038 documenta a decisão.
- Testes: **307 Rust**, **121 Vitest** e **51 Playwright**.

## 2026-11-09 — CLI headless e export de sessão

- O binário ganhou subcomandos `status`, `workspaces list`, `tasks
  list/create/run`, `runs list/show`, `queue list` e `export`.
- CLI usa a API local existente com Bearer token e `api.json`; não abre SQLite
  nem duplica execução.
- Saída humana compacta ou `--json`, com flags de workspace, agente, modelo,
  modo, prioridade, tags e limite.
- Export por run em Markdown, HTML autocontido e JSON, com status, agente,
  modelo, tokens, custo, resultado, passos, raciocínio, ferramentas e artefatos.
- Endpoint `GET /api/runs/{id}/export` e botões MD/HTML/JSON na página de
  execução.
- ADR 0039 documenta a decisão.
- Testes: **312 Rust**, **122 Vitest** e **51 Playwright**.

## 2026-11-09 — integração com OpenCode Browse

- Nova integração de primeira classe com o OpenCode Browse (MCP + extensão
  Chromium) para tarefas que exigem navegar na internet.
- Aba **Integrações → Browser** com status de binário, MCP, skill, comando,
  ativação/desativação por workspace/global e link de instalação.
- Tarefas com a tag `needs-browser` (ou URL/palavras-chave) recebem o MCP
  `browse`, a skill `browse-web` e instruções de navegação no prompt.
- Provisionamento idempotente no dispatch; sem reescrever config a cada run.
- Login, 2FA, pagamento e captcha via handoff; OpenDesk não guarda credenciais.
- Kanban exibe badge **web**; catálogo de MCPs e catálogo externo incluem o
  Browse.
- ADR 0040 documenta a decisão.
- Testes: **316 Rust**, **124 Vitest** e **52 Playwright**.

## 2026-11-09 — backup completo do workspace

- Novo módulo `backup` cria ZIP do workspace com manifesto, contagem, bytes e
  SHA-256.
- Exclui `.git`, `node_modules`, `target`, `dist`, caches e ambientes virtuais.
- Restauração cria um novo workspace, exige destino vazio/inexistente e nunca
  sobrescreve o atual.
- Proteções: zip-slip, symlinks, caminhos absolutos, limite de 2 GiB e 100 mil
  arquivos; hash verificado antes de extrair.
- API `/api/backups` com list/create/download/restore/delete.
- Nova aba **Configurações → Backups**.
- CLI `opendesk backups list/create/restore/download/delete`.
- ADR 0041 documenta a decisão.
- Testes: **319 Rust**, **124 Vitest** e **53 Playwright**.

## 2026-11-09 — observabilidade e contract tests

- Nova página **Observabilidade** com janelas de 7/30/90 dias.
- Métricas derivadas de `task_runs` e `execution_queue`: sucesso, custo,
  tokens, p50/p95/média de duração e espera na fila, falhas por código, série
  diária e performance por agente.
- Endpoint `GET /api/metrics`.
- Harness opt-in `npm run contract:opencode` valida OpenCode V2 real (health,
  meta, daemon, agentes, modelos e Studio).
- Threat model documentado em `docs/security/threat-model.md`.
- ADR 0042 documenta as métricas derivadas.
- Testes: **322 Rust**, **127 Vitest** e **54 Playwright**.

## 2026-11-09 — rate limit e auditoria da API local

- Rate limit em memória por classe (leitura 900/min, escrita 300/min,
  sensível 120/min), com 429 + `Retry-After`.
- Limites configuráveis por variáveis `OPENDESK_RATE_LIMIT_*`.
- Tabela `api_audit` registra mutações e leituras sensíveis sem corpo, prompt,
  header ou credencial.
- Retenção de 90 dias no maintenance.
- Nova aba **Configurações → Auditoria** com filtros e limites vigentes.
- Endpoint `GET /api/audit`.
- ADR 0043 documenta a decisão.
- Testes: **326 Rust**, **127 Vitest** e **55 Playwright**.

## 2026-11-09 — refatoração de governança, atenção e dedupe

- `GovernancePage` reduzida de 1322 para 407 linhas, com componentes em
  `src/components/governance/` (OrgChart, Goals, Delegations, Performance,
  ProfileModal, GoalModal, StatCard, SectionTitle e tipos).
- `AttentionCard` reduzido de 867 para 518 linhas, com `HitlCard`, `Section`,
  `Row`/`DismissButton` e tipos em `src/components/dashboard/attention/`.
- Deduplicação extraída de `db/tasks.rs` para `db/task_dedupe.rs`; arquivo caiu
  de 968 para 853 linhas.
- Sem mudanças de contrato ou comportamento; testes direcionados e completos
  verdes.
- Testes: **326 Rust**, **127 Vitest** e **55 Playwright**.

## 2026-11-09 — refatoração de backend e consolidação

- Handlers de sistema/health/meta/dashboard/workspaces/settings e middlewares
  movidos de `api/mod.rs` para `api/core.rs`; roteador caiu de 863 para 538
  linhas.
- Testes de starter packs extraídos para `starter_packs_tests.rs`;
  `starter_packs.rs` caiu de 1939 para 1476 linhas.
- Consolidação: `GovernancePage` 1322→407, `AttentionCard` 867→518,
  `db/tasks.rs` 968→853 e dedupe em `db/task_dedupe.rs`.
- Sem mudanças de contrato/comportamento.
- Testes: **326 Rust**, **127 Vitest** e **55 Playwright**.

## 2026-11-09 — release 0.31.0

- Gate completo passou com **286 Rust**, **101 Vitest** e **43 Playwright**.
- Cobertura Rust: **62,09% global** e **68,60% core**.
- Versão 0.31.0 publicada com artefatos locais, tag e GitHub Release.

## 2026-11-09 — release 0.30.0

- Gate completo passou com **285 Rust**, **101 Vitest** e **43 Playwright**.
- Cobertura Rust: **61,74% global** e **68,04% core**.
- Versão 0.30.0 publicada com artefatos locais, tag e GitHub Release.

## 2026-11-10 — suíte funcional de features

- Manifesto `tests/e2e/features.ts` com 64 capacidades e ao menos um teste de
  UI por capacidade.
- Endpoint `POST /api/test/seed` (somente com `OPENDESK_E2E=1`) para semear
  tarefas, runs, artefatos, HITL, propostas e permissões.
- Fixture de empresa realista em `tests/e2e/fixtures/company.ts` e SMTP local
  falso em `tests/e2e/fixtures/smtp.ts`; contratos externos reais continuam
  opt-in.
- `npm run test:features` roda a suíte completa sob demanda e
  `scripts/feature-coverage.mjs` exige ≥95% do manifesto. A suíte fica fora do
  gate normal e não roda a cada mudança.
- Cobertura funcional: **64/64 (100%)** com **65 testes Playwright**.
- Correção real: promoção de lição do Aprendizado grava
  `requested_by = "learning"` por padrão.
- Testes: **326 Rust**, **127 Vitest** e **65 Playwright**.

## 2026-11-10 — release 0.42.0

- MCP `opendesk-control` (stdio + API local) para tarefas, execuções, rotinas,
  receitas, fila, artefatos e leitura da atenção; HITL, permissões e propostas
  continuam somente leitura.
- `opendesk-config` e `opendesk-control` são registrados em todo workspace na
  criação e no boot dos existentes.
- Pacote base `controle-opendesk` com skill e comandos, instalado
  automaticamente em novos workspaces (manual, template e fábrica) sem
  sobrescrever arquivos.
- Rotinas e receitas novas nascem desligadas: `schedules::create` grava
  `is_active=false`; `save_yaml` grava `enabled=false` em arquivo novo; pacotes
  normalizam receitas; UI reflete o padrão.
- ADR 0044 documenta a exceção ao ADR 0032.
- Testes: **335 Rust**, **128 Vitest** e **67 Playwright**.

## 2026-11-10 — release 0.42.1

- Corrigido o `Modal` compartilhado que refocava o botão de fechar a cada
  tecla, impedindo digitar nos formulários e fechando diálogos ao apertar
  espaço/Enter.
- Efeito de foco agora depende só de `open`, com `onClose` via ref; foco
  inicial no primeiro campo, timer cancelado no cleanup e Tab/Shift+Tab
  contidos no diálogo.
- Regressões cobertas por Vitest (input controlado + `onClose` instável) e
  Playwright (`pressSequentially` com espaços).
- E2E ganhou limites de rate limit altos no ambiente de teste.
- Testes: **335 Rust**, **130 Vitest** e **68 Playwright**; cobertura funcional
  **69/69 (100%)**.

## 2026-11-11 — 0.43.0 em preparação

- Updates passam a ser assinados com Ed25519: `latest.json.sig` obrigatório,
  chave pública embutida e chave privada offline; `release.sh` assina e
  verifica; ADR 0045.
- MCP `opendesk-control`: rotinas aceitam todos os gatilhos (folder, webhook,
  IMAP, SQL poll, Signal, rsync) via `trigger_config`; CRON continua exigindo
  recorrência.
- MCP ganhou mensagens de tarefa, pin e rollback de artefato (23 ferramentas).
- Ativar rotina/receita na UI exige confirmação explícita e pode ser cancelado.
- Testes de assinatura, gatilhos não-CRON, autoFocus do modal e confirmação de
  ativação adicionados.
- ESLint (hooks + TypeScript) entrou no `check:changed` e no gate; 6 efeitos com
  dependências faltantes e um trecho de código morto foram corrigidos.
- Testes de digitação real para workspace, evento e rotina, além da tarefa.
- `npm audit --omit=dev` limpo via override de `dompurify` 3.4.17.
- Loop JSON-RPC compartilhado em `mcp_server.rs`; `mcp_control` dividido em
  `tools`, `payload` e `util` (1250 → 569 linhas no módulo principal).
- `api/tasks.rs` 953 → 742 com `task_dispatch.rs`; `starter_packs.rs`
  1502 → 1317 com `plan.rs`.
- Dashboard expõe rotinas ativas e receitas habilitadas; doctor reporta
  cobertura de MCP e pacote base por workspace.
- Auditoria axe com gate para violações críticas e sérias nas rotas principais
  e modal de tarefa.
- Browse ganhou allowlist por tag/UI; alto risco exige allowlist e o prompt
  instrui `session_allowlist`.
- Contrato Google opt-in (`npm run contract:google`) cobre status, MCP e busca
  read-only no Drive.
- Contraste global corrigido (zinc-500 mais claro, zinc-600/700 removidos,
  botão de acento mais escuro); Kanban focável por teclado; nome acessível do
  workspace inclui o texto visível.

## 2026-11-11 — release 0.43.1

- Axe ampliado para 17 rotas principais e 4 modais (tarefa, workspace, evento
  e rotina); contraste do dia selecionado no calendário corrigido.
- Testes de `split_model` e `job_dto` no queue manager.
- Novos testes de integração do cron: `run_now` enfileira, fila pausa/retoma,
  prioridade, cancelamento e retry; tick cobre `SKIPPED_OFFLINE` e
  `SKIPPED_MISFIRE`.

## 2026-11-11 — release 0.44.0

- Timeline de execução passou a renderizar todos os eventos do OpenCode, com
  payload genérico para tipos desconhecidos.
- Cards expansíveis por clique no cabeçalho/conteúdo, com animação e ferramentas
  mostrando entrada/saída reais.
- Modais centralizados com largura/altura mínimas e command palette ampliada.

## 2026-11-11 — release 0.44.1

- Preview de execução (`LogDrawer`) e preview de artefato deixaram de ser
  drawers laterais e agora são overlays centralizados, grandes e fecháveis com
  Esc/backdrop.

## 2026-11-11 — release 0.45.0

- Atalhos globais ignoram `input`, `textarea`, `select` e `contenteditable`;
  `?` e letras voltam a ser texto normal enquanto o usuário digita
  (`src/lib/keyboard.ts`, guard no `AppShell`).
- Roteador contextual por política: cards condicionais, teste de roteamento
  sem dispatch, edição de regras IF-THEN e `RouterDecision.origin`.
- Modelo padrão lido do `opencode.jsonc` (workspace → global) em dispatch,
  cron, roteador e fallback; fim do `opencode-go/*` fixo que causava 402.
- `POST /api/models/test` abre sessão real no OpenCode e classifica
  `engine_offline`, `provider_quota`, `auth_failed`, `model_not_found`,
  `timeout` e `provider_error`; botão “Testar modelo” em todos os
  `ModelCombobox`.
- `provider.quota`/saldo insuficiente classificado no settlement com cooldown
  de 60 min e fallback preferindo o modelo padrão.
- Filtros do Studio com nome acessível (axe verde).
- Testes: 352 Rust, 134 Vitest, 76 Playwright; 74/74 features (100%).

## 2026-11-11 — release 0.46.0

- Modelo padrão e modelo Manual do roteador passaram a ser configuração do
  OpenDesk por workspace (`router_models:<workspace_id>`), não o `model` do
  `opencode.jsonc`. `task_dispatch`, `cron_engine` e fallback da fila usam esses
  valores; o Manual aceita override da tarefa.
- `POST /api/router/models` e os novos campos de `/api/router/config` expõem os
  modelos; `/api/router/test` aceita política e overrides e informa
  `engine_online`, `model_available` e a contagem de modelos.
- Tela do roteador ganhou a tabela de teste das cinco modalidades (zero tokens),
  `Testar todas`, resultado por política e `Testar modelo` no modelo resolvido.
- `POST /api/models/test` virou geração mínima via
  `/api/experimental/generate/text` (`Output only: OK`), sem sessão, histórico
  ou ferramentas. Classifica 402/429/modelo inexistente/engine offline e
  registra cooldown de quota.
- `npm run verify:model` valida a conexão real com `deepseek/deepseek-flash`
  (resposta OK, 81 modelos, `model_available: true`).
- Testes: 356 Rust, 134 Vitest, 76 Playwright; 74/74 features (100%).

## 2026-11-12 — onda 1: estado único, notificações e feedback

- `Task` ganhou `workflow_state` e `workflow_reason` derivados no backend a
  partir de status, fase, job ativo e último run. A UI deixou de inferir
  “executando” apenas por `IN_PROGRESS`; tarefa movida manualmente para
  In progress sem job/run aparece como atenção com motivo
  `in_progress_without_run`.
- Novo `WorkflowStatusBadge`/`workflowMeta` compartilhado no Kanban (board e
  lista) e no detalhe da tarefa, com ícone, cor, texto e tooltip.
- `WorkingIndicator` passou a usar uma única engrenagem animada; Dashboard
  mostra o indicador no KPI da fila e nos runs ativos.
- Preferências de notificação separadas: fim de execução, falhas/bloqueios,
  interações, artefatos e rotinas puladas. Padrão mínimo com artefatos e
  rotinas puladas desligados; movimentação de Kanban não gera mais toast.
- Detalhe da tarefa mostra tokens de entrada, saída, total e cache; cada run
  em “Últimas execuções” mostra entrada/saída/total. Cabeçalho do run mostra
  `↑ entrada · ↓ saída · Σ total`.
- Testes: workflow no Rust e Vitest, notificações mínimas e E2E do falso
  “executando”.

## 2026-11-12 — onda 2: Kanban por workflow, autostart e fork revisável

- Kanban passou para 5 colunas derivadas do `workflow_state`: Fila, Em
  execução, Atenção, Concluído e Cancelado. `IN_REVIEW` e `BLOCKED` ficam em
  Atenção com motivo claro; `data-column` acompanha os novos grupos.
- Criar tarefa auto-inicia quando o motor está online, a fila não está pausada
  e a tarefa não está em espera. Um reconciliador no tick do cron inicia até 3
  tarefas READY por ciclo quando o motor volta. Nova opção
  `tasks_autostart` (padrão ligado) em Configurações → Fila.
- “Em espera” virou um estado real (`held`) com endpoint
  `POST /api/tasks/:id/hold`; retomar limpa o hold e tenta auto-iniciar.
- Arrastar para Em execução dispara um job real; se o motor estiver offline a
  transição falha e o card volta para Fila. Mover para Fila limpa hold e
  auto-inicia; Concluído/Cancelado cancelam job/run; Atenção abre um modal de
  motivo do bloqueio.
- Título de tarefa é opcional; quando vazio, deriva da primeira linha da
  descrição (até 80 caracteres). Vale para UI, API e MCP `task_create`.
- O card de tarefa usa descrição fixa de 3 linhas, altura consistente, um único
  status de workflow, tooltips nos metadados e link “arquivos” para os
  entregáveis. Saíram os chips duplicados de último run/fase.
- Fork no detalhe e no run abre a tarefa em modal de edição pré-preenchido;
  só cria depois da confirmação. A API de fork aceita overrides.
- Rodapé do detalhe da tarefa ficou fixo no fundo e centralizado, com “Ver
  arquivos” e ações agrupadas.
- Testes: 360 Rust, 138 Vitest, 77 Playwright e 74/74 features (100%). Inclui
  hold/título/transição no Rust e E2E de hold e falso running.

## 2026-11-12 — onda 3: continuidade de execução, timeline e HITL inline

- Follow-ups agora gravam `parent_run_id` e a página de execução usa
  `GET /api/runs/{id}/thread` para mostrar todas as tentativas em uma única
  linha do tempo. A lista de execuções agrupa por tarefa/thread e mostra
  “N tentativas”.
- `RunTimeline` ganhou `attempts` e marcadores de tentativa por evento;
  o cabeçalho soma tokens/custo de todas as tentativas.
- A linha do tempo é cronológica, agrupa ferramentas e mensagens consecutivas,
  marca cada tentativa e o modo “Resumo” esconde seções vazias. Accordions
  ficaram em 200 ms.
- Barra da execução: ícones+texto sem quebra, tooltips, Pasta com fallback
  “Copiar caminho”, Explorador abre a pasta de output via `dir=1`, Memória
  grava bloco marcado em `MEMORY.md` com Desfazer, Fork abre modal pré-preenchido,
  Copiar copia a transcrição completa e Recarregar mantém spinner mínimo.
- Composer com textarea maior, botão Enviar quadrado, valores resolvidos de
  agente/modelo e continuação na mesma execução.
- HITL genérico: tabela `task_interactions`, endpoints de pergunta/resposta,
  cartão inline no detalhe da tarefa, na timeline e no Attention Card.
  Nova ferramenta MCP `ask_user` bloqueia a tarefa em Atenção até a resposta.
  Permissões do OpenCode usam o mesmo cartão inline.
- Testes: 361 Rust, 138 Vitest, 78 Playwright e 74/74 features (100%). Inclui
  interação HITL no Rust e Playwright, thread de execução, timeline e toolbar.

## 2026-11-12 — onda 4: calendário correlacionado, perdidas e dashboard ao vivo

- Calendário agora correlaciona cada ocorrência de rotina/receita com o run
  real. Executados abrem o run; não executados abrem a rotina/receita.
  Projeções futuras só aparecem para rotinas ativas e receitas habilitadas.
- Dias passados escondem ocorrências não executadas, exceto falha, bloqueio,
  cancelamento ou execução perdida pendente.
- Itens do calendário ganharam `execution_status`, `run_id`, `missed_id` e
  `executed`, com ícone/cor/legenda para sucesso, falha, execução, fila,
  agendado, cancelado e perdido.
- Nova tabela `missed_runs` e política `missed_run_policy` (Perguntar padrão,
  Executar automaticamente ou Ignorar) em Configurações → Fila.
- O cron registra ocorrências perdidas por app/engine offline ou atraso,
  agrupa até 5 por rotina e aplica a política. Attention Card permite executar
  ou ignorar individualmente ou em lote.
- Dashboard ganhou a faixa “Em execução agora” com engrenagem animada, tarefa,
  agente, modelo e tempo decorrido.
- Testes: 362 Rust, 138 Vitest, 78 Playwright e 74/74 features (100%).

## 2026-11-12 — onda 5: artefatos, pacotes e ciclo de aprendizado

- Deep link de artefato agora busca `GET /api/artifacts/{id}` diretamente,
  mesmo fora da lista/filtro atual, e abre a prévia com estado de carregamento
  e erro acionável.
- Novo `DiffView`/`parseUnifiedDiff`: diff unificado com cabeçalhos de hunk,
  números de linha antigos/novos, adicionadas em verde, removidas em vermelho,
  contexto neutro, contagem `+/-` e copiar diff. Usado na aba Diff dos artefatos.
- Pacotes ganharam filtro Todos/Instalados/Disponíveis; cards instalados têm
  check e tooltip. No modal, cada item mostra `instalado`, `modificado` ou
  `disponível`, com filtro por status e botão Reaplicar.
- O backend de pacotes calcula o status por workspace comparando os arquivos
  planejados com o disco; API de itens aceita `workspace_id`.
- Aprendizado explica Promover (“criar proposta de skill”) e Memorizar
  (“salvar em MEMORY.md”), com filtro “Memorizadas”, reabrir descartadas e
  remover da memória. A remoção apaga o bloco marcado no `MEMORY.md` e limpa
  `remembered_at`.
- Testes: 362 Rust, 156 Vitest, 78 Playwright e 74/74 features (100%).

## 2026-11-12 — onda 6 e release 0.47.0

- Auditoria transversal de rotinas e fila: tooltips nos metadados, IconLabel
  nos botões e indicador animado de execução no controlador da fila.
- Revisão de consistência das telas citadas nas ondas anteriores: status real,
  navegação para o objeto específico, feedback de ações, notificações mínimas,
  preview compartilhado, diff, filtros de pacotes e ciclo de aprendizado.
- Gate axe mantido em todas as rotas principais e modais-chave.
- Corrigidos flakes de seletor em calendário e atenção.
- Release 0.47.0 com notas em `docs/releases/0.47.0.md`.
- Testes: 362 Rust, 156 Vitest, 78 Playwright; 74/74 features (100%).

## 2026-11-12 — passe de interface: precisão operacional e DESIGN.md

- `PRODUCT.md`, `DESIGN.md` e ADR 0046 registram a verdade de produto, a
  escala tipográfica, os tokens de superfície, os controles e as regras de
  modais. Páginas deixam de usar overlines decorativas e adotam título de 24px,
  corpo de 13,5px e metadados de 12px.
- Shell: a topbar perdeu o caminho duplicado e os glifos `0● 0○`; fila agora
  fala “0 ativos · 0 na fila”. O botão “Novo workspace” continua ao lado do
  seletor, com nome acessível. Custo e fila ficam visíveis em telas largas e a
  busca continua disponível em 1100px. Reiniciar o engine pede confirmação
  quando há jobs ativos.
- Orçamento do dia passa a vir de `/api/budget/status` em vez do limite fixo de
  US$ 5; topbar e dashboard mostram o mesmo gasto e o mesmo limite.
- Sidebar: ícone distinto para Observabilidade, separadores de grupo no modo
  recolhido, preferência de recolhimento persistida e compactação vertical para
  manter todas as rotas acessíveis em 700px de altura. Em larguras menores que
  1200px a sidebar recolhe automaticamente.
- Filtros de Execuções, Artefatos, Arquivos e Kanban usam `FilterBar`,
  `FilterSearch` e `FilterSelect`: mesma altura, limpeza por botão/Esc e
  placeholders completos.
- Observabilidade: a série diária agora preenche os dias sem runs (o gráfico
  antes não desenhava barras) e os eixos não quebram linha; o selo de saúde
  deixou de exibir “GREEN” cru e o plural de runs foi corrigido.
- Modais ganharam `size` (`sm/md/lg/xl`), sem largura/altura mínima forçada;
  confirmações destrutivas focam “Cancelar” em vez do botão de exclusão, e
  `aria-describedby`/ids únicos melhoram leitores de tela.
- Command palette ganhou ação explícita “Buscar em todo o workspace”, separando
  a navegação rápida da busca completa; estados selecionados usam o acento do
  workspace em vez de azul fixo.
- Acessibilidade: fallback de foco visível para links e botões sem ring,
  `aria-label` em controles icônicos e switch compartilhado no TLS do SMTP.
- `docs/interface-proposal.md` registra a direção “instrumento de precisão” e
  as próximas fases (views salvas no Kanban, IA de Configurações, organograma
  real, faixa única de KPIs e busca unificada).
- Hierarquia: KPIs do Dashboard e da Governança passam a usar número de 28px,
  rótulo de 11px e dica de 12px; títulos de card em 15px e corpo em 13,5px.
- Kanban: as cinco raias cabem em 1440px (`xl:flex-1` com mínimo de 220px); o
  card de tarefa ganhou altura automática, título em duas linhas sem competir
  com o status, status na linha de metadados e rodapé de 32px. A coluna vazia
  explica “Nenhuma tarefa nesta etapa” em vez de só pedir para arrastar.
- Rotinas: cards com altura automática, descrição legível, ações de 28px e
  menu “Mais ações”; os três cards explicativos só aparecem quando ainda não há
  rotina configurada.
- Studio: busca e filtros em 36px, linhas com título de 14px, metadados de
  11,5px e ações “Editar” 32px + ações secundárias 28px.
- Chat: coluna de conversas em 320px, títulos e mensagens em 13,5px, busca de
  36px e o modelo automático sem o card duplicado; o segmentado agora cabe em
  uma linha.
- Calendário e atenção: item de calendário com borda de 1px (sem o “side-tab”
  que o detector marcava), status internos traduzidos; cartão de atenção com
  linhas de altura automática, texto de consequência legível e ações de 28px.
- Status de execução traduzidos (sucesso, falhou, executando, cancelada) em
  Execuções, Rotinas e Calendário; “CREATED” vira “criado” em Artefatos.
- Acessibilidade: corrigido `label-content-name-mismatch` nos botões de busca e
  reinício da topbar; axe segue limpo em todas as rotas e modais-chave.
- Verificação: `npm run gate:fast` (whitespace, fmt Rust, TypeScript, ESLint e
  158 Vitest), axe, `ui-audit` em 1440×900 e 1100×800 e a suíte Playwright
  completa (78 testes) verdes. Detector Impeccable sem achados. O gate rápido
  `npm run gate:fast` passa a ser o padrão do dia a dia; o gate completo fica
  sob demanda.

## 2026-11-12 — onda 7: governança compacta, dashboard em faixa e shell responsivo

- Governança trocou os cards aninhados por uma tabela compacta com hierarquia
  por indentação, colunas de escopo, execuções 30d, sucesso, budget e ações.
  Metas, delegações e performance continuam abaixo; o check-in e a edição
  seguem acessíveis por linha.
- O resumo da Governança virou uma faixa única com divisores, no mesmo padrão
  do Dashboard; os números usam fonte tabular e alinhamento consistente.
- O Dashboard trocou quatro cards iguais por uma faixa de status com divisores
  (gasto, execuções, cache, fila), reduzindo a altura e antecipando o conteúdo
  operacional. O Attention Card continua acima quando há decisão pendente.
- A sidebar agora volta a expandir quando a janela cresce acima de 1200px,
  desde que o recolhimento tenha sido automático; a preferência manual continua
  preservada.
- O Kanban ganhou setas de rolagem nas bordas quando o quadro não cabe
  (1100px), com estado calculado pelo scroll real e sem overflow da página.
- `docs/interface-proposal.md` mantém as próximas fases (views salvas no
  Kanban, IA curta de Configurações e busca unificada).
- Verificação: `npm run gate:fast` (158 Vitest), `governance-ui`, `cards`,
  `ui-audit` e `axe` verdes em 1440×900 e 1100×800. Sem release nesta onda.

## 2026-11-12 — onda 8: filtros do Kanban

- Agente e prioridade saíram da barra principal e foram para o popover
  **Filtros**, que também ganhou **Somente atenção** (aprovação, pergunta,
  permissão ou falha).
- Filtros ativos aparecem como chips removíveis; o antigo “limpar filtros”
  duplicado foi eliminado. A busca continua com limpeza própria.
- O popover fecha ao clicar fora e com Esc, tem contagem no botão e estado
  acessível (`aria-expanded`). O board continua com as setas de rolagem.
- Verificação: `npm run gate:fast` (158 Vitest), `kanban-origins`, `cards`,
  `ui-audit` e axe com o popover aberto. Sem release.

## 2026-11-12 — onda 9: consistência de dados e estados vazios

- Observabilidade alinhou colunas numéricas à direita, com fonte tabular e
  sucesso em badge alinhado; a leitura de performance por agente ficou
  consistente com as demais tabelas.
- Estados vazios dentro de cards (Execuções, agentes, telemetria, atividades,
  rotinas e governança) usam a variante compacta, reduzindo altura sem perder
  orientação.
- Verificação: `npm run gate:fast` (158 Vitest), `dashboard-data`,
  `observability-data`, `runs-detail-ui` e `governance-ui`. Sem release.

## 2026-11-12 — onda 10: navegação agrupada, Packs e release 0.49.0

- Configurações e Integrações agrupam abas com divisores visuais, mantendo
  todos os destinos e os testes por `role="tab"`.
- Pacotes padronizou abas de 32px, busca de 36px e rótulos de resultado.
- `SectionTabs` aceita `group` opcional e desenha o divisor apenas na troca de
  grupo; sem grupo, o comportamento anterior é preservado.
- Detector Impeccable sem achados e suíte Playwright completa verde (78).
- Release 0.49.0 com notas em `docs/releases/0.49.0.md`.

## 2026-11-12 — onda 11: HITL no Kanban, favoritos e visualizadores

- Kanban passou a carregar `/api/attention` junto com tarefas e fila. Cards com
  interação ou permissão pendente vão para **Atenção**, ganham o selo
  “aguardando você” e mostram um painel com o motivo real e a ação primária.
- “Responder” abre um modal focado com `InteractionCard`/`PermissionRequestCard`;
  ao resolver, o board recarrega e o card sai da atenção. Aprovação de plano
  continua direta no card. O card deixou de ser `role="button"`; título e ação
  “Abrir tarefa” são botões, eliminando controles interativos aninhados.
- Popovers de visibilidade e filtros usam `useDismissable`: fecham com clique
  fora e Esc, com teste E2E específico.
- Modais ganharam header sticky, `initialFocus="dialog"` para detalhes longos e
  foco sem scroll forçado. O visualizador de artefatos usa o `Modal`
  compartilhado; arquivos usam `FileViewer` embutido sem duplicar caminho/tamanho.
- Receitas e rotinas receberam hierarquia e estados mais legíveis; execuções
  mantêm status, custo e navegação direta.
- Pacotes ganharam favoritos globais, filtro **Favoritos** e a aba
  **Recomendados** com 11 skills curadas. A instalação reutiliza o preview/apply
  do Studio e pré-seleciona a skill pedida; Impeccable usa o instalador oficial.
- Verificação: `npm run gate:fast` (161 Vitest), suíte Playwright completa
  (84 testes), axe nas novas superfícies e detector Impeccable sem achados.
  Sem release.
- Execuções ganharam resumo de sucesso/falhas/em andamento no filtro, cabeçalho
  do detalhe com ID curto e status traduzido no topo do RunLog. O teste real do
  fluxo de instalação de uma skill recomendada (Ponytail) foi validado contra a
  URL raw, além do teste de UI com preview mockado.
- Busca global agrupa os resultados por tipo na aba **Todos**, com cabeçalho,
  ícone e contagem por grupo; a lista plana continua nas abas específicas.
- Rodapés de instalação de pacote/skill ficaram sticky para manter a ação
  visível em listas longas, e o formulário de rotina manteve o rodapé no fluxo
  para não cobrir campos no viewport compacto.
- Configurações trocou a barra horizontal de dez abas por um trilho vertical
  agrupado (Geral, Motor, Operação, Workspace, Sobre). O conteúdo ganhou largura
  e a navegação continua com os mesmos `role="tab"` e nomes, preservando os
  testes e deep links.
- Kanban ganhou **views salvas**: nome, origem, busca, agente, prioridade e
  “somente atenção” são persistidos em `localStorage`, com aplicar/remover no
  popover de filtros e teste E2E de persistência.
- Integrações adotou o mesmo trilho vertical agrupado de Configurações
  (Mensageria, I/O, Sistema), com `role="tab"` preservado e largura maior para
  formulários e cartões.

## 2026-11-12 — onda 12: navegação compartilhada, busca agrupada e release 0.50.0

- Kanban ganhou views nomeadas persistidas, aplicáveis pelo popover de filtros.
- Configurações e Integrações compartilham o trilho vertical `SectionNav`
  (grupos, `role="tab"`, deep links preservados).
- Busca global agrupa a aba Todos por tipo, com contagens; abas específicas
  continuam planas.
- Rodapés de instalação ficam visíveis em listas longas; o formulário de rotina
  evita sobreposição em viewports compactos.
- Execuções exibem resumo de sucesso/falhas/em andamento e status traduzido no
  RunLog.
- Release 0.50.0 com notas em `docs/releases/0.50.0.md`.
- Verificação: gate rápido, suíte Playwright completa, axe e detector limpos.

## 2026-11-13 — P1: gate completo, features, axe e goldens visuais

- Gate completo verde: cobertura de branches em 91,51%, statements 96,77%,
  functions 98,43% e lines 97,56%; clippy, testes Rust, build e Playwright.
- Suíte funcional completa: **74/74 features (100%)**.
- Axe estendido para **1024×768** e **1920×1080** em todas as rotas principais;
  corrigido contraste dos dias de outro mês no calendário.
- Novo harness de goldens visuais em `tests/visual/` para Dashboard, Kanban,
  Execuções, Pacotes/Recomendados, Configurações e Artefatos em 1024, 1440 e
  1920. Comandos: `npm run test:visual` e `npm run test:visual:update`.
- Testes unitários adicionais cobrem favoritos, views do Kanban, dismissable,
  lista de recomendadas e o store de UI; a cobertura voltou a passar do gate.
- Playwright completo: **87 testes**.

## 2026-11-13 — P2: fila, busca, HITL, modais, viewers e tipografia

- Kanban: a concorrência saiu da barra do board e ficou no controlador da fila,
  com rótulo e alvo maiores. A barra do board fica só com visão, origem, busca,
  filtros e visibilidade.
- Busca: a aba selecionada virou deep link (`?tab=`), preservada ao navegar e
  ao atualizar a query; as abas usam `role="tab"`/`aria-selected`.
- HITL: suporte a múltiplas interações/permissões por tarefa, modal com todas
  as pendências, teste de aprovação direta no card e teste de permissão negada.
  O seed de teste passou a aceitar `phase_status` e request ids completos.
- Modais: PackInstall, RoutineForm, RecipeRun e StudioInstall cobertos por axe;
  corrigidos controles aninhados, regiões scrolláveis sem foco e contraste na
  lista de receitas.
- Viewers: testes para binário, ZIP e arquivo grande; estado sem prévia e ação
  de abrir arquivo validados.
- Settings/Integrations: abaixo de 1024px o trilho vira um select agrupado,
  mantendo os mesmos destinos e testes em viewport compacto.
- Tipografia: codemod dos `text-[8px]`…`text-[13px]` para os valores computados
  e remoção do remap; nenhum tamanho funcional abaixo de 11px.
- Validação real de preview de Caveman e das skills do Matt Pocock.
- Goldens visuais regravados após o codemod tipográfico.
- Fechamento do P2: **176 Vitest**, **93 Playwright**, **74/74 features**,
  cobertura Rust e JS acima do gate, clippy, build, axe em 1024/1440/1920 e
  goldens visuais estáveis.
- Release 0.50.1 publica no binário as melhorias de P1/P2 (fila, busca, HITL,
  modais, viewers, navegação responsiva e tipografia), com notas em
  `docs/releases/0.50.1.md`.

## 2026-11-13 — refatoração segura sem release

- Status de execução e de receita centralizados em `src/lib/status.ts`
  (labels + tons); Kanban, Execuções, Receitas e Rotinas deixaram de manter
  mapas próprios. `task-ui` reexporta os labels antigos para não quebrar
  imports.
- Primitivas de formulário em `src/components/ui/field.tsx` (`Field`, `Input`,
  `Select`, `Textarea`, `FormSection`, `FormError`) usando as classes
  compartilhadas de `ui-classes`. Adotadas no workspace de Configurações, na
  regra do roteador, em metas e governança de agentes e na criação de
  workspace por template. `TimezoneSelect` e `PathInput` aceitam `id` para
  rótulo acessível.
- Kanban dividido em `kanban-columns.ts` (definição das colunas),
  `KanbanBoard.tsx` (board + DnD) e `KanbanListView.tsx` (tabela com seleção
  em lote); `KanbanPage` ficou com filtros, toolbar, fila e modais.
- Roteador dividido em `RouterPolicyPicker`, `RouterModelsCard`,
  `RouterPolicyTestCard`, `RouterCostCard`, `RouterBudgetCard`,
  `RouterRulesCard` e `RouterRuleModal`, com tipos em `router-types.ts`; a
  página manteve apenas estado, carregamento e ações.
- Run log dividido em `RunLogHeader` (status, ações e contexto),
  `RunLogBands` (passos, raciocínio, ferramentas e resultado),
  `RunLogResult` e `RunLogComposer`; `RunLogView` ficou com dados e streaming.
- Novos testes cobrem os labels/tons compartilhados e as primitivas de
  formulário (associação de rótulo, hint/erro e estilos base).
- Pacotes divididos em `PacksToolbar`, `PackCatalog`, `PackCard` e
  `WorkspaceTemplateCard`.
- Sem mudança de comportamento ou de visual: `tsc`, ESLint, 178 Vitest, 93
  Playwright e os goldens visuais seguem verdes. Nenhuma release criada.

## 2026-11-13 — onda 13: produto (dashboard, busca, execuções, viewers e pacotes)

- Dashboard: Attention Card resume as pendências por categoria e abre em modo
  compacto com “ver detalhes”; as atividades recentes agrupam por dia e têm
  filtro runs/artefatos. O feed saiu para `ActivityFeed`.
- Busca: a paleta de comandos passou a usar `/api/search` (mesmo ranking da
  página), com resultados agrupados por tipo, contagem e atalho para a busca
  completa. Buscas recentes são persistidas (`recent-searches.ts`) e aparecem
  na paleta e em `/search`.
- Execuções: nova visão em tabela (execução, status, agente, duração, tokens,
  custo, quando) com ordenação por recentes/duração/custo/tokens. O run
  detalhado filtra eventos (tudo, ferramentas, erros, conversa) e expande ou
  recolhe todas as seções.
- Viewers: ZIP lista as entradas sem extrair, com download autenticado; o
  fallback binário oferece baixar. Artefato fora do workspace explica a
  política de segurança e mantém abrir externamente/copiar caminho.
- Pacotes: “Atualizar todos” reinstala pacotes com atualização preservando
  arquivos modificados; modo de gerenciamento com seleção, “selecionar
  instalados” e remoção em lote confirmada; favoritos têm estado vazio próprio.
- Backend: `/api/files/archive` e `/api/artifacts/{id}/archive` listam ZIPs com
  limite de 400 entradas, preservando a fronteira do workspace.
- Verificação: 196 Vitest, 93 Playwright, **74/74 features**, 364 testes Rust,
  clippy, build, goldens visuais e axe sem regressões. Sem release.

## 2026-11-13 — onda 14: plataforma (builds, contrato Google e cobertura Rust)

- Builds: `scripts/release.sh` detecta Linux/macOS/Windows, coleta `.deb`/`.rpm`/
  `.AppImage`, `.dmg`/`.app` ou `.msi`/NSIS, usa binário `opendesk.exe` no
  Windows, assina/verifica `latest.json` e grava a plataforma no manifesto.
  Checksums e retenção de 3 releases deixaram de depender de `sha256sum`/
  `sort -V` e usam Node, funcionando também no macOS.
- Novo `npm run build:platform` (`scripts/build-platform.sh`) valida
  pré-requisitos por SO, roda o gate rápido, executa `tauri build` e lista os
  instaladores sem publicar. Guia em `docs/platform-builds.md`, incluindo
  assinatura Apple, Authenticode, WebView2 e chave do updater.
- Google: contrato expandido para Drive, Gmail (threads), Calendar, Tasks e
  Docs opcional, com checagem de scopes e `tools/list`. `gmail.readonly` entrou
  nos scopes (contas antigas precisam reconectar). Guia em
  `docs/google-contract.md`; `disconnect` e workspace desconectado ganharam
  testes sem rede.
- Cobertura Rust: **64,20% total** e **70,82% core** (antes 62,50%/68,67%).
  Ratchets do gate subiram para 64% e 70,5%; plano em
  `docs/coverage-plan.md`. Novos testes cobrem seed E2E, dedupe, missed runs,
  HITL (parse/resolve/dispatch), SQL sink, ZIP e OAuth desconectado.
- Verificação: 377 testes Rust, clippy/fmt, 196 Vitest, 93 Playwright, 74/74
  features, goldens e axe; sem release.

## 2026-11-13 — onda 15: qualidade (goldens totais, gate e documentação)

- Regressão visual ampliada de 6 superfícies para **18 rotas + 5 modais**
  (nova tarefa, detalhes, rotina, pacote e command palette), em 700, 1024,
  1100, 1440 e 1920 conforme densidade. 49 goldens, ~6 MB.
- Determinismo: Calendário com data fixa, relógio congelado no modal de rotina
  e fuso `America/Sao_Paulo` no contexto visual.
- `scripts/gate.sh` passou a rodar os goldens (opt-out
  `OPENDESK_SKIP_VISUAL=1`); guia em `docs/visual-regression.md` com política
  de atualização.
- Documentação sincronizada: README sem “fases 1–11”, plano com concorrência,
  retry de entregas e Fase 7, interface-proposal com a verificação atual e
  AGENTS apontando para os guias de cobertura, builds e contrato Google.
- Verificação: gate completo (TypeScript, ESLint, cobertura JS, fmt/clippy,
  377 testes Rust, cobertura Rust 64,2%/70,8%, build, 93 Playwright, goldens,
  axe e 74/74 features). Sem release.

## 2026-11-13 — review completo, correções e release 0.51.0

- Review do diff uncommitted encontrou e corrigiu: atualização cross-platform
  aceitando binário de outro SO (agora valida `platform` e nome exato; macOS/
  Windows usam instalador), paleta filtrando resultados já ranqueados, seleção
  de pacotes vazando entre workspaces, filtros de run escondendo permissões,
  ZIP abortando por entrada corrompida, boundary de artefato duplicado e
  `release.sh` sobrescrevendo versão de outra plataforma.
- `release.sh` normaliza `macos-arm64`→`aarch64`, valida versões entre
  package/tauri/cargo, exige instaladores reais e não usa flags GNU-only.
- Calendário dos goldens fixado em data passada para não depender do destaque
  de “hoje”.
- Gate completo verde após as correções: 198 Vitest, 380 testes Rust, 93
  Playwright, 49 goldens, axe e 74/74 features.
- Release **0.51.0** publicada: commit `87bea55`, tag `v0.51.0`, assets Linux
  (deb/rpm/AppImage/binário), `latest.json` assinado, SHA256SUMS conferido e
  retenção mantendo 0.50.0, 0.50.1 e 0.51.0.

## 2026-11-13 — fase 0: entrada, estado e layout

- Busca: removido o eco `URL → input` que apagava o caractere recém-digitado
  quando o debounce gravava a query anterior; o campo mantém a digitação e a
  URL continua sincronizada para deep links/back-forward. Teste de digitação
  rápida adicionado.
- Status único: `runStatusMeta`/`RunStatusIcon` centralizam ícone, cor e label.
  Cancelada agora é cinza com X (não spinner), sucesso verde, falha vermelha,
  execução azul e fila âmbar; Execuções, Calendário, Run log e detalhes usam a
  mesma fonte.
- Sidebar: badge de fila em duas linhas dentro do espaço disponível e indicador
  animado no item Kanban & Fila; o header do Kanban mostra a contagem de
  execuções em andamento.
- Cards do Kanban: ações viraram ícones acessíveis (arquivos, retomar, cancelar,
  executar, abrir, logs), eliminando a sobreposição em cards estreitos.
- Composer: `SlashTextarea` ganhou container flexível; Chat, Meta-Builder e run
  log ocupam toda a largura, com botão Enviar da mesma altura do campo.
- Scroll: hook `useStickToBottom` abre no fim, acompanha novos eventos, pausa
  quando o usuário sobe e oferece “ir para o fim” em Chat e Run log (modal e
  página).
- Ordem: Kanban e exibição da fila mostram o item mais recente no topo,
  preservando a ordem real de execução no backend.
- Atualizar: ícones giram durante a ação em Dashboard, Atenção, Chat,
  Atualizações, importação Google, Topbar, retry do motor e Run log.
- Verificação: tsc, ESLint, 204 Vitest, 93 Playwright, 49 goldens, build e 74/74
  features. Sem release.

## 2026-11-13 — fase 1: Kanban/Fila só de trabalho x Execuções com automações

- Kanban: removido o seletor Trabalho/Automações/Tudo e o campo `origin` das
  views salvas. `/api/tasks?scope=work` devolve apenas `MANUAL` e `CAPTURE`;
  board, lista, contadores e ações em lote trabalham só com essas tarefas.
- Fila: jobs de automação não aparecem. `/api/queue` e eventos passaram a
  expor `task_origin`, e a UI filtra `MANUAL`/`CAPTURE` (Sidebar, header do
  Kanban e controlador). Pausa e concorrência continuam globais, com aviso na
  tela.
- Execuções: default mostra tudo e ganhou filtro por origem — tudo, trabalho,
  automações, rotinas e receitas. Runs carregam `task_origin` e mostram badge
  de origem na tabela e nos cards.
- Classificação: `runScope` cobre MANUAL/CAPTURE como trabalho, RECIPE como
  receita, ROUTINE como rotina, IO_BUS/DELEGATION como automação e runs de
  agenda sem tarefa como rotina.
- Testes: API `scope=work` esconde receitas/rotinas/IO; unidade de `runScope`,
  `workQueue` e views sem origem; E2E atualizado (Kanban sem automações e
  filtro de Execuções visível).
- Verificação: tsc, ESLint, **208 Vitest**, **381 testes Rust**, clippy/fmt,
  **93 Playwright**, goldens de Kanban/Execuções regravados, 74/74 features.
  Sem release.

## 2026-11-13 — fase 2: catálogo, Studio, receitas, downloads e HTML

- Favoritos: storage v2 migra packs/skills e cobre templates, workspaces,
  agentes, comandos, MCPs e recursos externos. Estrela nos templates,
  recomendados, recursos e itens efetivos do Studio; filtro “Só favoritos”
  aplica em todas as abas de Pacotes.
- Recomendados aparecem também na visão geral e na busca, não só na aba
  dedicada; o filtro de favoritos vale para pacotes, templates, skills e
  recursos.
- Instalação de recomendados/externos virou tarefa: título, origem, URL,
  comando, notas e critério de sucesso são enviados ao agente; a tarefa é
  enfileirada e acompanhada em Execuções. O Impeccable não abre mais o site
  como ação principal.
- Studio: resumo de escopo (agentes, skills, comandos, MCPs, habilitados,
  desabilitados e sobrepostos) nas abas Global, Workspace e Efetivo.
- Receitas: sem receita selecionada a tela mostra “Criar ou selecionar”; fluxo,
  YAML e ações só aparecem com receita aberta. Nome editável no topo do fluxo e
  editor YAML colapsável.
- Downloads: preferências de pasta e “perguntar onde salvar”; backend
  `/api/files/save` e `/api/artifacts/{id}/save` copiam para o destino
  escolhido. Ícone de download em Artefatos e botões de pasta/arquivo no
  visualizador.
- HTML: preferência global de scripts e liberação por visualização, com iframe
  `sandbox="allow-scripts"` apenas quando autorizado.
- Verificação: tsc, ESLint, **219 Vitest**, **383 testes Rust**, clippy/fmt,
  **93 Playwright**, goldens atualizados, 74/74 features. Sem release.

## 2026-11-13 — fase 3: I/O, arquivos de entrada, notificações, SMTP e Signal

- Folder Watch: gatilho e fonte aceitam espera antes de processar (`settle_secs`),
  retry (`retry_minutes`), processar vazios, processar arquivos existentes e
  padrões ignorados. O dropzone lê arquivo estabilizado, tenta novamente antes
  de mover para falha e inclui fonte, pasta, tamanho e horário no título/corpo/
  metadados. O runtime de receitas aplica os mesmos campos (estabilidade,
  vazios, existentes, retry).
- Execuções: timeline ganhou `inputs` com arquivos de entrada de receitas,
  anexos e tarefas. O run log exibe uma faixa “arquivos de entrada” com tamanho,
  gatilho e ações abrir arquivo/pasta.
- Notificações: toast e notificação nativa mostram nome da tarefa, agente,
  duração, custo, tokens e motivo da falha; a mensagem é a mesma nos dois canais.
- SMTP: porta 587 com credenciais força STARTTLS exigido, porta 465 usa SSL,
  espaços da App Password/usuário são normalizados, erros de AUTH recebem dica
  objetiva. Sink de e-mail aceita destinatário opcional (fallback para o e-mail
  padrão do workspace), assunto, texto extra e anexos opcionais.
- Signal: o vínculo lê o URI `sgnl://` de stdout **ou stderr**, gera novo QR ao
  repetir e limpa o QR anterior; teste com `signal-cli` falso cobrindo stderr.
- Verificação: tsc, ESLint, **222 Vitest**, **384 testes Rust**, clippy/fmt,
  **93 Playwright**, 49 goldens e 74/74 features. Sem release.

## 2026-11-13 — fase 4: agente OpenDesk e nível de cuidado

- Novo `care_level` na tarefa (`quick`, `standard`, `careful`) com migração
  0034, persistência na API, campo no formulário de tarefa e resumo no detalhe.
- O dispatch injeta instruções de cuidado no prompt:
  - rápido: sem plano formal/auditoria, uma validação objetiva;
  - padrão: checklist curto e uma validação;
  - cuidadoso: revisão de premissas, validação e riscos.
- Em `quick`, quando o agente não foi escolhido manualmente e o arquivo
  `.opencode/agents/opendesk.md` existe, o dispatch usa o novo agente
  **OpenDesk**; caso contrário mantém o `build` (compatível com workspaces
  antigos).
- Pacote base `controle-opendesk` 1.1.0 passa a incluir o agente `opendesk.md`,
  com postura direta para tarefas comuns, instalações e rotinas; tarefas de
  código continuam no `build`.
- Tarefas de instalação de capacidades nascem com cuidado `quick`.
- Testes: API de cuidado (criar/editar/inválido), instruções de prompt e
  detecção do agente no workspace.
- Verificação: tsc, ESLint, **222 Vitest**, **387 testes Rust**, clippy/fmt,
  **93 Playwright**, goldens atualizados e 74/74 features. Sem release.

## 2026-11-13 — fase 5: verificação e entrega (sem release)

- Gate completo verde após todas as fases: tsc, ESLint, cobertura JS, fmt,
  clippy, testes Rust, cobertura Rust, build, 93 Playwright e 49 goldens.
- Cobertura Rust: **64,81% total / 71,14% core**; ratchets subiram para
  64,5% e 70,8%. Novos testes cobrem opções do Folder Watch, processamento real
  de arquivo em tarefa, inputs de run (receita e descrição), credenciais SMTP e
  opções do runtime de receitas.
- Suíte funcional: **74/74 (100%)**. `npm run audit:prod`: 0 vulnerabilidades.
- Relatório de aceitação em `docs/verification.md`, com pendências manuais
  (Gmail real, Signal real, builds macOS/Windows e contrato Google).
- Versão publicada segue 0.51.0; nenhuma tag ou release foi criada.

## 2026-11-13 — release 0.52.0

- Publicados os avanços das fases 0–5: correções de entrada/estado, separação
  trabalho × automações, catálogo/Studio/receitas, I/O e integrações, agente
  OpenDesk com níveis de cuidado e verificação final.
- Review pré-release corrigiu deep links de tarefas de automação, duplicidade
  de eventos no Folder Watch, retry de receitas com falha, seleção do agente
  `opendesk` antes do budget guard, limpeza do e-mail padrão do workspace,
  senha SMTP, downloads em pasta inexistente e escopo de favoritos do Studio.
- Release `1d397c5`, tag `v0.52.0`; assets Linux (deb/rpm/AppImage/binário),
  `latest.json` assinado e SHA256SUMS conferido. Retenção manteve 0.50.1,
  0.51.0 e 0.52.0.

## 2026-11-14 — fluxo de verificação e release rápido

- A verificação padrão passou a ser proporcional (`npm run verify` →
  `scripts/check-changed.sh`): só as áreas tocadas rodam, com Rust
  (fmt/clippy/`cargo test --lib`) e frontend (ESLint/tsc/Vitest) separados.
- `scripts/gate-fast.sh` continua como checagem ampla sem cobertura/E2E e ganhou
  `--changed`.
- Release padrão (`scripts/release.sh`) roda a verificação proporcional e o
  perfil rápido de build (thin LTO, `codegen-units=16`, incremental);
  `--quality-build` mantém LTO completo, `--full-gate` roda o gate completo e
  `--reuse-build` reaproveita o binário quando nenhum fonte é mais novo.
- `scripts/clean.sh` preserva `src-tauri/target`; `--all` remove o target e
  `--deep` remove também `node_modules`. O release não limpa mais o cache.
- `scripts/build-platform.sh` usa a mesma verificação proporcional e o perfil
  rápido por padrão.
- O AppImage deixou de rodar `linuxdeploy` em todo release: o AppDir é reusado e
  só o `appimagetool` roda (~9 s). A chave de cache cobre bibliotecas ligadas
  (`ldd`), configuração sem a versão e ícones; `--refresh-appimage` força o
  rebuild e `--no-appimage` permite release só com `.deb`/`.rpm`.

## 2026-11-14 — Telegram e atalho do AppImage

- Integração Telegram por Bot API em long polling, sem webhook: token no
  `SecretStore`, allowlist por chat ID ou `@username`, offset persistido,
  `/start`/`/help`, `aprovar`/`negar`, HITL e thread de chat `source=telegram`.
- Notificações de rotinas por Telegram (`notify_telegram*`), canal `TELEGRAM`
  no ledger de entregas e teste de aviso; migração 0035.
- Pedidos de permissão do OpenCode também são enviados à allowlist do Telegram.
- AppImage no Linux passa a criar/atualizar o `.desktop` do usuário com
  `StartupWMClass=Opendesk` e ícone absoluto extraído de `$APPDIR`, corrigindo a
  engrenagem genérica na barra de tarefas. Botão **Reintegrar atalho** em
  Configurações → Sobre.
- Testes: 410 Rust e 222 Vitest verdes; verificações manuais com bot real e
  ambiente gráfico continuam pendentes.

## 2026-11-14 — release 0.53.0

- Publicados a integração Telegram (long polling, token criptografado,
  allowlist por chat ID/@username, chat, aprovações, HITL e avisos de rotinas) e
  o atalho automático do AppImage com ícone correto na barra de tarefas.
- O fluxo de verificação/release rápido entrou na versão: `npm run verify`,
  perfil de build thin LTO/incremental, cache preservado e AppImage reaproveitando
  o AppDir.
- Release `77715ca`, tag `v0.53.0`; assets Linux (deb/rpm/AppImage/binário),
  `latest.json` assinado e SHA256SUMS conferido. Retenção manteve 0.51.0,
  0.52.0 e 0.53.0.

## 2026-11-14 — chat operacional e HITL no Telegram

- Chat do Telegram ganhou contexto operacional para usar o MCP
  `opendesk-control`: tarefas, execuções, rotinas, receitas, fila, atenção e
  artefatos. Novo `artifact_read` lê resultados textuais com limite de bytes.
- Indicador “digitando…” via `sendChatAction`, respostas encadeadas com
  `reply_parameters`, reutilização de thread entre workspaces e recuperação de
  sessão expirada do OpenCode.
- `ChatUpdated` mantém a thread aberta no OpenDesk atualizada ao vivo.
- `ask_user` de tarefas envia contexto/pergunta ao Telegram com botões inline;
  texto `#código` é o fallback. Permissões do OpenCode também têm botões
  Aprovar/Negar. `callback_query` entrou no long polling.
- Chat com altura definida e rolagem interna; composer não é mais coberto por
  mensagens. Goldens `chat-1024`/`chat-1440` atualizados e E2E `chat-scroll`.
- Pacote base `controle-opendesk` 1.2.0; workspaces antigos precisam de
  **Pacotes → Atualizar todos**.
- Verificação: 417 testes Rust e 222 Vitest; `npm run verify` verde.

## 2026-11-14 — release 0.54.0

- Publicado o chat operacional do Telegram: tarefas, execuções, rotinas,
  receitas, fila, atenção, resultados e `artifact_read`; “digitando…”,
  continuidade de thread/sessão, atualização ao vivo e botões inline para HITL
  de tarefa e permissões.
- Chat interno corrigido: transcript com rolagem própria, composer sempre
  visível, atualização ao vivo e goldens `chat-1024`/`chat-1440`.
- `dashboard-700` atualizado após o rótulo “inclui automações”. Pacote base
  `controle-opendesk` 1.2.0.
- Release `ca6dd93`, tag `v0.54.0`; assets Linux (deb/rpm/AppImage/binário),
  `latest.json` assinado e SHA256SUMS conferido. Retenção manteve 0.52.0,
  0.53.0 e 0.54.0.

## 2026-11-14 — Folder Watch sem duplicatas e chat no turno certo

- Folder Watch ganhou estado persistente em `recipe_file_state` (migração 0036):
  fingerprint `mtime ns + tamanho`, `SEEN` para arquivos existentes, sem retry
  automático de run falho sem alteração. O primeiro scan após a atualização
  concilia arquivos já presentes em `recipe_runs` para evitar reprocessamento em
  massa.
- O chat passou a esperar o turno novo do OpenCode (`idle`/estabilidade) e a
  interpretar somente mensagens `assistant` criadas após o prompt. A resposta
  antiga não é mais reutilizada quando o modelo ainda está respondendo.
- ADRs 0051 e 0052; 421 testes Rust.

## 2026-11-14 — release 0.55.0

- Folder Watch passou a usar fingerprint persistente (`recipe_file_state`,
  migração 0036); reinícios e runs falhos sem alteração não reprocessam o mesmo
  arquivo. Primeiro scan concilia runs antigos.
- O chat aguarda o turno novo do OpenCode e interpreta somente mensagens
  `assistant` criadas após o prompt; o Telegram deixou de repetir a resposta
  anterior.
- Release `5061662`, tag `v0.55.0`; assets Linux (deb/rpm/AppImage/binário),
  `latest.json` assinado e SHA256SUMS conferido. Retenção manteve 0.53.0,
  0.54.0 e 0.55.0.

## 2026-11-14 — acabamento operacional (fase 1)

- Dashboard ganhou rodapé discreto com status de SMTP, Google, Drive, Calendar,
  Signal e Telegram (`/api/integrations/overview`).
- Busca mostra as últimas atividades quando o campo está vazio
  (`/api/search/recent`): tarefas, execuções, artefatos, comentários, conversas,
  rotinas e arquivos recentes.
- Receitas: execuções clicáveis com página `/recipe-runs/:id`, exclusão com
  confirmação no mesmo botão e editor YAML com divisória redimensionável.
- Abrir pasta/arquivo de workspace usa um comando Rust validado; o escopo do
  plugin opener e workspaces em `/tmp` deixaram de bloquear a ação. Workspaces
  temporários existentes mostram aviso em Configurações.
- GETs read-only do Studio/agentes/modelos saíram do rate limiter local, e
  `apiGet` repete uma vez em 429 respeitando `Retry-After`.
- Verificação: 424 testes Rust, 223 Vitest, E2E `search-recent` e visual verdes.

## 2026-11-14 — contexto pessoal e inspeção de prompt (fase 2)

- Aprendizado ganhou grupos de Personal Context (`workspace` ou `global`), com
  CRUD, ativação, remoção, ordem e injeção automática nos prompts.
- O prompt de tarefas, continuações, rotinas/receitas e chat agora inclui uma
  seção “Contexto pessoal do usuário”.
- Nova aba Configurações → Prompt com o prompt final, seções por origem e o
  contexto carregado pelo OpenCode (agente e skills).
- Migração 0037; `GET /api/tasks/{id}/prompt-preview`; ADR 0053.
- Verificação: 427 testes Rust e 223 Vitest.

## 2026-11-14 — arquivos no Telegram, anexos e watchers (fase 3)

- Telegram aceita documentos, fotos, áudio, voz e vídeo; os arquivos são salvos
  em `input/telegram/YYYY-MM-DD/` e entram no prompt do chat.
- `TELEGRAM_MSG` envia artefatos com `sendDocument`; a entrega de rotinas no
  Telegram também anexa o artefato principal.
- Tarefas podem assinar Telegram e/ou Signal. Cada execução terminal (sucesso,
  falha ou cancelamento) envia status, resumo/erro e artefato principal, com
  deduplicação em `integration_deliveries`.
- Migração 0038 (`task_watchers`); `GET/PUT /api/tasks/{id}/watchers`.
- Formulário de tarefa ganhou “Avisar quando concluir”; editor de receitas
  ganhou configuração do sink Telegram com anexos.

## 2026-11-14 — HITL 2.0 (fase 4)

- HITL unificado em `APPROVAL`, `SINGLE_CHOICE`, `MULTI_CHOICE` e `TEXT`,
  com mapeamento dos nomes legados de receita.
- `ask_user` ganhou `kind`; o agente escolhe a estrutura e pode incluir um
  payload opcional junto da decisão.
- Resposta única em `POST /api/interactions/{id}/resolve` com
  `{ decision, selected, text }` e validação por estrutura.
- Telegram com botões Aprovar/Negar e payload por texto; Signal com
  notificação e resposta `#código`; modal global no AppShell com notificação
  nativa.
- Receitas aceitam os quatro nomes e instruções de resposta por canal.
- Migração 0039; ADR 0055; testes de API, parsing remoto e InteractionCard.

## 2026-11-14 — tela única de execução (fase 5)

- Modal de execução aberto pelo Kanban, lista e card de detalhes: modo, agente,
  modelo, effort, comando/skill/instrução avulsa, status ao vivo e stop.
- `DispatchBody.prompt_append` injeta “Instruções desta execução” no prompt do
  job sem alterar a tarefa, com limite de 8.000 caracteres.
- “Parar execução” usa a transição `CANCELLED` e encerra o job ativo.
- Quando o plano aguarda aprovação, o mesmo botão aprova e executa.
- ADR 0056; testes do modal e do append.

## 2026-11-14 — chat com arquivos e correção do turno

- O turno do chat só termina com texto + `idle`; mensagens de ferramenta sem
  texto não encerram mais a leitura, e turno sem texto vira erro explícito.
- `POST /api/chat/threads/{id}/files` + ferramenta MCP `chat_send_file` enviam
  arquivos do workspace pelo Telegram/Signal, com validação de caminho e limite
  de 50 MB.
- Contexto de canal informa `thread_id` e a ferramenta de envio.
- Runs agora varrem o diretório de saída e registram arquivos criados por shell,
  com deduplicação por hash e `MODIFIED` quando o conteúdo muda.
- ADR 0057.

## 2026-11-15 — chat como execução, prompts configuráveis e prévias

- Chat/Telegram: todo turno fica em `chat_turns`; turnos com ferramenta ou
  arquivo criam uma execução (`task_runs` com `source=chat`, canal e thread),
  registram artefatos e aparecem em Execuções/Artefatos.
- “Pensando” com tempo decorrido, Parar no chat e `/parar` no Telegram; turnos
  longos não são mais cortados em 5 minutos.
- Observabilidade ganhou a tabela “Por origem” (trabalho, rotina, receita,
  automação, chat desktop/Telegram/Signal).
- Prompt: seções estruturadas, templates editáveis com placeholders, camada
  OpenCode (agente, AGENTS.md, skills, comandos e MCP), visão final
  reconstruída e preview específico do chat com contexto do canal e próxima
  mensagem.
- Arquivos: prévia HTML com recarga automática/botão, JSON formatado nativo,
  contexto pessoal sem título e ferramentas externas detectadas no Dashboard.
- Migração 0040; ADRs 0058, 0059 e 0060.

## 2026-11-15 — abas de prompt, cobertura e memória via MCP

- Configurações → Prompt divididas em Templates de prompts, Prompts enviados e
  Prompts de chat; previews com Seções, OpenDesk, OpenCode e Final reconstruído.
- Templates ganharam política de memória, correção de step de receita e prompt
  de teste de modelo; o prompt do chat é o contexto de canal.
- MCP: `learning_save` (aprendizado/MEMORY.md) e `personal_context_save` só com
  pedido explícito e citação do usuário.
- Migração 0041 (`source`/`source_quote` do contexto pessoal); ADR 0061.

## 2026-11-16 — v1.0 onda 0: fundação do control plane

- Contrato `AgentRuntimeAdapter` + `RuntimeEvent` + capabilities + registry.
- Binário `opendeskd` headless com SIGTERM/SIGINT e desligamento gracioso.
- Migração 0042: handoffs, tickets, channel_routes, memória/ACL, workers,
  adapter_registry e runtime_events.
- Registro de variáveis de prompt, incluindo memória, handoff, trust e harness.
- ADR 0062. Próxima onda: daemon/PWA/auth/CLI.

## 2026-11-16 — v1.0 onda 1 (parte 1): daemon, PWA e CLI chat

- `OPENDESK_API_HOST` configurável; `opendeskd` com bind explícito.
- Axum serve a SPA/PWA quando `OPENDESK_WEB_DIR` aponta para `dist`.
- Login remoto por sessão (hash, TTL e revogação), API same-origin e WebSocket
  pelo host atual.
- Manifest/service worker e configuração mobile.
- `opendesk chat` com mensagem única ou loop interativo, além de
  `approvals list/resolve` e `diff <run>`.
- Serviços de exemplo systemd/launchd/Windows e `docs/daemon.md`.
- ADR 0063. Pendências: core extraction, passkey e TUI completa.

### Web Push/VAPID

- Migração 0044 (`push_subscriptions`) e chaves VAPID em `push_vapid.json` 0600.
- `GET /api/push/vapid`, `POST /api/push/subscribe|unsubscribe|test` e
  `GET /api/push/subscriptions`.
- Dispatcher no `EventBus` para HITL, permissões, fim de execução, propostas e
  orçamento, com remoção de endpoints expirados.
- Painel de ativação/teste no PWA e handlers `push`/`notificationclick` no
  service worker.
- ADR 0064.

## 2026-11-16 — v1.0 onda 2: harnesses plugáveis

- `OpenCodeAdapter` (daemon REST/SSE) e `CliAdapter` (Claude Code, Codex, Pi,
  Hermes) sob o mesmo contrato normalizado.
- Execução compartilhada: o mesmo `RunOutcome` alimenta fila, artefatos, budget
  e notificações, independente do harness.
- Diffs de CLIs via `git status --porcelain`.
- Migração 0045: `tasks.preferred_harness`, `execution_queue.resolved_harness`
  e metadados de comando/health em `adapter_registry`.
- API `/api/adapters` (listar, editar, health, padrão) e UI Configurações →
  Harnesses.
- DSH escolhe harness (tarefa → padrão do workspace → OpenCode) e o exibe no
  teste de roteamento; o formulário de tarefa permite a escolha.
- ADR 0065.

## 2026-11-16 — v1.0 onda 3: canais, tickets e confiança

- `channel_routes` com identidade, agente, perfil e trust; rota exata + wildcard.
- Tickets idempotentes por mensagem, com triagem e vínculo opcional a tarefa.
- Webhook universal `/api/webhooks/channels/{channel}` autenticado por token
  (SecretStore) e política de confiança: untrusted só ticket; internal retido;
  trusted cria tarefa.
- Configuração estrutural de WhatsApp (provider/número/notas) sem conector.
- Aba Integrações → Canais e tickets com rotas, tokens, WhatsApp e inbox.
- Evento `TicketUpdated` e push para novos tickets.
- ADR 0066.

## 2026-11-16 — v1.0 onda 4: handoffs de tarefa

- Contrato de handoff (objetivo, artefatos, schema, aceite, sub-orçamento) e
  máquina de estados com transições explícitas.
- Aceite humano cria tarefa `DELEGATION` para o agente destino; rejeição
  preserva o motivo; conclusão do run fecha o handoff.
- Guard de orçamento com confirmação forçada.
- MCP `handoff_create/list/respond/finish`.
- Seção “Handoffs de tarefa” na governança e push para handoffs pendentes.
- ADR 0067.

## 2026-11-16 — v1.0 onda 5: memória, ACL e avaliação de skills

- Notas estruturadas com namespace, tipo, tags, confiança, trust e validade.
- ACL por agente (leitura/escrita) com namespace aberto por padrão.
- Busca e injeção de até 8 notas no prompt (`{{structured_memory}}`), filtradas
  pela ACL; MEMORY.md permanece separado.
- Limpeza de notas expiradas na manutenção.
- MCP `memory_save`/`memory_search`.
- `/api/skills/eval` e cards de memória/avaliação na página de aprendizado.
- ADR 0068.

## 2026-11-16 — v1.0 onda 6: MCP Hub

- Inventário global + workspace de `mcp.servers` (V1/V2, local/remoto).
- Saúde estática e de runtime; teste de comando no PATH ou URL remota.
- Catálogo curado (filesystem, fetch, sqlite, playwright, memory) com instalação
  explícita no config nativo.
- Toggle por `disabled`, remoção com backup e auditoria da API.
- Aba Integrações → MCP Hub.
- ADR 0069.

## 2026-11-16 — v1.0 onda 7: sandbox e computer-use

- Política de sandbox (`off`/`workspace`/`strict`) com allowlist, env scrubbing,
  timeout configurável e bubblewrap sem fallback silencioso.
- Sessões de computer-use com aprovação humana, capturas grim/import e entrada
  xdotool/ydotool.
- Texto digitado não é persistido; ações são contadas e resumidas.
- API `/api/sandbox` e `/api/computer/*`; UI Configurações → Sandbox.
- Migração 0046 e ADR 0070.

## 2026-11-16 — v1.0 onda 8: voz, multi-node e binários

- STT/TTS por motores do host, com placeholders, limites e capacidades.
- Áudio temporário fora do workspace; fala em `output/YYYY-MM/voice`.
- Protocolo de nós: registro com token, heartbeat, claim atômico, lease e
  conclusão com run/usage/eventos.
- Token de nó restrito aos endpoints do protocolo; fila única com work stealing.
- CLI `opendesk nodes list`; UI de voz e nós em Configurações.
- Migração 0047 e ADR 0071.

## 2026-11-16 — v1.0 onda 9: hardening e fechamento

- Cabeçalhos de segurança e CSP para a PWA; `/readyz` sem autenticação.
- Retenção de sessões remotas e de computer-use na manutenção.
- Versão 1.0.0 em package.json, Cargo.toml/Cargo.lock e tauri.conf.json.
- Notas de release em `docs/releases/1.0.0.md`, threat model e AGENTS atualizados.
- Verificação final: 513 testes Rust, Vitest, ESLint, tsc e goldens visuais.
- ADR 0072.

## 2026-11-16 — 1.1.0: passkeys, bootstrap compartilhado e TUI

- Passkeys WebAuthn: cerimônias no servidor, credenciais em SQLite, sessão
  revogável e fallback por token (migração 0048, ADR 0073).
- `control_plane::start` unifica a inicialização do desktop e do `opendeskd`.
- `opendesk tui` com tarefas, runs, aprovações, fila, nós e diffs.
- UI de passkeys em Configurações → Segurança e login por passkey no PWA.
- ADR 0074.

## 2026-11-16 — 1.2.0: worker remoto e opendeskd no release

- `opendesk node run` executa jobs CLI em nós remotos com heartbeat, claim,
  execução no workspace e complete com run/uso/resumo.
- Harnesses suportados no worker: Claude Code, Codex, Pi e Hermes.
- Release passa a publicar `opendeskd-<plataforma>` além do desktop.
- Teste do mapeamento de comandos e documentação do worker.
- ADR 0075.

## 2026-11-16 — pós-1.2.0: WhatsApp Cloud API e hardening multi-node

- WhatsApp Cloud API: verificação de webhook, assinatura HMAC, normalização de
  texto/botão/interativo/mídia, envio de texto e credenciais no SecretStore.
- Worker remoto aplica sandbox do workspace, envia diffs e a central registra
  artefatos com validação de caminho.
- Rotação de token de nó e botão na UI.
- Sandbox por workspace com fallback global.
- AppImage passa a incluir `opendeskd`.
- ADRs 0076 e 0077.

## 2026-11-16 — worker remoto com OpenCode

- `opendesk node run` aceita `--opencode-url`/`--opencode-password` (ou env) e
  executa jobs `opencode` via REST/SSE local do nó.
- Texto, falha, idle e diffs seguem o mesmo `complete` dos harnesses CLI.
- ADR 0078.

## 2026-11-16 — mídia do WhatsApp e daemon no .deb

- Download de mídia Meta (25 MB) para `input/whatsapp/` com extensão segura.
- `.deb` passa a incluir `opendeskd` e unit systemd de usuário via pós-processo
  `dpkg-deb`; RPM segue sem reconstrução.
- ADR 0079.

## 2026-11-16 — receitas duráveis e contratos JSON

- Migração 0049: `recipe_run_nodes` e `cancel_requested`.
- Resume após restart, cancelamento cooperativo, retry do run e timeline por nó.
- HITL idempotente em retomadas.
- `input_schema`/`output_schema` por passo com validação e retry de formatação.
- Editor visual de schemas e timeline na execução.
- ADRs 0080 e 0081.

## 2026-11-16 — MCP dinâmico e tools de governança

- `OPENDESK_MCP_PHASE`/`ROLE` filtram tools por fase; `ALL` mantém tudo.
- `mcp_discover_tools` retorna schemas sob demanda.
- `recipe_resume`, `recipe_cancel` e `voice_speak` no catálogo (38 tools).
- ADR 0082.

## 2026-11-16 — passos condicionais em receitas

- Campo `when` com caminho, `==`, `!=` e `not`; passo pulado vira nó `SKIPPED`.
- Editor visual do passo e testes de avaliação.
- ADR 0083.

## 2026-11-16 — ciclo de vida de memória

- Notas expiradas/duplicadas vão para arquivo, não são apagadas.
- Decaimento de confiança para notas não verificadas e consolidação na manutenção.
- Migração 0050 e ADR 0084.

## 2026-11-16 — pack assistente pessoal

- Novo pack `assistente-pessoal`: briefing matinal com HITL, agente, skill e comando.
- Catálogo atualizado; receita nasce desligada.
- ADR 0085.

## 2026-11-16 — doctor --live

- `opendesk doctor --live` sonda daemon, harnesses CLI, SMTP, Google, Telegram,
  Signal, WhatsApp e origem WebAuthn com relatório acionável.
- Sem release.

## 2026-11-16 — custo estimado para harnesses CLI

- Tokenizador aproximado (chars/4) e tabela de preços por família de modelo.
- Runs CLI sem telemetria preenchem tokens/custo estimados e registram no log.
- ADR 0086.

## 2026-11-16 — Inbox Triage & Drafts

- Segunda receita do pack assistente-pessoal: triagem, rascunhos, HITL e lista final sem envio automático.

## 2026-11-16 — Personal Life & Travel Ops

- Terceira receita do pack assistente-pessoal: prazos pessoais, viagens e checklists com HITL.

## 2026-11-16 — probe de daemon externo

- `daemon_probe` lê api.json, aceita apenas localhost e confirma /healthz em 1s.
- Base para o auto-attach Desktop↔daemon (REF-02).

## 2026-11-16 — auto-attach Desktop↔daemon (REF-02)

- Desktop sonda `/healthz` via `api.json` antes de criar estado local.
- `ShellState::Remote/Embedded`; modo remoto não abre SQLite, não reserva porta e não escreve `api.json`.
- `server.pid` impede duplicação acidental; `opendeskd` usa o mesmo lock.
- Bandeja e `get_daemon_status` falam com o daemon remoto; abrir caminhos valida via `/api/workspaces`.
- ADR 0087.

## 2026-11-16 — write-buffer de heartbeats (REF-03)

- Heartbeats de jobs e presença de nós passam por um writer único com lote de 250 ms e `BEGIN IMMEDIATE`.
- Coalescing por job/nó, backpressure por canal limitado e métricas em `/api/about`.
- Transições de estado continuam síncronas; carga de 4 runs + 8 nós sem falha de lock no teste.
- ADR 0088.

## 2026-11-16 — opendesk-core (REF-01, primeira fatia)

- `src-tauri` virou workspace com `crates/opendesk-core` sem Tauri.
- Movidos `error`, `events`, `paths`, `text` e `token_estimate`, reexportados pelo crate raiz.
- Testes e clippy cobrem core e shell via `default-members`.
- ADR 0089.

## 2026-11-16 — receitas DAG (REQ-02)

- `branch`, `parallel` (join all/any) e `map` com concorrência limitada.
- Nós duráveis por caminho (`node_path`) e migração 0051; grupos concluídos restauram outputs.
- Resume não repete nós DONE/SKIPPED; map materializa itens e ordena resultados.
- Limites de profundidade/nós/branches/concorrência; HITL/sink fora de grupos concorrentes.
- ADR 0090.

## 2026-11-16 — receitas/agentes como tools MCP (MCP-04)

- MCP de controle registra `recipe_<id>` e `agent_<id>` dinamicamente por workspace.
- Receitas rodam com `trigger`/`dry_run`; agentes criam tarefa e tentam dispatch.
- `mcp_discover_tools` inclui as tools dinâmicas; AUDIT fica sem elas.
- ADR 0091.

## 2026-11-16 — worktrees por run (REQ-06/MCP-05)

- Worktree Git com branch temporário e overlay sem Git; diff e merge com aprovação.
- Tabela `run_worktrees` (migração 0052), rotas de API e tools MCP `worktree_*`.
- Conflito aborta sem alterar a árvore; cleanup remove worktree/branch.
- ADR 0092.

## 2026-11-16 — shell mobile-first (REQ-12)

- Barra inferior de 4 destinos em `<768px`; sidebar oculta no celular.
- Safe area, viewport-fit=cover e main com padding para a barra.
- Topbar compacta e testes Playwright de 390×844 sem overflow.
- ADR 0093.

## 2026-11-16 — onboarding guiado (REQ-19)

- `GET /api/onboarding` valida workspace, motor, MCP, pacote base e integrações reais.
- Diálogo "Primeiros passos" com essenciais/opcionais, progresso e links diretos.
- Teste E2E com workspace real; goldens de Kanban/Pacotes atualizados.
- ADR 0094.

## 2026-11-16 — doctor live completo (REF-05)

- Google valida refresh token; Telegram chama getMe; Signal valida CLI; WhatsApp consulta Graph.
- Estados error/disabled/not-configured distintos e timeouts curtos.
- ADR 0095.

## 2026-11-16 — sub-receitas (REQ-04)

- Item `type: recipe` com `with`, `when`, outputs isolados e caminho durável `call/...`.
- Detecção de ciclo e limite de profundidade; resume preserva passos da sub-receita.
- ADR 0096.

## 2026-11-16 — sandbox em container (REQ-09)

- Modo `container` com Docker/Podman, imagem única, rootfs read-only, rede none/bridge e UID local.
- `container_runtime` exposto na API/UI; Dockerfile base em `packaging/sandbox/`.
- Sem runtime falha explicitamente; nunca degrada para execução sem sandbox.
- ADR 0097.

## 2026-11-16 — SessionContextPacket (REQ-07)

- Pacote entre harnesses com plano, decisões, diffs, artefatos, memória e orçamento.
- Persistido em `session_context_packets` e injetado no prompt de dispatch/follow-up/preview.
- `GET /api/tasks/{id}/session-packet` para inspeção; limites de tamanho.
- ADR 0098.

## 2026-11-16 — versionamento de receitas (REQ-04)

- `recipe_versions` com conteúdo, SHA e timestamp; edições idênticas não duplicam.
- API de listagem, leitura, diff unificado e rollback revalidado; tools MCP `recipe_versions/diff/rollback`.
- ADR 0099.

## 2026-11-16 — orçamento por sub-receita (REQ-04)

- `budget_usd` em chamadas de sub-receita; gasto acumulado por escopo e grupos.
- Estouro falha com gasto/limite; resume preserva `spent_usd` de grupos concluídos.
- ADR 0100.

## 2026-11-16 — display virtual isolado (REQ-15)

- Sessões computer-use com Xvfb próprio, display persistido e cleanup ao encerrar.
- Captura X11 usa `import -window root`; UI liga isolamento por padrão.
- Teste E2E cria sessão isolada, captura e encerra; visual de Pacotes estabilizado.
- ADR 0101.

## 2026-11-16 — segunda fatia do core (REF-01)

- `worktree` e `virtual_display` movidos para `opendesk-core`, sem Tauri.
- Testes de git/overlay/Xvfb passam a rodar no crate do core.
- ADR 0102.

## 2026-11-16 — allowlist de janela/app (REQ-15)

- Allowlist persistida com janelas/processos, checagem antes de captura e ação, falha fechada.
- API e UI de configuração; sessões isoladas mostram display.
- ADR 0103.

## 2026-11-16 — Eval Gate histórico de skills (REQ-16)

- `skill_eval_gates` com PASS/BLOCKED/INSUFFICIENT e thresholds configuráveis.
- API de execução, último gate e decisão humana; aprovar bloqueado é conflito.
- ADR 0104.

## 2026-11-16 — sync delta de workspace (REQ-10)

- Manifesto com SHA/mtime, delta e bundle tar.zst; apply com dry-run e proteção de path.
- API de manifest/bundle/apply por workspace; bundles restritos a data/sync.
- mTLS e sync automático no worker ainda pendentes.
- ADR 0105.

## 2026-11-16 — MCP gateway HTTP (MCP-01)

- `POST /api/mcp` JSON-RPC com initialize/tools/list/tools/call reutilizando o control plane.
- Auth/rate limit/audit do middleware; filtro por fase e agente padrão por header.
- Faltam SSE/proxy de servidores externos e RBAC por agente.
- ADR 0106.

## 2026-11-16 — terceira fatia do core (REF-01)

- `workspace_sync` e `server_lock` movidos para `opendesk-core`.
- Helper `paths::test_paths` para testes sem AppState.
- ADR 0107.

## 2026-11-16 — DB no core (REF-01)

- `db`, migrações, `dedupe`, `nodes` e `passkey` movidos para `opendesk-core`.
- `write_buffer` usa `tokio::spawn`; test_support próprio do core.
- Core com 83 testes, shell com 495; sem Tauri no core.
- `AppPaths.allow_temp_workspaces` e reclaim de displays Xvfb obsoletos.
- ADR 0108.

## 2026-11-16 — worktree no executor (REQ-06)

- Setting `worktree_runs` liga isolamento opt-in no `execute_job`.
- Output, artefatos e harness usam o root do worktree; merge segue via API/MCP.
- Falha de preparação falha o job sem fallback silencioso.
- ADR 0109.

## 2026-11-16 — RBAC de role no MCP HTTP (MCP-01)

- Header `x-opendesk-mcp-role` restringe fases; phase explícito vence.
- Teste HTTP garante que read não vê tools de build.
- ADR 0110.

## 2026-11-16 — secrets no core (REF-01)

- `secrets` movido para `opendesk-core`; shell reexporta.
- Próximo bloqueio: notificações Telegram/Signal no `session_runner`.
- Core com 84 testes, shell com 496.
- ADR 0111.

## 2026-11-16 — runtime/adapters no core (REF-01)

- `opencode_client`, `runtime`, `sandbox` e `adapters` no `opendesk-core`.
- `RunNotifier` remove integrações do session_runner; shell injeta Telegram/Signal.
- Core com 120 testes; shell 460.
- ADR 0112.

## 2026-11-16 — browse tags no core (REF-01)

- `browse_tags` (allowlist, intenção e risco) movido para o core.
- Shell browse reexporta; dsh_router pode deixar de depender de AppState.
- Core com 122 testes; shell 460.
- ADR 0113.

## 2026-11-16 — dsh_router/config_sync no core (REF-01)

- `dsh_router` e `config_sync` movidos para `opendesk-core`.
- Core com 162 testes; shell 420.
- ADR 0114.

## 2026-11-16 — threads de e-mail (REQ-14)

- SMTP com In-Reply-To/References e endpoint `/api/integrations/smtp/reply`.
- IMAP preserva message_id/in_reply_to/references/subject no metadata.
- ADR 0115.

## 2026-11-16 — módulos folha no core (REF-01)

- `media`, `skill_eval`, `personal_context` e `run_timeline` movidos.
- Core 168 testes; shell 415.
- ADR 0116.

## 2026-11-16 — browse no core (REF-01)

- Browse sem AppState; recebe pool/paths e roda no core.
- Core 174 testes; shell 409.
- ADR 0117.

## 2026-11-16 — parser/store de receitas no core (REF-01)

- `hitl`, `recipes::parser` e `recipes::store` movidos para `opendesk-core`.
- Engine/HITL de canais/sinks/cron seguem no shell.
- Core 197 testes; shell 386.
- ADR 0118.

## 2026-11-16 — recurrence/cron de receitas no core (REF-01)

- `next_occurrence` em `opendesk-core::recurrence`; `recipes::cron` movido.
- Core 201 testes; shell 382.
- ADR 0119.

## 2026-11-16 — run_export no core (REF-01)

- Export/timeline sem AppState; recebe pool.
- Core 205 testes; shell 378.
- ADR 0120.

## 2026-11-16 — metrics no core (REF-01)

- `metrics` movido; core 207 testes, shell 376.
- ADR 0121.

## 2026-11-16 — audit no core (REF-01)

- `audit` movido; core 210 testes, shell 373.
- ADR 0122.

## 2026-11-16 — backup no core (REF-01)

- `backup` sem AppState; core 212 testes, shell 371.
- ADR 0123.

## 2026-11-16 — update_signing/mcp_server no core (REF-01)

- Módulos movidos; core 216 testes, shell 367.
- ADR 0124.

## 2026-11-16 — dispatch de receita por trait (REF-01)

- `RecipeStepDispatcher` no core; engine não importa mais api::tasks.
- ADR 0125.

## 2026-11-16 — desktop_integration no core (REF-01)

- Módulo movido; core 219 testes, shell 364.
- ADR 0126.

## 2026-11-16 — dispatcher injetado no engine (REF-01)

- execute/DAG recebem RecipeStepDispatcher; testes usam Noop.
- ADR 0127.
