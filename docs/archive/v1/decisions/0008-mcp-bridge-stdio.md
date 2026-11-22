# ADR 0008 — MCP bridge stdio

- **Status:** aceito
- **Data:** 2026-11-06
- **Contexto:** o OpenCode V2 conecta servidores MCP locais por stdio. O
  OpenDesk precisa expor as ferramentas Google aos agentes durante o loop de
  inferência sem duplicar a implementação HTTP.

## Decisão

O próprio binário `opendesk` aceita `mcp-google` e fala JSON-RPC 2.0 por
newline-delimited JSON em stdin/stdout. Stdout é reservado para frames; logs vão
para stderr.

O bridge carrega `OPENDESK_DATA_DIR`, `OPENDESK_CONFIG_DIR` e
`OPENDESK_WORKSPACE_ID`, abre o SQLite e usa o mesmo `GoogleApi`/`GoogleOAuth`
do backend. Ferramentas expostas:

- `gmail_search_threads`, `gmail_create_draft`, `gmail_send_email`
- `gdocs_read_document`, `gdocs_create_from_markdown`, `gdocs_append_text`
- `gdrive_search_files`, `gdrive_upload_file`, `gdrive_create_folder`
- `gcalendar_list_events`, `gcalendar_create_event`
- `gtasks_list`, `gtasks_create`
- `gkeep_list_notes`, `gkeep_create_note` (retornam erro explicativo; Keep é
  enterprise)

O endpoint `POST /api/integrations/google/mcp/register` grava
`mcp.servers.opendesk-google` no `opencode.jsonc` do workspace com
`command: [current_exe, "mcp-google"]` e as variáveis de ambiente necessárias.

## Consequências

- O agente pode usar Gmail/Docs/Drive/Calendar/Tasks no mesmo loop de execução.
- O bridge precisa de um workspace conectado; sem OAuth, as ferramentas
  retornam erro de “não conectado”.
- O executável precisa estar disponível no caminho registrado; em dev é o
  binário de `target/debug`.
- O protocolo é compatível com MCP clássico (initialize/tools/list/tools/call);
  recursos/prompts podem ser adicionados depois.
