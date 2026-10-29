#!/usr/bin/env python3
"""Enforce the 800-line limit on repository-owned source code."""

import subprocess
from pathlib import Path

from verify_jpegxr_vendor import verify as verify_jpegxr_vendor
from verify_jpegxl_vendor import verify as verify_jpegxl_vendor
from verify_jxl_decoder_vendor import verify as verify_jxl_decoder_vendor

ROOT = Path(__file__).resolve().parent.parent
MAX_LINES = 800
# Documentation, manifests, fixtures and generated data have no source line cap.
SOURCE_SUFFIXES = {
    ".rs", ".py", ".wgsl", ".glsl", ".vert", ".frag",
    ".js", ".jsx", ".ts", ".tsx", ".mjs", ".cjs", ".astro",
    ".c", ".h", ".cc", ".cpp", ".hpp",
    ".sh", ".bash", ".zsh", ".html", ".css", ".scss",
}


def main():
    try:
        inherited = (verify_jpegxr_vendor(ROOT) | verify_jpegxl_vendor(ROOT)
                     | verify_jxl_decoder_vendor(ROOT))
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Third-party source verification failed: {error}")
        return 1
    paths = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=ROOT,
    ).split(b"\0")
    failures = []
    for raw in sorted(set(paths)):
        if not raw:
            continue
        relative = raw.decode("utf-8")
        if relative in inherited:
            continue  # Exact reviewed upstream bytes, never a directory exemption.
        path = ROOT / relative
        if path.suffix not in SOURCE_SUFFIXES or not path.is_file():
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
    print(
        "Repository-owned source files are within the 800-line limit "
        f"({len(inherited)} verified upstream exceptions)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
