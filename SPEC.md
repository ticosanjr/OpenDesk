# OpenDesk — Especificação

Fonte única de produto e requisitos. Arquitetura em `ARCHITECTURE.md`; design
visual em `DESIGN.md`; histórico V1 em `docs/archive/v1/`.

## 1. Produto

OpenDesk é o control plane visual do OpenCode: um app desktop (Tauri v2 +
WebView) que governa agentes, tarefas, automações, artefatos, fila, custos e
proveniência, mantendo a configuração do OpenCode em arquivos nativos.

**Usuários.** Operadores técnicos e pequenos times de IA que rodam agentes em
projetos reais e precisam ver o que rodou, por quê, quanto custou e o que
produziu, alternando entre planejar, executar, revisar e governar.

**Posicionamento.** Não é wrapper de chat nem plataforma hospedada. A promessa é
governança local-first: configuração e workspaces no disco, execução delegada ao
daemon OpenCode V2, e toda ação operacional observável e auditável em uma única
superfície.

**Princípios.** Verdade antes de decoração; execução delegada; intervir no
momento certo; denso mas legível; entrega determinística.

## 2. Contexto operacional

- Workspaces apontam para pastas reais; `input/` e `output/` fazem parte do
  contrato.
- O daemon pode ser local, compartilhado ou estar offline; a UI explica o estado
  em vez de esconder.
- Runs são longos e assíncronos; o app fica aberto, usa tray e retoma decisões.
- Aprovações chegam pela UI, e-mail, Signal e Telegram; precisam convergir para
  um único estado canônico.
- Dados densos são normais: fila, timeline, tabelas, logs, diffs, custos e
  tokens.
- SSOT no disco (`opencode.jsonc`, `.opencode/agents`, `.opencode/skills`,
  `.opencode/commands`, `mcp.servers`); SQLite só guarda estado operacional.

## 3. Capacidades

### 3.1 Workspaces e Studio

- Criar/editar/abrir workspaces, definir pasta raiz e workspace ativo.
- Studio: agentes, skills, comandos, MCP e JSONC bruto; catálogo curado,
  copiar/mover/duplicar, toggle, backup e instalação.
- `name`, `avatar` e `color` de agentes são metadados do OpenDesk no frontmatter;
  o restante é nativo.

### 3.2 Kanban e tarefas

- Colunas: BACKLOG, TODO, IN_PROGRESS, IN_REVIEW, BLOCKED, DONE, CANCELLED.
- Transições validadas; `DONE`/`CANCELLED` são terminais (fork reabre).
- Edição, comentários, anexos, tags, prioridade, agente/modelo/harness, fork,
  deduplicação com carência, hold e retomada.
- Views salvas, filtros, modo lista/lote e paginação.

### 3.3 Execução

- Fases Plan → Build → Audit, com agente/modelo/esforço por fase.
- Fila com concorrência, grupos de recurso, lease/heartbeat, retry, fallback de
  modelo e budget guard.
- Timeline ao vivo, stop, permissões interativas e continuidade de sessão.
- Artefatos com proveniência, diff, preview, pin, rollback e export.
- Entrega pós-run em pasta determinística `output/YYYY-MM/<slug>/`.

### 3.4 Rotinas

- Cron com timezone, recorrência intuitiva, histórico, edição e dry-run.
- Política de misfire (`RUN_IMMEDIATELY`/`SKIP_TO_NEXT`), modo seguro
  somente-leitura e notificação só por novidade.
- Gatilhos: pasta, e-mail (IMAP), webhook, SQL poller, Signal, Telegram e
  receitas.

### 3.5 Receitas

- YAML em `<workspace>/.opencode/recipes/<slug>.yaml` é a verdade; SQLite
  indexa metadados e execuções.
- Steps sequenciais no mesmo pipeline de fila/sessão, com DAG, budget e retry.
- Checkpoints HITL, sinks (PDF/XLSX, FS atômico, S3/MinIO, SMTP, Git, SQL,
  Signal/Telegram, Rsync local/remoto).
- Editor dual-mode (formulário + Monaco/textarea), validação em tempo real e
  dry-run sem efeitos externos.

### 3.6 HITL 2.0

Quatro tipos canônicos: `APPROVAL`, `SINGLE_CHOICE`, `MULTI_CHOICE`, `TEXT`.
As cinco modalidades legadas (binary gate, approval with feedback, artifact edit
handoff, tool permission proxy, acknowledge) mapeiam para esses tipos.

- Token curto, one-shot, com expiração; o primeiro canal que responde vence.
- Respostas: aprovar/rejeitar, índice de opção, texto ou anexo.
- Persistência em `messages(kind=hitl_request)`; UI, e-mail, Signal e Telegram
  compartilham o mesmo estado.
- Timeout: pausar e alertar, auto-aprovar ou abortar.

### 3.7 Barramento universal de I/O

Quatro famílias de entrada normalizadas em `NormalizedTaskEvent`:

