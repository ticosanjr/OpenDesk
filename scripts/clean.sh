#!/usr/bin/env bash
# Keep the repository small without throwing away the incremental build cache.
# `--all` removes src-tauri/target; `--deep` also removes node_modules.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-}"
BEFORE="$(du -sk . 2>/dev/null | awk '{print $1}')"

echo "==> Removing transient build and test artifacts"
rm -rf \
  dist \
  coverage \
  test-results \
  playwright-report \
  blob-report \
  .vite \
  node_modules/.vite

find . -type f \( -name '*.profraw' -o -name '*.profdata' -o -name 'lcov.info' -o -name '*.tmp' -o -name '.DS_Store' \) -delete 2>/dev/null || true

if [[ "$MODE" == "--all" || "$MODE" == "--deep" ]]; then
  echo "==> Removing src-tauri/target (next build will be cold)"
  rm -rf src-tauri/target
fi

if [[ "$MODE" == "--deep" ]]; then
  echo "==> Removing node_modules (run npm install before developing again)"
  rm -rf node_modules
fi

# Keep at most the three most recent releases locally.
echo "==> Keeping at most 3 releases"
mapfile -t versions < <(
  find releases -mindepth 1 -maxdepth 1 -type d -printf '%f\n' 2>/dev/null | sort -V
)
if (( ${#versions[@]} > 3 )); then
  keep_from=$(( ${#versions[@]} - 3 ))
  for old in "${versions[@]:0:keep_from}"; do
    echo "    prune releases/$old"
    rm -rf "releases/$old"
  done
fi

AFTER="$(du -sk . 2>/dev/null | awk '{print $1}')"
FREED=$(( BEFORE - AFTER ))
printf '==> Clean done: freed %d MB\n' "$((FREED / 1024))"
du -sh . releases node_modules 2>/dev/null || true
