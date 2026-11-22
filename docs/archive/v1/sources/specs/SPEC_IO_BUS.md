# Especificação Incremental: Barramento Universal de I/O e Gatilhos (`SPEC_IO_BUS.md`)

> **Status no repositório:** documento fonte arquivado no plano. Na implementação,
> copiar para `docs/specs/SPEC_IO_BUS.md` e criar o ADR `0028-io-bus-uen.md`.
>
> **Instrução para o Agente Desenvolvedor:** Implemente este subsistema utilizando
> o padrão **Universal Event Normalization (UEN)**. Não construa integrações
> monolíticas para cada ferramenta externa. Reduza todos os modos de entrada a
> **4 adaptadores base em Rust** que convertem qualquer sinal externo em um
> `NormalizedTaskEvent` (Markdown + Metadados) para a fila do OpenDesk, e reduza
> todas as saídas a **5 despachantes pós-execução (Sinks)**.

---

## 1. Arquitetura Minimalista: O Padrão "Funil Único"

```
[ENTRADAS / GATILHOS]                   [NORMALIZADOR RUST]              [CORE]              [SAÍDAS / SINKS]

• Arquivos / Imagens / Áudio ──(notify)──▶ Pre-Processor CLI ──┐                          ┌──▶ FS Atômico / MinIO
  (csv, xlsx, pdf, wav, png)              (Typst/Whisper/OCR) │                          ├──▶ Chat Adapters (TG/Matrix/MM)
• E-mail Corporativo ────────(IMAP IDLE)─▶ MIME Parser ───────┼──▶ NormalizedTaskEvent ──┼──▶ SMTP Direto (lettre)
• Webhooks / Forms / Chat ───(Axum POST)─▶ JSON Mapper ───────┤    (Fila SQLite + DSH)   ├──▶ Git Auto-Commit (CLI)
  (Telegram, Matrix, Baserow)                                  │                          └──▶ SQL Sink (SQLite/Postgres)
• Bancos de Dados Externos ──(SQL Poll)──▶ Row Serializer ────┘
```

---

## 2. Requisitos Funcionais de Entrada (Ingestão e Triggers)

Todos os canais de entrada são gerenciados pelo módulo
`src-tauri/src/io_bus/inbounds/` usando apenas **4 motores nativos**.

### RF-IN-01: Unified Dropzone Watcher (Arquivos, Documentos, Imagens e Áudio)

Todo arquivo físico cai em pastas monitoradas (`<workspace>/input/dropzone/`)
via crate `notify`, com debounce de 500 ms e verificação de lock de escrita
concluída.

1. **Planilhas e Docs** (`.csv`, `.xlsx`, `.pdf`, `.docx`, `.json`, `.md`):
   - extrair texto/schema via `calamine` para Excel/CSV, `pdf-extract` para PDF
     texto, ou anexar o caminho bruto ao contexto da tarefa.
2. **Imagens e documentos digitalizados** (`.png`, `.jpg`, `.tiff`, PDF escaneado):
   - modo minimalista: `tesseract <file> stdout -l por+ita+eng`;
   - ou endpoint vision local compatível com OpenAI (`:11434/v1`, `:3100/v1`);
   - texto extraído vira o corpo da tarefa.
3. **Áudio e gravações** (`.wav`, `.mp3`, `.ogg` de Asterisk/FreePBX ou gravador):
   - modo minimalista: `whisper-cli` via `tokio::process::Command`;
   - transcrição vira prompt;
   - áudio original movido para `<workspace>/input/processed/`.

### RF-IN-02: Listener de E-mail Corporativo via IMAP IDLE

- `async-imap` + `mail-parser`.
- Conexão persistente (`IDLE`) por workspace/caixa (`tasks@dominio.local`,
  Mailcow/Dovecot).
- Extrai `Subject` como título, `From` para resposta futura, converte
  `text/plain`/`text/html` para Markdown, salva anexos em
  `<workspace>/input/attachments/<msg_id>/` e enfileira a tarefa.

### RF-IN-03: Gateway Universal de Webhooks e Mensageria