1. **Dropzone** (`input/dropzone/`): documentos, planilhas, imagens (OCR) e áudio
   (STT), com debounce e lock.
2. **IMAP**: inbox por workspace, MIME → Markdown, anexos em
   `input/attachments/<id>/`.
3. **Webhooks e chat**: JSONPath para título/corpo/prioridade; HMAC obrigatório,
   comparação em tempo constante.
4. **SQL poller**: query + confirmação, com limite por ciclo.

Sinks determinísticos após SUCCESS: compilador (PDF/XLSX/CSV/DOCX), FS atômico,
S3/MinIO, Git, SQL, SMTP e mensageria. Sink nunca bloqueia a finalização; falha
vira delivery/retry.

### 3.8 Canais e notificações

- Telegram (long polling, botões, mídia), Signal (allowlist, respostas, áudio),
  e-mail (SMTP/IMAP, `Message-ID`/`In-Reply-To`/`References`) e Web Push.
- WhatsApp Cloud API e voz contínua são roadmap (exigem telefone/QR e
  VAD/barge-in).
- `NotificationRouter` centraliza severidade, quiet hours e roteamento; nenhum
  adaptador decide política.
- Segredos só por `secret_ref` resolvido no `SecretStore`.

### 3.9 Governança e observabilidade

- Organograma, metas, budgets, quota, performance, delegação HITL e heartbeat.
- p50/p95, fila, falhas, custo, tokens, cache, model mix e economia do roteador.
- Doctor com probes de binário, banco, sandbox, display, mTLS, canais e webhooks.

### 3.10 Memória, aprendizado e avaliação

- Contexto pessoal e memória de workspace via MCP, com ACL e namespaces.
- Lições de runs e propostas de skill com aprovação HITL.
- Eval gate com dataset histórico, replay sombra e promoção/rollback.

### 3.11 Chat e captura

- Chat com qualquer agente, anexos, arquivos, execuções e stop.
- Modo pair-programming com troca de modelo e arrastar/soltar arquivos.
- Quick Capture global (Ctrl+Shift+Space) com clipboard/imagem.
- Busca global e command palette; deep links universais.

## 4. Contratos

### 4.1 OpenCode V2

- `/api/session`, `/api/session/:id/prompt`, eventos SSE em `/api/event`.
- Basic auth `opencode:<senha>`; `permissions`, `commands`, `mcp.servers`,
  `disabled`, `type: "remote"`.
- Proibido: `/v1/*`, `permission` objeto, `command` singular, `type: "sse"`,
  `enabled`.

### 4.2 API local

- Bearer token em `0600`; rate limit e auditoria.
- Rotas `/api/*`, `/ws` e webhooks públicos autenticados por HMAC.
- Envelope WS `{ event_id, event_type, payload }`; perda de eventos emite
  `ResyncRequired`.

### 4.3 Segredos

- `SecretStore` com AES-256-GCM e arquivos `0600`; keyring e senha mestra são
  roadmap.
- Configuração carrega `secret_ref`; nunca o valor.
- Proibido logar senha, token, prompt completo ou conteúdo de `.env`.

### 4.4 Dados

- Baseline V2 única, `STRICT`, em `crates/opendesk-storage/migrations/`.
- Conversas/mensagens unificadas; cursor por UUIDv7.
- Acesso V1 é export-only (`opendesk-cli migrate-v1-export`).

## 5. Interface

**Direção:** control plane silencioso, instrumento de precisão. Superfícies
escuras, um acento do workspace, tipografia com papéis claros e dados densos
separados por espaço, não por caixas dentro de caixas.

- Um trabalho por zona; ação primária visível; filtros secundários sob demanda.
- Acento marca estrutura; azul/verde/âmbar/vermelho só informam.
- Corpo 13,5px, metadados 12px, overline 11px; mono para números/IDs/caminhos.
- Divisores antes de cards; revelação progressiva; todo controle tem estados.
- Kanban com views salvas; Settings/Integrações em abas; governança em
  organograma; dashboard com faixa única + Attention Card.
- Acessibilidade WCAG 2.1 AA: contraste, foco visível, alvos de teclado, modais
  com focus trap e `prefers-reduced-motion`.

## 6. Não-objetivos e limitações

- Windows/macOS: builds externos (VMs, Authenticode, notarização).
- WhatsApp, voz contínua e computer-use Win/mac: roadmap.
- Container sandbox depende de Docker/Podman com permissão.
- Cobertura Rust segue o piso acordado no gate; não é 80% ainda.
- Sem promessas de benchmark, clientes ou preços sem evidência.

## 7. Glossário

- **Run:** uma execução de agente, com log, uso, artefatos e status.
- **Job:** item da fila que origina um run.
- **Receita:** automação YAML com trigger, steps, HITL e sinks.
- **Sink:** entrega determinística pós-run.
- **HITL:** decisão humana que destrava um fluxo.
- **SSOT:** arquivo nativo no disco como fonte de verdade.
- **DSH:** roteador de modelo por custo/benefício.
