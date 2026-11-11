#!/usr/bin/env bash
# Proportional verification for non-release work. The full gate (coverage +
# Playwright) is only required when the user asks for a release.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

changed="$(
  {
    git diff HEAD --name-only
    git ls-files --others --exclude-standard
  } | sort -u
)"

if [[ -z "$changed" ]]; then
  echo "No modified files; nothing to verify."
  exit 0
fi

rust=0
frontend=0
while IFS= read -r file; do
  case "$file" in
    src-tauri/*.rs|src-tauri/Cargo.toml|src-tauri/Cargo.lock) rust=1 ;;
    src/*|tests/*|package.json|package-lock.json|vite.config.ts|vitest.config.ts|playwright.config.ts|eslint.config.js|index.html) frontend=1 ;;
  esac
done <<< "$changed"

if [[ "$rust" == "1" ]]; then
  echo "==> Rust changed: fmt, clippy and unit tests"
  (cd src-tauri && cargo fmt -- --check)
  (cd src-tauri && cargo clippy --all-targets -- -D warnings)
  (cd src-tauri && cargo test)
fi

if [[ "$frontend" == "1" ]]; then
  echo "==> Frontend changed: lint, TypeScript and Vitest"
  npm run lint
  npm run check
  npm run test:ui
fi

echo "==> Changed-only verification passed"
