#!/usr/bin/env bash
# Coverage report for both sides. The gate lives in scripts/gate.sh.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> Frontend logic coverage"
npm run test:coverage

echo "==> Rust coverage"
if command -v cargo-llvm-cov >/dev/null 2>&1; then
  (cd src-tauri && cargo llvm-cov --summary-only)
  echo "    HTML report: src-tauri/target/llvm-cov/html/index.html"
  (cd src-tauri && cargo llvm-cov report --html >/dev/null)
else
  echo "    cargo-llvm-cov not installed"
  echo "    rustup component add llvm-tools-preview"
  echo "    cargo install cargo-llvm-cov --locked"
fi
