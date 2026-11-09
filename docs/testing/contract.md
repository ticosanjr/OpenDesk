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

## Google / SMTP

These stay manual because they send real data:

1. Configure Google in **Integrações → Google** and run the OAuth flow.
2. Verify `/api/integrations/google/status` reports `connected: true` and the
   configured scopes.
3. Configure SMTP and use **Integrações → SMTP → Testar**.
4. For delivery, run a task with `export_to_gdocs` and confirm the delivery
   record in **Integrações → Histórico**.

A future automated contract test should use a dedicated test account and a
disposable Drive folder; do not run it against production data.
