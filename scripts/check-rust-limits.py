#!/usr/bin/env python3
"""Rust limits gate.

- No `.rs` file above 500 lines.
- No `.unwrap()`/`.expect(` outside `#[cfg(test)]`.

Existing debt is tracked in `scripts/rust-limits-baseline.txt` as a ratchet:
new files/occurrences fail the gate, and the baseline only shrinks. Fixing a
file without updating the baseline prints a warning, so the file can be
removed from the baseline in the same change.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

MAX_LINES = 500
PATTERNS = (".unwrap()", ".expect(")


def is_test_file(path: pathlib.Path) -> bool:
    name = path.name
    return (
        name == "tests.rs"
        or name.endswith("_tests.rs")
        or name.startswith("test_")
        or "tests" in path.parts
    )


def production_text(text: str) -> str:
    lines = text.splitlines()
    cut = next(
        (index for index, line in enumerate(lines) if line.strip() == "#[cfg(test)]"),
        len(lines),
    )
    return "\n".join(lines[:cut])


def scan(roots: list[pathlib.Path], check_lines: bool = True) -> dict[str, tuple[str, int]]:
    issues: dict[str, tuple[str, int]] = {}
    for root in roots:
        for path in sorted(root.rglob("*.rs")):
            try:
                text = path.read_text(errors="ignore")
            except OSError:
                continue
            lines = len(text.splitlines())
            if check_lines and lines > MAX_LINES:
                issues[str(path)] = ("lines", lines)
            if is_test_file(path):
                continue
            body = production_text(text)
            for pattern in PATTERNS:
                count = body.count(pattern)
                if count:
                    key = f"{path}:{pattern}"
                    issues[key] = (pattern, count)
    return issues


def load_baseline(path: pathlib.Path) -> dict[str, int]:
    baseline: dict[str, int] = {}
    if not path.is_file():
        return baseline
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("|")
        if len(parts) != 3:
            continue
        name, _kind, value = parts
        try:
            baseline[name] = int(value)
        except ValueError:
            continue
    return baseline


def write_baseline(path: pathlib.Path, issues: dict[str, tuple[str, int]]) -> None:
    lines = ["# path[:pattern]|kind|value — ratchet; only shrink."]
    for name in sorted(issues):
        kind, value = issues[name]
        lines.append(f"{name}|{kind}|{value}")
    path.write_text("\n".join(lines) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", action="append", default=None)
    parser.add_argument("--no-line-limit", action="store_true")
    parser.add_argument("--baseline", default="scripts/rust-limits-baseline.txt")
    parser.add_argument("--write-baseline", action="store_true")
    args = parser.parse_args()

    roots = [pathlib.Path(root) for root in (args.root or ["src-tauri/crates"])]
    issues = scan(roots, check_lines=not args.no_line_limit)
    baseline_path = pathlib.Path(args.baseline)

    if args.write_baseline:
        write_baseline(baseline_path, issues)
        print(f"wrote {baseline_path} with {len(issues)} entries")
        return 0

    baseline = load_baseline(baseline_path)
    failures = 0
    for name, (kind, value) in sorted(issues.items()):
        allowed = baseline.get(name)
        if allowed is None:
            print(f"FAIL new violation: {name} ({kind}={value})")
            failures += 1
        elif value > allowed:
            print(f"FAIL grew: {name} ({kind}={value}, baseline={allowed})")
            failures += 1
    for name, allowed in sorted(baseline.items()):
        current = issues.get(name)
        if current is None:
            print(f"note: baseline can shrink: {name} (was {allowed})")
        elif current[1] < allowed:
            print(f"note: baseline can shrink: {name} (now {current[1]}, was {allowed})")

    if failures:
        print(
            f"\n{failures} new Rust limit violation(s). Fix them or, for legacy "
            "debt, update the baseline deliberately.",
            file=sys.stderr,
        )
        return 1
    print(f"rust limits: ok ({len(issues)} grandfathered entries)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
