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
