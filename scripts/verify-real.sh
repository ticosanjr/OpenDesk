#!/usr/bin/env bash
# Real OpenCode verification: a task must finish SUCCESS, move to REVIEW and
# produce an artifact. Opt-in because it consumes provider tokens.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT="${OPENDESK_VERIFY_PORT:-4399}"
MODEL="${OPENDESK_VERIFY_MODEL:-deepseek/deepseek-flash}"
PROMPT="${OPENDESK_VERIFY_PROMPT:-Use a ferramenta write para criar o arquivo output/verify.txt contendo exatamente VERIFY_OK. Depois responda apenas DONE.}"
TMP="$(mktemp -d /tmp/opendesk-verify.XXXXXX)"
BIN="${OPENDESK_VERIFY_BIN:-}"
if [[ -z "$BIN" ]]; then
  cargo build --manifest-path src-tauri/Cargo.toml >/dev/null
  BIN="$ROOT/src-tauri/target/debug/opendesk"
fi

cleanup() {
  if [[ -n "${APP_PID:-}" ]]; then
    for proc in /proc/[0-9]*; do
      environ="$(tr '\0' '\n' 2>/dev/null <"$proc/environ" || true)"
      [[ -n "$environ" ]] || continue
      if grep -q "^OPENDESK_PARENT_PID=$APP_PID$" <<<"$environ"; then
        kill "${proc##*/}" 2>/dev/null || true
      fi
    done
    kill "$APP_PID" 2>/dev/null || true
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT

OPENDESK_DATA_DIR="$TMP/data" \
OPENDESK_CONFIG_DIR="$TMP/config" \
OPENDESK_CACHE_DIR="$TMP/cache" \
XDG_STATE_HOME="$TMP/xdg-state" \
OPENDESK_API_PORT="$PORT" \
RUST_LOG=opendesk=info \
"$BIN" --headless >"$TMP/app.log" 2>&1 &
APP_PID=$!

for _ in $(seq 1 60); do
  sleep 1
  curl -sf "http://127.0.0.1:$PORT/healthz" >/dev/null && break
done
TOKEN="$(jq -r .token "$TMP/data/api.json")"
AUTH="Authorization: Bearer $TOKEN"
BASE="http://127.0.0.1:$PORT"

for _ in $(seq 1 60); do
  STATE="$(curl -s -H "$AUTH" "$BASE/api/engine/status" | jq -r '.daemon.state')"
  [[ "$STATE" == "online" ]] && break
  [[ "$STATE" == "offline" ]] && { echo "engine offline"; exit 1; }
  sleep 2
done

WORKSPACE="$(curl -s -X POST -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"name\":\"Verify\",\"root_path\":\"$TMP/ws\"}" "$BASE/api/workspaces")"
WSID="$(echo "$WORKSPACE" | jq -r .id)"

RUNTIME="$(curl -s -H "$AUTH" "$BASE/api/studio/runtime?workspace_id=$WSID")"
SKILLS="$(echo "$RUNTIME" | jq '.skills | length')"
COMMANDS="$(echo "$RUNTIME" | jq '.commands | length')"
MCPS="$(echo "$RUNTIME" | jq '.mcps | length')"
echo "runtime: skills=$SKILLS commands=$COMMANDS mcps=$MCPS"
(( SKILLS > 0 && COMMANDS > 0 && MCPS > 0 )) || { echo "runtime inventory empty"; exit 1; }

SKILL_ID="$(echo "$RUNTIME" | jq -r '.skills[0].id')"
IMPORTED="$(curl -s -X POST -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"kind\":\"skill\",\"id\":\"$SKILL_ID\",\"target_scope\":\"workspace\",\"target_workspace_id\":\"$WSID\"}" \
  "$BASE/api/studio/runtime/import")"
[[ "$(echo "$IMPORTED" | jq -r .ok)" == "true" ]] || { echo "runtime skill import failed: $IMPORTED"; exit 1; }
echo "runtime skill imported: $SKILL_ID"

TASK="$(curl -s -X POST -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"title\":\"Verify real\",\"description\":\"$PROMPT\",\"execution_mode\":\"BUILD_DIRECT\",\"priority\":2,\"tags\":[],\"effort_level\":\"low\"}" \
  "$BASE/api/tasks?workspace_id=$WSID")"
TID="$(echo "$TASK" | jq -r .id)"
curl -s -X PATCH -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"preferred_model\":\"$MODEL\",\"model_override\":true,\"assigned_agent\":\"build\",\"agent_override\":true}" \
  "$BASE/api/tasks/$TID" >/dev/null
curl -s -X POST -H "$AUTH" -H 'content-type: application/json' -d '{}' \
  "$BASE/api/tasks/$TID/dispatch" >/dev/null

STATUS=""
for _ in $(seq 1 300); do
  STATUS="$(curl -s -H "$AUTH" "$BASE/api/runs?workspace_id=$WSID&task_id=$TID" | jq -r '.[0].status // "none"')"
  [[ "$STATUS" == "SUCCESS" || "$STATUS" == "FAILED" ]] && break
  sleep 2
done
TASK_STATE="$(curl -s -H "$AUTH" "$BASE/api/tasks" | jq -r ".[] | select(.id==\"$TID\") | .column_stage + \"/\" + .phase_status")"
RUN_INFO="$(curl -s -H "$AUTH" "$BASE/api/runs?workspace_id=$WSID&task_id=$TID" | jq -c '.[0] | {status, duration_ms, model_id, error_message}')"
echo "run: $RUN_INFO"
ARTIFACTS="$(curl -s -H "$AUTH" "$BASE/api/artifacts?workspace_id=$WSID&limit=20" | jq '[.[] | select(.task_id=="'"$TID"'")] | length')"
echo "task: status=$STATUS state=$TASK_STATE artifacts=$ARTIFACTS"
[[ "$STATUS" == "SUCCESS" ]] || { echo "task failed"; exit 1; }
[[ "$TASK_STATE" == REVIEW/build_complete ]] || { echo "task did not move to REVIEW"; exit 1; }
(( ARTIFACTS > 0 )) || { echo "no artifact"; exit 1; }
echo "REAL VERIFY OK"
