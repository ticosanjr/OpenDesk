#!/usr/bin/env bash
# Opt-in contract test against a running OpenDesk + OpenCode V2 daemon.
# This is intentionally not part of `gate.sh`: it needs a real engine.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA_DIR="${OPENDESK_DATA_DIR:-$HOME/.local/share/opendesk}"
API_FILE="$DATA_DIR/api.json"
ALLOW_OFFLINE="${OPENDESK_CONTRACT_ALLOW_OFFLINE:-0}"

if [[ ! -f "$API_FILE" ]]; then
  echo "contract: OpenDesk não está em execução ($API_FILE ausente)" >&2
  echo "contract: abra o app ou rode 'opendesk --headless'" >&2
  exit 1
fi

PORT="$(python3 -c "import json;print(json.load(open('$API_FILE'))['port'])")"
TOKEN="$(python3 -c "import json;print(json.load(open('$API_FILE'))['token'])")"
BASE="http://127.0.0.1:$PORT"

get() {
  curl -fsS -H "Authorization: Bearer $TOKEN" "$BASE$1"
}

HEALTH="$(get /api/health)"
META="$(get /api/meta)"
DAEMON="$(get /api/daemon/status)"

python3 - "$HEALTH" "$META" <<'PY'
import json, sys
health = json.loads(sys.argv[1])
meta = json.loads(sys.argv[2])
assert health.get("ok") is True, health
assert meta.get("name") == "OpenDesk", meta
assert meta.get("opencode_api") == "v2", meta
print(f"contract: OpenDesk {health.get('version')} · API {meta.get('opencode_api')} OK")
PY

ONLINE="$(python3 -c "import json,sys;print(str(json.loads(sys.argv[1]).get('online')).lower())" "$DAEMON")"
if [[ "$ONLINE" != "true" ]]; then
  if [[ "$ALLOW_OFFLINE" == "1" ]]; then
    echo "contract: daemon offline (permitido por OPENDESK_CONTRACT_ALLOW_OFFLINE=1)"
    exit 0
  fi
  echo "contract: daemon OpenCode offline; contract test requer o motor real" >&2
  echo "contract: use OPENDESK_CONTRACT_ALLOW_OFFLINE=1 para validar só a API local" >&2
  exit 2
fi

WORKSPACES="$(get /api/workspaces)"
ACTIVE="$(get /api/settings/active_workspace_id || true)"
readarray -t PAIR < <(python3 - "$WORKSPACES" "$ACTIVE" <<'PY'
import json, sys
workspaces = json.loads(sys.argv[1])
active = json.loads(sys.argv[2]) if sys.argv[2].strip() else None
if not workspaces:
    raise SystemExit("contract: nenhum workspace cadastrado")
chosen = next((w for w in workspaces if w.get("id") == active), workspaces[0])
print(chosen["id"])
print(chosen["root_path"])
PY
)
WORKSPACE_ID="${PAIR[0]}"
WORKSPACE_ROOT="${PAIR[1]}"

AGENTS="$(get "/api/agents?directory=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$WORKSPACE_ROOT")")"
MODELS="$(get "/api/models?directory=$(python3 -c "import urllib.parse,sys;print(urllib.parse.quote(sys.argv[1]))" "$WORKSPACE_ROOT")")"
SCoped="$(get "/api/studio/agents?scope=workspace&workspace_id=$WORKSPACE_ID")"

python3 - "$AGENTS" "$MODELS" "$SCoped" "$WORKSPACE_ID" <<'PY'
import json, sys
agents = json.loads(sys.argv[1])
models = json.loads(sys.argv[2])
studio = json.loads(sys.argv[3])
assert isinstance(agents.get("data"), list), agents
assert isinstance(models.get("data"), list), models
assert isinstance(studio, list), studio
print(
    f"contract: workspace {sys.argv[4]} · "
    f"{len(agents.get('data', []))} agentes runtime · "
    f"{len(models.get('data', []))} modelos · "
    f"{len(studio)} agentes no Studio"
)
PY
echo "contract: OK"
