# Contract tests

Contract tests are **opt-in** and never run inside `scripts/gate.sh`, because
they need a real OpenCode V2 daemon and, optionally, real Google/SMTP
credentials.

## OpenCode V2

With the app or `opendesk --headless` running:

```bash
npm run contract:opencode
```

The script reads `api.json` from `OPENDESK_DATA_DIR` (default
`~/.local/share/opendesk`), authenticates with the local Bearer token and
asserts:

- `/api/health` reports `ok` and a version;
- `/api/meta` declares `opencode_api: "v2"`;
- the daemon is online;
- `/api/agents` and `/api/models` return runtime lists for the active workspace;
- `/api/studio/agents` returns the workspace-scoped Studio list.

If the daemon is offline and you only want to validate the local API shape:

```bash
OPENDESK_CONTRACT_ALLOW_OFFLINE=1 npm run contract:opencode
```

## Google (opt-in, read-only)

With Google connected in **Integrações → Google**, run:

```bash
OPENDESK_CONTRACT_GOOGLE=1 npm run contract:google
```

The script reads `api.json`, checks `/api/integrations/google/status`, then
invokes the `opendesk mcp-google` bridge and calls `gdrive_search_files` with a
read-only query. It never creates or deletes documents. Use a dedicated account
for CI-like runs.

Optional variables:

- `OPENDESK_DATA_DIR`
- `OPENDESK_CONTRACT_WORKSPACE_ID`
- `OPENDESK_CONTRACT_BIN`

## SMTP

SMTP remains manual because it sends real mail:

1. Configure SMTP and use **Integrações → SMTP → Testar**.
2. For delivery, run a task with `notify_email` and confirm the record in
   **Integrações → Histórico**.

The default E2E suite already covers SMTP against a local fake server; this
manual step is only for real providers.
