#!/usr/bin/env python3
"""Inspect DMG and update ZIP payloads without launching or installing the app."""
import argparse
from pathlib import Path, PurePosixPath
import os
import stat
import subprocess
import sys
import tempfile
import zipfile

import package_release as release


def files(bundle):
    paths = list(bundle.rglob("*"))
    if any(path.is_symlink() for path in paths):
        raise ValueError("Unexpected symlink inside PaintCraft.app")
    return {path.relative_to(bundle).as_posix(): release.digest(path)
            for path in paths if path.is_file()}


def verify(version, target, image, update):
    if sys.platform != "darwin":
        raise ValueError("DMG verification requires macOS")
    with zipfile.ZipFile(update) as archive:
        for info in archive.infolist():
            path = PurePosixPath(info.filename)
            if (not info.filename.startswith("PaintCraft.app/") or ".." in path.parts
                    or "\\" in info.filename or stat.S_ISLNK(info.external_attr >> 16)):
                raise ValueError(f"Unexpected updater ZIP entry: {info.filename}")
    with tempfile.TemporaryDirectory(prefix="paintcraft-macos-verify-") as directory:
        root = Path(directory)
        extracted = root / "extracted"
        subprocess.run(["/usr/bin/ditto", "-x", "-k", str(update.resolve()), str(extracted)], check=True)
        app = extracted / "PaintCraft.app"
        subprocess.run(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(app)], check=True)
        for binary in (app / "Contents/MacOS/paintcraft-launcher",
                       app / "Contents/Helpers/PaintCraft Editor.app/Contents/MacOS/paintcraft"):
            release.validate_package_input(version, target, binary)
            if binary.stat().st_mode & 0o111 != 0o111:
                raise ValueError(f"Updater ZIP lost executable permissions: {binary}")
        mount = root / "mounted"
        mount.mkdir()
        subprocess.run(["/usr/bin/hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint",
                        str(mount), str(image.resolve())], check=True)
        try:
            installed = mount / "PaintCraft.app"
            subprocess.run(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(installed)], check=True)
            if files(installed) != files(app):
                raise ValueError("DMG and updater ZIP contain different application files")
            if not (mount / "Applications").is_symlink() or os.readlink(mount / "Applications") != "/Applications":
                raise ValueError("DMG is missing its Applications shortcut")
        finally:
            subprocess.run(["/usr/bin/hdiutil", "detach", str(mount)], check=True)
    print(f"Verified {target}: identical signed DMG/ZIP app, both executable architectures and permissions")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=("aarch64-apple-darwin", "x86_64-apple-darwin"), required=True)
    parser.add_argument("--dmg", type=Path, required=True)
    parser.add_argument("--update", type=Path, required=True)
    args = parser.parse_args()
    try:
        verify(args.version, args.target, args.dmg, args.update)
    except (OSError, ValueError, subprocess.CalledProcessError, zipfile.BadZipFile) as error:
        parser.exit(1, f"macOS release verification failed: {error}\n")


if __name__ == "__main__":
    main()
