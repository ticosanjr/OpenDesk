# Especificação: Motor de Receitas, Editor Dual-Mode e HITL Modular (`SPEC_RECIPES_HITL.md`)

> **Status:** documento fonte arquivado no plano. Na implementação, copiar para
> `docs/specs/SPEC_RECIPES_HITL.md` e criar o ADR `0029-recipes-hitl.md`.
>
> **Lei:** Zero Abstraction Layer. Toda receita é um arquivo físico em
> `<workspace>/.opencode/recipes/<slug>.yaml`. O SQLite guarda apenas índice,
> execuções e checkpoints.

## 1. Topologia

```text
[GATILHO] -> [RECIPE ENGINE Rust] -> [STEP 1..N via OpenCode :3100]
                    |
                    +-> [HITL CHECKPOINT] -> persiste WAITING_HITL + token
                    |        +-> Signal / E-mail / Desktop UI
                    |        +-> primeiro canal que responde destrava
                    |
                    +-> [SINKS] PDF, XLSX, SMTP, Signal, Git, Drive, Rsync
```

## 2. Arquivo de receita

Exemplo canônico (sem coordenadas visuais/X-Y):

```yaml
id: fechamento-mensal-notas
name: Processamento e Auditoria de Notas Fiscais
description: Extrai dados de PDFs, audita e pede aprovação.
enabled: true

trigger:
  type: FOLDER_WATCH
  config:
    path: "input/dropzone/faturas"
    glob: "*.{pdf,png,jpg}"
    pre_processor: "ocr_tesseract"

steps:
  - id: extracao
    name: Extração e Consolidação
    mode: build
    agent: "@extrator-fiscal"
    model: "opencode-go/deepseek-v4.1-flash"
    effort: low
    prompt: |
      Analise {{trigger.file_path}} e salve {{run.output_dir}}/resumo_faturas.json.

  - id: auditoria
    name: Validação Contábil
    mode: plan
    agent: "@auditor"
    model: "opencode-go/glm-5.3"
    effort: high
    prompt: |
      Verifique {{run.output_dir}}/resumo_faturas.json.
      Redija {{run.output_dir}}/parecer_auditoria.md.

hitl_checkpoints:
  - id: aprovacao_diretoria
    after_step: auditoria
    interaction_type: APPROVAL_WITH_FEEDBACK
    channels: ["SIGNAL", "EMAIL", "DESKTOP_UI"]
    timeout_minutes: 240
    on_timeout: PAUSE_AND_ALERT
    prompt_template: |
      Auditoria concluída para {{trigger.file_name}}.
      Resumo: {{steps.auditoria.summary}}
      Responda APROVAR, REJEITAR ou envie observações.
    attach_files:
      - "{{run.output_dir}}/parecer_auditoria.md"

sinks:
  - type: DOC_COMPILER
    config:
      source: "{{run.output_dir}}/parecer_auditoria.md"
      format: "pdf"
      output: "{{run.output_dir}}/parecer_auditoria.pdf"
  - type: EMAIL_SMTP
    config:
      to: ["contabilidade@empresa.com"]
      subject: "[Aprovado] Fechamento Fiscal - {{run.date}}"
      body_from_file: "{{run.output_dir}}/parecer_auditoria.md"
      attachments: ["{{run.output_dir}}/parecer_auditoria.pdf"]
  - type: SIGNAL_MSG
    config:
      recipient: "+393330000000"
      message: "Fechamento fiscal aprovado."
      attachments: ["{{run.output_dir}}/parecer_auditoria.pdf"]
```

## 3. HITL — 5 modalidades

1. **BINARY_GATE** — aprovar/rejeitar antes de um passo ou sink.
2. **MULTI_CHOICE** — N opções; resposta numérica.
3. **APPROVAL_WITH_FEEDBACK** — aprovar, rejeitar ou texto/áudio que re-executa
   o step anterior com a correção.
4. **ARTIFACT_EDIT_HANDOFF** — anexo editado volta e substitui o artefato.
5. **TOOL_PERMISSION_PROXY** — permissão `ask` do OpenCode encaminhada para
   Signal/E-mail com token; resposta libera o endpoint de permissão.

Regras:

