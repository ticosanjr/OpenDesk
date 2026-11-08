#!/usr/bin/env bash
# Core Rust coverage ratchet. Reads the `cargo llvm-cov --summary-only` output
# produced by scripts/gate.sh and fails when the modules that carry business
# logic fall below the threshold. App glue (tray, updater, daemon manager,
# MCP bridge) is intentionally excluded from this specific gate.
set -euo pipefail

SUMMARY="${1:-}"
MIN="${CORE_MIN_LINES:-59}"

if [[ -z "$SUMMARY" || ! -f "$SUMMARY" ]]; then
  echo "usage: $0 <coverage-summary.txt>" >&2
  exit 2
fi

python3 - "$SUMMARY" "$MIN" <<'PY'
import re
import sys

summary, minimum = sys.argv[1], float(sys.argv[2])
core = re.compile(
    r"^(db/|api/|recipes/|queue_scheduler/|io_bus/|config_sync/|dsh_router/"
    r"|starter_packs\.rs|run_timeline\.rs|events\.rs|proposals\.rs|secrets\.rs"
    r"|paths\.rs|error\.rs)"
)
lines = missed = 0
files = 0
for raw in open(summary, encoding="utf-8"):
    parts = raw.split()
    if len(parts) < 10 or not parts[0].endswith(".rs"):
        continue
    if not core.match(parts[0]) or parts[0].endswith("tests.rs"):
        continue
    try:
        total = int(parts[7])
        miss = int(parts[8])
    except ValueError:
        continue
    lines += total
    missed += miss
    files += 1

if lines == 0:
    print("core coverage: no core files found", file=sys.stderr)
    sys.exit(2)
pct = (lines - missed) / lines * 100
print(f"==> Core Rust coverage: {pct:.2f}% ({files} files, {lines - missed}/{lines} lines)")
if pct < minimum:
    print(
        f"core coverage {pct:.2f}% is below the {minimum:.1f}% ratchet",
        file=sys.stderr,
    )
    sys.exit(1)
PY
