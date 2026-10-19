#!/usr/bin/env python3
"""Enforce the 800-line limit on repository-owned text files."""

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAX_LINES = 800
# Cargo owns this generated file; splitting it would invalidate the lockfile.
GENERATED = {"Cargo.lock"}


def main():
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=ROOT,
    ).split(b"\0")
    failures = []
    for raw in sorted(set(paths)):
        if not raw:
            continue
        relative = raw.decode("utf-8")
        path = ROOT / relative
        if relative in GENERATED or not path.is_file():
            continue
        try:
            lines = len(path.read_text(encoding="utf-8").splitlines())
        except UnicodeDecodeError:
            continue  # Binary assets have no source line count.
        if lines > MAX_LINES:
            failures.append(f"{relative}: {lines} lines (maximum {MAX_LINES})")
    if failures:
        print("\n".join(failures))
        return 1
    print("Repository text files are within the 800-line limit (Cargo.lock excluded).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