- Token curto único (`8F2A`), expira, é one-shot e idempotente.
- Primeiro canal que responde vence; os demais recebem ACK.
- Respostas: `APROVAR`, `REJEITAR`, `1/2/3`, `OK <token>`, texto ou anexo.
- Áudio no Signal é transcrito por whisper-cli e vira feedback.
- Timeout: `PAUSE_AND_ALERT`, `AUTO_APPROVE` ou `ABORT`.

## 4. E-mail (SMTP + IMAP)

- Envio via `lettre`, multipart text/plain + HTML.
- Subject com tag `[OD-#8F2A]`.
- Header `Message-ID: <hitl-8F2A@opendesk.local>`.
- Resposta detectada por `In-Reply-To` ou tag no subject.
- Whitelist de aprovadores por workspace.
- Remover citações (`>` e `On ... wrote:`) antes de interpretar.
- Anexos novos: salvar em `input/hitl/<token>/`, validar path e substituir o
  artefato alvo com registro em `artifacts_provenance`.
- IMAP IDLE real; fallback de reconexão/poll se o servidor derrubar IDLE.

## 5. Signal

- `signal-cli` local via daemon/JSON-RPC/stdio; sem abrir portas.
- Envio com anexos e timestamp externo persistido.
- Entrada por allowlist de números/UUIDs.
- Suporte a reply/quote, tapback 👍/👎/🔄, código curto, texto e áudio.
- O listener atual (SignalBridge) deve ser estendido, não duplicado.
- ACK após resolução.

