#!/usr/bin/env python3
"""Validate an installed MSI payload against this checkout's release binaries.

Does not execute the installed application or helper. Registry and MSI lifecycle
checks remain in verify_windows_install.ps1.
"""
import argparse
from pathlib import Path

import codec_worker
import package_release as release


def verify(binary, directory):
    version = release.workspace_version()
    target, source_helper = codec_worker.prepare(binary, version)
    if target not in ("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"):
        raise ValueError("Windows installation requires a Windows release binary")
    installed = directory / "paintcraft.exe"
    release.validate_binary(target, installed)
    if release.digest(installed) != release.digest(binary):
        raise ValueError("Installed executable differs from the release binary")
    helper = directory / codec_worker.filename(target)
    if release.digest(helper) != release.digest(source_helper):
        raise ValueError("Installed decoder helper differs from the release helper")
    # Reuse the exact schema/profile/variants/build-record/hash and notice
    # validator used by producers and the private-prefix installer.
    codec_worker.verify_installed_manifest(directory, version, target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--directory", required=True, type=Path)
    args = parser.parse_args()
    try:
        verify(args.binary, args.directory)
    except (OSError, ValueError) as error:
        parser.exit(1, f"Windows installed payload verification failed: {error}\n")
    print("Verified installed executable, exact codec manifest and dependency notices")


if __name__ == "__main__":
    main()
