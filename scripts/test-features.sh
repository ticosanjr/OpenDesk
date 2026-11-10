#!/usr/bin/env bash
# Functional coverage gate. Run on demand (release/feature sign-off), not on
# every change. Requires the full Playwright suite.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mkdir -p test-results
status=0
PLAYWRIGHT_JSON_OUTPUT_NAME=test-results/features.json \
  npx playwright test --reporter=json || status=$?

node scripts/feature-coverage.mjs test-results/features.json || status=1
exit "$status"