## 6. Traits Rust

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HitlResolution {
    Approved,
    Rejected { reason: Option<String> },
    ChoiceSelected { option_index: usize, option_value: String },
    FeedbackProvided { feedback_text: String, attached_files: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlRequestPayload {
    pub token: String,
    pub run_id: String,
    pub workspace_id: String,
    pub interaction_type: String,
    pub title: String,
    pub message_body: String,
    pub options: Vec<String>,
    pub attachments: Vec<String>,
}

#[async_trait]
pub trait HitlChannelAdapter: Send + Sync {
    fn channel_id(&self) -> &'static str;
    async fn dispatch_request(
        &self,
        req: &HitlRequestPayload,
        config: &serde_json::Value,
    ) -> Result<String, String>;
    async fn send_resolution_ack(
        &self,
        external_msg_id: &str,
        summary: &str,
        config: &serde_json::Value,
    ) -> Result<(), String>;
}

#[async_trait]
pub trait TriggerAdapter: Send + Sync {
    fn trigger_type(&self) -> &'static str;
    async fn start(&self, ctx: RecipeContext) -> Result<(), String>;
}

#[async_trait]
pub trait SinkAdapter: Send + Sync {
    fn sink_type(&self) -> &'static str;
    async fn deliver(&self, ctx: RecipeContext, run: &RecipeRun) -> Result<(), String>;
}
```

## 7. Schema SQLite

O documento original numerava `0003_recipes_and_hitl.sql`. No OpenDesk atual a
próxima migração é **`0017_recipes_hitl.sql`**.

```sql
CREATE TABLE recipes_index (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    content_sha256 TEXT,
    trigger_type TEXT NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT 1,
    last_error TEXT,
    last_run_at TEXT,
    total_runs INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE recipe_runs (
    id TEXT PRIMARY KEY,
    recipe_id TEXT NOT NULL REFERENCES recipes_index(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    status TEXT NOT NULL CHECK(status IN (
        'RUNNING_STEP', 'WAITING_HITL', 'RUNNING_SINKS',
        'COMPLETED', 'REJECTED', 'FAILED', 'TIMED_OUT'
    )),
    current_step_id TEXT,
    trigger_context_json TEXT NOT NULL,
    step_outputs_json TEXT NOT NULL DEFAULT '{}',
    output_dir_path TEXT NOT NULL,
    error_message TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    completed_at TEXT
);

CREATE TABLE hitl_requests (
    token TEXT PRIMARY KEY,
    recipe_run_id TEXT NOT NULL REFERENCES recipe_runs(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    checkpoint_id TEXT NOT NULL,
    interaction_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK(status IN ('PENDING','RESOLVED','EXPIRED','CANCELLED')),
    dispatched_channels_json TEXT NOT NULL,
    resolved_by_channel TEXT,
    resolved_by_sender TEXT,
    resolution_type TEXT,
    resolution_payload TEXT,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    resolved_at TEXT
);
```

Índices: `recipes_index(workspace_id,is_enabled)`, `recipe_runs(recipe_id,started_at)`,
`recipe_runs(status)`, `hitl_requests(status,expires_at)`,
`hitl_requests(recipe_run_id)`.

## 8. Motor

- Parser `serde_yaml` + validação de schema (IDs únicos, steps válidos,
  checkpoints referenciam step existente, sinks válidos, variáveis conhecidas).
- Watch de `.opencode/recipes/`; supressão de eco já existente.
- `recipes_index` espelha metadados, mas o YAML é a verdade.
- Criar um card no Kanban por execução (`tasks`), com tag `recipe:<id>`.
- Steps sequenciais usando o **mesmo pipeline de fila/sessão** existente:
  `dispatch_task_internal`/`queue`/`session_runner`, sem bypass.
- Reutilizar `task_runs.session_id` entre steps para continuidade.
- `output_dir_path` determinístico `<workspace>/output/YYYY-MM/<recipe-slug>-<run>`.
- Variáveis resolvidas antes de enfileirar; desconhecidas falham cedo.
- Estado persistido; ao reiniciar, retomar runs `RUNNING_STEP` ou
  `WAITING_HITL` sem duplicar.
- `RunFinished`/`RunStreamChunk` alimentam a máquina; nunca polling cego de LLM.

## 9. Editor dual-mode

- Página **Receitas** no menu.
- `RecipeStudio` em duas colunas:
  - Stepper vertical: Trigger, Steps reordenáveis, HITL, Sinks.
  - Monaco Editor lazy-loaded (fallback textarea) com YAML.
- Two-way binding com `yaml` Document API para preservar comentários.
- Debounce de persistência ~400ms; escuta externa via watcher.
- Validação em tempo real; erros por caminho YAML.
- Dry-run: simula trigger/step/HITL e acende estados 🟡/🔴/🟢.
- Attention Card e Kanban Review exibem `WAITING_HITL` com aprovar/rejeitar e
  feedback.
- Drag-and-drop vertical apenas para steps/checkpoints; sem canvas X/Y.

## 10. Rsync

Adicionar suporte a Rsync para pastas locais e remotas:

- Sink `RSYNC_FS`:
  ```json
  {
    "direction": "push",
    "destination": "user@host:/srv/entregas",
    "ssh_key_ref": "rsync-key",
    "port": 22,
    "delete": false,
    "checksum": true
  }
  ```
- Inbound `RSYNC_FOLDER`:
  ```json
  {
    "source": "user@host:/srv/inbox",
    "path": "input/dropzone",
    "ssh_key_ref": "rsync-key",
    "interval_secs": 300
  }
  ```
- Usar `rsync -a --protect-args`, `--delay-updates`, `--partial-dir`,
  `--checksum`, `--timeout`; `--delete` só explícito.
- Local ou remoto: mesma implementação; `host` vazio = local.
- Nunca shell; `Command::arg`; chave SSH via SecretStore; strict host key.
- `rsync` entra no catálogo de ferramentas externas, com detecção e instalação
  opcional; nunca embutido no release.
- Sink e inbound entram na migração 0016 (ou 0017 se 0016 já estiver aplicada)
  com rebuild dos CHECKs.

## 11. Sprints

1. Parser YAML + migration 0017 + engine de steps + watcher.
2. HITL manager + timeout + Desktop UI + Approval Card.
3. E-mail SMTP/IMAP-IDLE + Signal replies/anexos/áudio.
4. RecipeStudio dual-mode + página Receitas + dry-run.
5. Rsync sink/inbound + ferramenta externa + testes.

## 12. Critérios de aceite

- Receita editada fora do app aparece na UI e é validada.
- Step, HITL e sinks sobrevivem a restart sem duplicar execução.
- Token HITL é one-shot e o primeiro canal vence.
- E-mail e Signal resolvem o mesmo checkpoint.
- Anexo editado substitui o artefato com proveniência.
- Dry-run não envia e-mail/Signal nem altera artefatos reais.
- Rsync funciona local e remoto; `--delete` é opt-in.
- Nenhum segredo em YAML, logs ou eventos.
