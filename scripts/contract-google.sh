#!/usr/bin/env bash
# Opt-in Google contract test against a dedicated account. Requires a running
# OpenDesk with Google connected for a workspace. Calls only read-only tools,
# except gdocs_read_document which needs a document shared with the account.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${OPENDESK_CONTRACT_GOOGLE:-0}" != "1" ]]; then
  cat <<'MSG'
contract:google é opt-in.
Conecte uma conta dedicada em Integrações → Google e rode:
  OPENDESK_CONTRACT_GOOGLE=1 npm run contract:google
Variáveis:
  OPENDESK_CONTRACT_DOC_ID=<id de um Google Doc legível pela conta>
  OPENDESK_CONTRACT_SKIP=gmail,calendar,tasks,docs   # opcional
  OPENDESK_CONTRACT_WORKSPACE_ID=<id do workspace>   # opcional
  OPENDESK_DATA_DIR=<diretório do app>               # opcional
  OPENDESK_CONTRACT_BIN=<binário do OpenDesk>        # opcional
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
required = [
    "https://www.googleapis.com/auth/drive.file",
    "https://www.googleapis.com/auth/gmail.send",
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/calendar.readonly",
    "https://www.googleapis.com/auth/tasks",
]
missing = [scope for scope in required if scope not in scopes]
assert not missing, f"scopes ausentes: {missing}"
print(f"contract: Google conectado · {len(scopes)} scopes")
PY

BIN="${OPENDESK_CONTRACT_BIN:-}"
if [[ -z "$BIN" ]]; then
  if [[ -x "$ROOT/src-tauri/target/debug/opendesk" ]]; then
    BIN="$ROOT/src-tauri/target/debug/opendesk"
  else
    BIN="opendesk"
  fi
fi

SKIP="${OPENDESK_CONTRACT_SKIP:-}"
skip() { [[ ",$SKIP," == *",$1,"* ]]; }

DOC_ID="${OPENDESK_CONTRACT_DOC_ID:-}"
NOW_MIN="$(python3 -c "import datetime;print((datetime.datetime.now(datetime.timezone.utc)-datetime.timedelta(days=30)).strftime('%Y-%m-%dT%H:%M:%SZ'))")"
NOW_MAX="$(python3 -c "import datetime;print((datetime.datetime.now(datetime.timezone.utc)+datetime.timedelta(days=30)).strftime('%Y-%m-%dT%H:%M:%SZ'))")"

OUT="$(mktemp)"
trap 'rm -f "$OUT"' EXIT
{
  printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
  printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"gdrive_search_files","arguments":{"query":"trashed = false"}}}'
  if ! skip gmail; then
    printf '%s\n' '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"gmail_search_threads","arguments":{"query":""}}}'
  fi
  if ! skip calendar; then
    printf '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"gcalendar_list_events","arguments":{"time_min":"%s","time_max":"%s"}}}\n' "$NOW_MIN" "$NOW_MAX"
  fi
  if ! skip tasks; then
    printf '%s\n' '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"gtasks_list","arguments":{}}}'
  fi
  if ! skip docs && [[ -n "$DOC_ID" ]]; then
    printf '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"gdocs_read_document","arguments":{"document_id":"%s"}}}\n' "$DOC_ID"
  fi
} | OPENDESK_WORKSPACE_ID="$WORKSPACE_ID" OPENDESK_DATA_DIR="$DATA_DIR" "$BIN" mcp-google > "$OUT"

python3 - "$OUT" "$SKIP" "$DOC_ID" <<'PY'
import json
import sys

path, skip_raw, doc_id = sys.argv[1], sys.argv[2], sys.argv[3]
skip = set(filter(None, skip_raw.split(",")))
messages = [json.loads(line) for line in open(path, encoding="utf-8") if line.strip()]
assert messages and messages[0].get("id") == 1, messages[:1]
assert "result" in messages[0], messages[0]

tools = next(item for item in messages if item.get("id") == 2)["result"]["tools"]
names = {tool.get("name") for tool in tools}
expected = {"gdrive_search_files", "gmail_search_threads", "gcalendar_list_events", "gtasks_list", "gdocs_read_document"}
assert expected <= names, f"ferramentas ausentes: {sorted(expected - names)}"
print(f"contract: {len(names)} ferramentas Google listadas")


def call(call_id, label):
    message = next(item for item in messages if item.get("id") == call_id)
    result = message["result"]
    if result.get("isError"):
        detail = result.get("content", [{}])[0].get("text", "")
        raise AssertionError(f"{label} falhou: {detail[:400]}")
    text = result.get("content", [{}])[0].get("text", "")
    print(f"contract: {label} OK ({len(text)} bytes)")
    return text


drive = call(3, "Drive search")
assert '"files"' in drive or drive.strip() == "null" or drive.strip() == "{}", drive[:200]

if "gmail" not in skip:
    call(4, "Gmail search")
if "calendar" not in skip:
    call(5, "Calendar list")
if "tasks" not in skip:
    call(6, "Tasks list")
if "docs" not in skip:
    if doc_id:
        text = call(7, "Docs read")
        assert text.strip(), "documento vazio"
    else:
        print("contract: Docs read ignorado (defina OPENDESK_CONTRACT_DOC_ID)")
PY

echo "contract: Google OK"
