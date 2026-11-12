#!/usr/bin/env bash
# Fast day-to-day gate. Catches type, lint, unit-test and formatting regressions
# without coverage, Playwright, release builds or the full Rust test suite.
#
# Usage:
#   scripts/gate-fast.sh            # frontend + Rust formatting
#   scripts/gate-fast.sh --rust     # also clippy and Rust unit tests
#   scripts/gate-fast.sh --e2e      # also the Playwright smoke/scroll suite
#
# The full release gate remains `scripts/gate.sh` and only runs on request.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WITH_RUST=0
WITH_E2E=0
for argument in "$@"; do
  case "$argument" in
    --rust) WITH_RUST=1 ;;
    --e2e) WITH_E2E=1 ;;
    --help|-h)
      sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown argument: $argument" >&2
      exit 2
      ;;
  esac
done

started="$(date +%s)"

echo "==> Whitespace"
git diff --check

echo "==> Rust formatting"
(cd src-tauri && cargo fmt -- --check)

echo "==> TypeScript"
npm run check

echo "==> ESLint"
npm run lint

echo "==> Vitest"
npm run test:ui

if [[ "$WITH_RUST" == "1" ]]; then
  echo "==> Clippy (-D warnings)"
  (cd src-tauri && cargo clippy --all-targets -- -D warnings)
  echo "==> Rust tests"
  npm run test:rust
fi

if [[ "$WITH_E2E" == "1" ]]; then
  echo "==> Playwright smoke"
  npx playwright test tests/e2e/scroll.spec.ts tests/e2e/a11y.spec.ts
fi

elapsed="$(( $(date +%s) - started ))"
echo "==> Fast gate passed in ${elapsed}s"
