# ADR 0028 — Barramento Universal de I/O (UEN)

- **Status:** aceito
- **Data:** 2026-11-07

## Decisão

- Inbounds e outbounds passam por `NormalizedTaskEvent` e `OutboundSink`.
- Tabelas `io_inbound_sources`, `io_outbound_sinks` e `task_sink_bindings`
  (migração 0016).
- Credenciais nunca ficam em `config_json`; usar `SecretStore` com `*_ref`.
- Webhook único em `POST /api/webhooks/{source_id}` com HMAC opcional,
  JSONPath e idempotência por `UUIDv5(source_id + external_id)`.
- Inbound workers: dropzone/OCR/áudio, IMAP, SQL poller, Telegram, Matrix.
- Sinks: DOC_COMPILER (XLSX/PDF), ATOMIC_FS, MINIO_S3, GIT_COMMIT, SQL_INSERT,
  SMTP, Telegram, Matrix, Mattermost.
- Ferramentas externas opcionais ficam em `~/.local/share/opendesk/tools`,
  detectadas/reutilizadas e nunca redistribuídas no pacote.
- Sinks rodam após `RunFinished` e não alteram o status do run.

## Consequências

- Um único caminho para criar tarefas e um único caminho para entregar.
- Novas integrações exigem adapter, não um novo core.
- Falhas de sink são isoladas e visíveis.
