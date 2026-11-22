#!/usr/bin/env python3
"""Fail when a frontend source file grows past the 500-line limit.

Tests and generated declarations are ignored: the limit exists to keep
components and stores reviewable, not to fragment fixtures.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

MAX_LINES = 500
SKIP_SUFFIXES = (".test.ts", ".test.tsx", ".d.ts")


def scan(root: pathlib.Path, max_lines: int) -> dict[str, int]:
    issues: dict[str, int] = {}
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        if path.suffix not in (".ts", ".tsx"):
            continue
        if path.name.endswith(SKIP_SUFFIXES):
            continue
        lines = len(path.read_text(encoding="utf-8", errors="ignore").splitlines())
        if lines > max_lines:
            issues[str(path)] = lines
    return issues


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default="src")
    parser.add_argument("--max-lines", type=int, default=MAX_LINES)
    args = parser.parse_args()

    issues = scan(pathlib.Path(args.root), args.max_lines)
    if not issues:
        print(f"frontend limits: ok (<= {args.max_lines} lines)")
        return 0
    for path, lines in sorted(issues.items(), key=lambda item: -item[1]):
        print(f"{path}: {lines} lines (> {args.max_lines})")
    print(f"frontend limits: {len(issues)} file(s) over {args.max_lines} lines")
    return 1


if __name__ == "__main__":
    sys.exit(main())
