#!/usr/bin/env python3
"""Package the headless CLI separately from desktop installers on every target."""
import argparse
from pathlib import Path
import tarfile
import zipfile

import package_release as release


def package(version, target, binary, output):
    release.validate_package_input(version, target, binary)
    output.mkdir(parents=True, exist_ok=True)
    windows = "windows" in target
    name = f"paintcraft-cli-{version}-{release.TARGETS[target]}"
    executable = "paintcraft-cli.exe" if windows else "paintcraft-cli"
    archive = output / (name + (".zip" if windows else ".tar.gz"))
    files = [(binary, executable), (release.ROOT / "LICENSE", "LICENSE")]
    if windows:
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as stream:
            for source, destination in files:
                stream.write(source, destination)
    else:
        with tarfile.open(archive, "w:gz") as stream:
            for source, destination in files:
                info = stream.gettarinfo(str(source), arcname=destination)
                info.mode = 0o755 if destination == executable else 0o644
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                with source.open("rb") as contents:
                    stream.addfile(info, contents)
    release.write_checksum(archive)
    return archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=release.TARGETS, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        print(package(args.version, args.target, args.binary, args.output))
    except (OSError, ValueError) as error:
        parser.exit(1, f"CLI packaging failed: {error}\n")