`POST /api/webhooks/{source_id}` (Axum polimórfico) recebe JSON de Formbricks,
Baserow, NocoDB, GLPI, Redmine, Mosquitto HTTP bridge e bots.

- **Mapeamento por JSONPath simples:** cada fonte define `title_path`,
  `body_path` e `priority_path`.
- **Chat:**
  - Telegram: webhook local ou long polling (`getUpdates`) via `reqwest`.
  - Matrix: `/sync` long-polling para `m.room.message`.
  - Mattermost: Outgoing Webhook/Slash Command via HTTP POST.

### RF-IN-04: SQL Polling & Trigger Listener

- SQL Poller leve via `sqlx` (SQLite, PostgreSQL, MySQL/MariaDB).
- Query configurada em intervalo regular:
  `SELECT * FROM erp_orders WHERE sync_status = 'PENDING' LIMIT 20`.
- Cada linha é serializada em JSON, vira tarefa, e a query de confirmação é
  executada:
  `UPDATE erp_orders SET sync_status = 'QUEUED' WHERE id = $1`.

---

## 3. Interação Bidirecional em Chat

Trait `ChatProvider` em `src-tauri/src/io_bus/chat_bridge.rs` com tradução de
botões interativos.

### RF-CHAT-01: Tradutor Unificado de Mensagens e Botões (`ActionCard`)

Estrutura interna:

```rust
ChatNotification {
    text_md: String,
    buttons: Vec<{ label: String, callback_action_id: String }>,
}
```

| Plataforma | Texto | Botões | Callback |
|---|---|---|---|
| Telegram | HTML | `InlineKeyboardMarkup`, `callback_data: "act:<task_id>:approve"` | `getUpdates`/webhook capturando `callback_query` |
| Matrix | `m.room.message` body + `formatted_body` | reações 🟢/🔴 ou `!approve <id>` | sync loop capturando `m.reaction`/`m.room.message` |
| Mattermost | Markdown/tabelas | `attachments.actions` para `/api/chat/callback` | HTTP POST no clique |

---

## 4. Sinks Determinísticos Pós-Execução

Quando o OpenCode finaliza uma tarefa (`SUCCESS`), o módulo
`src-tauri/src/io_bus/outbounds/` executa os sinks vinculados à tarefa/rotina,
sem depender do LLM.

### RF-OUT-01: Compilador Headless de Documentos

- PDF: `typst compile <input.typ> <output.pdf>`; Markdown/Typst → PDF.
- Excel: `.json` tabular ou `.csv` → `.xlsx` com `rust_xlsxwriter`
  (cabeçalhos congelados, largura automática), sem Python.
- CSV/DOCX: conversões determinísticas.

### RF-OUT-02: Escrita Atômica em FS (SMB/NFS/MinIO S3)

- Gravar em `<destino>/.tmp_<filename>` e `rename` atômico.
- S3/MinIO via `rust-s3`.

### RF-OUT-03: Git Auto-Commit & Branching

- `git checkout -b opendesk/task-<id>` (ou branch configurada).
- `git add` apenas arquivos de `artifacts_provenance`.
- commit:
  `chore(agent): <agent_name> completed <task_title> [run:<run_id>]`.
- push opcional para Gitea/Forgejo/local.

### RF-OUT-04: SQL Sink Analítico

- JSON validado por schema → `INSERT`/`UPSERT` em SQLite/PostgreSQL.
- Alimenta Grafana/Metabase.

### RF-OUT-05: SMTP e Mensageria Direta

- Relatório HTML + PDF/XLSX via `lettre`.
- Resumo/anexo em Telegram, Matrix ou Mattermost.

---

## 5. Schema SQLite Incremental

O documento original numerava `0002_io_bus_schema.sql`; no repositório atual a
próxima migração é **`0016_io_bus.sql`**.

