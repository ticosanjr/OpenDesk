#!/usr/bin/env bash
# Keep the repository small. Build artifacts are reproducible and are never
# needed after a release has copied its assets into releases/<version>/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-}"
BEFORE="$(du -sk . 2>/dev/null | awk '{print $1}')"

echo "==> Removing build and test artifacts"
rm -rf \
  src-tauri/target \
  dist \
  coverage \
  test-results \
  playwright-report \
  blob-report \
  .vite \
  node_modules/.vite

find . -type f \( -name '*.profraw' -o -name '*.profdata' -o -name 'lcov.info' -o -name '*.tmp' -o -name '.DS_Store' \) -delete 2>/dev/null || true

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
printf '==> Clean done: freed %.1f MB\n' "$(echo "$FREED / 1024" | bc -l 2>/dev/null || echo 0)"
du -sh . releases node_modules 2>/dev/null || true
