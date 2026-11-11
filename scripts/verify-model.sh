#!/usr/bin/env bash
# Opt-in real model verification. Starts a headless OpenDesk with a temporary
# data directory, creates a workspace and runs one minimal stateless generation
# through the same endpoint used by the "Testar (mínimo)" button.
#
#   OPENDESK_VERIFY_MODEL=deepseek/deepseek-flash npm run verify:model
#
# It consumes a handful of provider tokens.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PORT="${OPENDESK_VERIFY_PORT:-4399}"
MODEL="${OPENDESK_VERIFY_MODEL:-deepseek/deepseek-flash}"
MANUAL_MODEL="${OPENDESK_VERIFY_MANUAL_MODEL:-$MODEL}"
TMP="$(mktemp -d /tmp/opendesk-verify-model.XXXXXX)"
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
OPENDESK_ALLOW_TEMP_WORKSPACES=1 \
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

STATE=""
for _ in $(seq 1 90); do
  STATE="$(curl -s -H "$AUTH" "$BASE/api/engine/status" | jq -r '.daemon.state')"
  [[ "$STATE" == "online" ]] && break
  if [[ "$STATE" == "offline" ]]; then
    echo "verify:model: engine offline" >&2
    tail -60 "$TMP/app.log" >&2
    exit 1
  fi
  sleep 2
done
[[ "$STATE" == "online" ]] || {
  echo "verify:model: engine did not become online" >&2
  exit 1
}

WORKSPACE="$(curl -s -X POST -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"name\":\"Verify model\",\"root_path\":\"$TMP/ws\"}" "$BASE/api/workspaces")"
WSID="$(echo "$WORKSPACE" | jq -r '.id // empty')"
[[ -n "$WSID" ]] || {
  echo "verify:model: workspace creation failed: $WORKSPACE" >&2
  exit 1
}

RESULT="$(curl -s -X POST -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"workspace_id\":\"$WSID\",\"model\":\"$MODEL\"}" \
  "$BASE/api/models/test")"
echo "verify:model: $RESULT"
echo "$RESULT" | jq -e '.ok == true' >/dev/null || {
  echo "verify:model: model test failed" >&2
  exit 1
}

SAVED="$(curl -s -X PUT -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"workspace_id\":\"$WSID\",\"default_model\":\"$MODEL\",\"manual_model\":\"$MANUAL_MODEL\"}" \
  "$BASE/api/router/models")"
echo "$SAVED" | jq -e --arg model "$MODEL" '.default_model == $model' >/dev/null || {
  echo "verify:model: could not persist router models: $SAVED" >&2
  exit 1
}

ROUTED="$(curl -s -X POST -H "$AUTH" -H 'content-type: application/json' \
  -d "{\"workspace_id\":\"$WSID\",\"title\":\"Relatório\",\"description\":\"Analise os dados.\",\"tags\":[\"relatorio\"]}" \
  "$BASE/api/router/test")"
echo "verify:model: routing=$ROUTED"
BASE_MODEL="${MODEL%%#*}"
echo "$ROUTED" | jq -e --arg model "$BASE_MODEL" \
  '.default_model == $model and (.decision.selected_model | startswith($model))' >/dev/null || {
  echo "verify:model: router did not use the configured default model" >&2
  exit 1
}

echo "VERIFY MODEL OK ($MODEL)"
