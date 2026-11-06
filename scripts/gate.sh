#!/usr/bin/env bash
# Local release gate. There is no CI by design: `release.sh` runs this before
# building and publishing.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MIN_LINES="${OPENDESK_MIN_LINES:-40}"

echo "==> TypeScript"
npm run check

echo "==> Frontend tests + coverage gate (logic modules)"
npm run test:coverage

echo "==> Rust formatting"
(cd src-tauri && cargo fmt -- --check)

echo "==> Clippy (-D warnings)"
(cd src-tauri && cargo clippy --all-targets -- -D warnings)

echo "==> Rust tests"
npm run test:rust

echo "==> Frontend build"
npm run build

if command -v cargo-llvm-cov >/dev/null 2>&1; then
  echo "==> Rust coverage (fail under ${MIN_LINES}% lines)"
  (cd src-tauri && cargo llvm-cov --summary-only --fail-under-lines "$MIN_LINES")
else
  echo "    cargo-llvm-cov not installed; skipping the Rust coverage ratchet"
  echo "    install with: cargo install cargo-llvm-cov"
fi

if [[ "${OPENDESK_SKIP_E2E:-0}" != "1" ]]; then
  echo "==> Playwright E2E"
  npm run test:e2e
else
  echo "    Playwright skipped (OPENDESK_SKIP_E2E=1)"
fi

if [[ "${OPENDESK_REAL_TASK:-0}" == "1" ]]; then
  echo "==> Real OpenCode task verification"
  scripts/verify-real.sh
fi

echo "==> Gate passed"