```sql
CREATE TABLE io_inbound_sources (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    source_type TEXT CHECK(source_type IN (
        'FOLDER_WATCH', 'OCR_WATCH', 'AUDIO_STT_WATCH',
        'IMAP_IDLE', 'WEBHOOK_HTTP', 'SQL_POLLER',
        'TELEGRAM_BOT', 'MATRIX_BOT', 'MATTERMOST_BOT'
    )) NOT NULL,
    config_json TEXT NOT NULL,
    target_agent TEXT,
    target_column TEXT DEFAULT 'TODO',
    auto_execute BOOLEAN DEFAULT 1,
    is_enabled BOOLEAN DEFAULT 1,
    last_triggered_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE io_outbound_sinks (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    sink_type TEXT CHECK(sink_type IN (
        'ATOMIC_FS', 'MINIO_S3', 'SMTP_EMAIL',
        'TELEGRAM_MSG', 'MATRIX_MSG', 'MATTERMOST_MSG',
        'DOC_COMPILER', 'GIT_COMMIT', 'SQL_INSERT'
    )) NOT NULL,
    config_json TEXT NOT NULL,
    is_enabled BOOLEAN DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE task_sink_bindings (
    id TEXT PRIMARY KEY,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    schedule_id TEXT REFERENCES schedules(id) ON DELETE CASCADE,
    inbound_source_id TEXT REFERENCES io_inbound_sources(id) ON DELETE CASCADE,
    sink_id TEXT NOT NULL REFERENCES io_outbound_sinks(id) ON DELETE CASCADE,
    execution_order INTEGER DEFAULT 1
);
```

### Correções obrigatórias ao schema original

- `target_column DEFAULT 'PLAN'` → usar status Paperclip atual (`TODO`/`BACKLOG`);
  `PLAN` vira fase `plan`, não coluna.
- `config_json` **não pode** conter senhas/tokens. Guardar apenas referências
  (`secret_ref`, `token_ref`) e resolver no `SecretStore`.
- Adicionar índices em `workspace_id`, `source_type`, `is_enabled`,
  `(task_id, execution_order)`, `(schedule_id, execution_order)`.
- Adicionar `CHECK` de exclusividade em `task_sink_bindings`
  (`task_id` XOR `schedule_id`).
- Adicionar `UNIQUE` para impedir o mesmo sink duplicado no mesmo destino.
- Idempotência sem tabela extra: `task.id` determinístico por
  `source_id + external_id` (UUIDv5) e/ou `task_messages` com metadata.

---

## 6. Plano de Implementação Enxuto (4 Sprints)

### Etapa 1: Core do barramento e compiladores locais

1. Traits `InboundTrigger` e `OutboundSink` em `src-tauri/src/io_bus/mod.rs`.
2. `NormalizedTaskEvent` com Markdown + metadados.
3. Compilador nativo: `rust_xlsxwriter` e wrapper `typst`.
4. Escrita atômica e Git auto-commit.

### Etapa 2: Dropzone multimodal

1. Watcher `notify` em `input/dropzone/` com debounce 500 ms e lock de escrita.
2. Wrappers `tesseract` e `whisper-cli` com timeout/limites.
3. Saída de texto → `DispatchTaskRequest` no roteador DSH.

### Etapa 3: Webhooks, SQL Poller e mensageria interativa

1. `POST /api/webhooks/{source_id}` com `serde_json_path`.
2. SQL Poller e SQL Sink.
3. ChatBridge Telegram, Matrix e Mattermost com botões ligados a
   `BLOCKED`/`Plan → Build`.

### Etapa 4: IMAP IDLE e painel de conectores

1. Worker `async-imap`.
2. UI de Integrações:
   - **Gatilhos de Entrada (Inbounds)**
   - **Entregas Automáticas (Outbound Sinks)**
3. Checkboxes de sinks no card da tarefa e na rotina.

---

## 7. Observações de compatibilidade com o OpenDesk atual

- O endpoint deve ser `/api/webhooks/{source_id}`, não `/api/v1/...`
  (regra V2-only).
- O callback do Mattermost deve apontar para a API local do OpenDesk, não para
  `:3100` do OpenCode.
- O `target_column` deve usar os status atuais do Kanban.
- Sinks rodam depois de `RunFinished`/`deliver_run`, com idempotência própria.
- Nenhum sink pode bloquear a finalização do run; falhas viram delivery/retry.
- Credenciais sempre via `SecretStore`; nunca em `config_json` ou logs.
