#!/usr/bin/env python3
"""Build x64 and ARM64 MSI installers using the same WiX 5 template."""
import argparse
from pathlib import Path
import subprocess
import tempfile

import codec_worker
import package_release as release

ARCHITECTURES = {"x86_64-pc-windows-msvc": "x64", "aarch64-pc-windows-msvc": "arm64"}


def package(version, target, binary, output):
    codec_worker.prepare(binary, version, target)
    if target not in ARCHITECTURES:
        raise ValueError(f"Unsupported MSI architecture: {target}")
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    installer = output / f"paintcraft-{version}-{release.TARGETS[target]}.msi"
    numeric = version.split("-")[0].split("+")[0]
    with tempfile.TemporaryDirectory(prefix="paintcraft-msi-helper-") as directory:
        helper = codec_worker.copy(binary, Path(directory), version, target)
        subprocess.run(["wix", "build", "-arch", ARCHITECTURES[target], "-pdbtype", "none",
                        "-d", f"Version={numeric}", "-d", f"Binary={binary.resolve()}",
                        "-d", f"CodecWorker={helper}",
                        "-d", f"CodecManifest={Path(directory) / codec_worker.MANIFEST}",
                        "-d", f"CodecNotice={Path(directory) / codec_worker.NOTICE}",
                        "-d", f"Icon={release.ROOT / 'assets/paintcraft-icon.ico'}",
                        "-d", f"License={release.ROOT / 'LICENSE'}", "-o", str(installer),
                        "-d", f"FontLicenses={release.ROOT / 'packaging/FONT_LICENSES.txt'}",
                        str(release.ROOT / "packaging/windows.wxs")], check=True)
    release.write_checksum(installer)
    print(installer)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        package(args.version, args.target, args.binary, args.output)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Windows packaging failed: {error}\n")


if __name__ == "__main__":
    main()
