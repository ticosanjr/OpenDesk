#!/usr/bin/env bash
# Opt-in Google contract test. Requires a running OpenDesk with Google already
# connected for a workspace. It only performs read-only calls (Drive search),
# so it never creates or deletes documents.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${OPENDESK_CONTRACT_GOOGLE:-0}" != "1" ]]; then
  cat <<'MSG'
contract:google é opt-in.
Configure o Google em Integrações → Google e rode:
  OPENDESK_CONTRACT_GOOGLE=1 npm run contract:google
Variáveis opcionais:
  OPENDESK_DATA_DIR=<diretório do app>
  OPENDESK_CONTRACT_WORKSPACE_ID=<id do workspace>
  OPENDESK_CONTRACT_BIN=<binário do OpenDesk>
MSG
  exit 0
fi

DATA_DIR="${OPENDESK_DATA_DIR:-$HOME/.local/share/opendesk}"
API_FILE="$DATA_DIR/api.json"
if [[ ! -f "$API_FILE" ]]; then
  echo "contract: OpenDesk não está em execução ($API_FILE ausente)" >&2
  exit 2
fi

PORT="$(python3 -c "import json;print(json.load(open('$API_FILE'))['port'])")"
TOKEN="$(python3 -c "import json;print(json.load(open('$API_FILE'))['token'])")"
BASE="http://127.0.0.1:$PORT"
AUTH="Authorization: Bearer $TOKEN"
get() { curl -fsS -H "$AUTH" "$BASE$1"; }

WORKSPACE_ID="${OPENDESK_CONTRACT_WORKSPACE_ID:-}"
if [[ -z "$WORKSPACE_ID" ]]; then
  WORKSPACE_ID="$(get /api/workspaces | python3 -c "import json,sys; rows=json.load(sys.stdin); print(rows[0]['id'] if rows else '')")"
fi
if [[ -z "$WORKSPACE_ID" ]]; then
  echo "contract: nenhum workspace cadastrado" >&2
  exit 2
fi

STATUS="$(get "/api/integrations/google/status?workspace_id=$WORKSPACE_ID")"
python3 - "$STATUS" <<'PY'
import json, sys
status = json.loads(sys.argv[1])
assert status.get("connected") is True, f"Google não conectado: {status}"
scopes = status.get("scopes") or []
print(f"contract: Google conectado · scopes={','.join(scopes) or 'não informados'}")
PY

BIN="${OPENDESK_CONTRACT_BIN:-}"
if [[ -z "$BIN" ]]; then
  if [[ -x "$ROOT/src-tauri/target/debug/opendesk" ]]; then
    BIN="$ROOT/src-tauri/target/debug/opendesk"
  else
    BIN="opendesk"
  fi
fi

printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"gdrive_search_files","arguments":{"query":"trashed = false"}}}' \
  | OPENDESK_WORKSPACE_ID="$WORKSPACE_ID" OPENDESK_DATA_DIR="$DATA_DIR" "$BIN" mcp-google \
  > /tmp/opendesk-google-contract.out

python3 - <<'PY'
import json
path = "/tmp/opendesk-google-contract.out"
messages = [json.loads(line) for line in open(path, encoding="utf-8") if line.strip()]
assert messages and messages[0]["id"] == 1, messages[:1]
tools = next(item for item in messages if item.get("id") == 2)["result"]["tools"]
assert any(tool.get("name") == "gdrive_search_files" for tool in tools), tools
result = next(item for item in messages if item.get("id") == 3)["result"]
assert result.get("isError") is False, result
text = result["content"][0]["text"]
print(f"contract: Drive search respondeu ({len(text)} bytes de JSON)")
PY

rm -f /tmp/opendesk-google-contract.out
echo "contract: Google OK"
