"""Finite shipping rollout; native runtime evidence is target-specific."""
import argparse
import os
from pathlib import Path
import subprocess

import codec_worker
import package_release as release

# Integer JPEG XL and the installed JPEG XR writer are qualified on all six
# targets. Keep this finite table explicit so new targets require qualification.
V10_SELECTION = ("jxr-codecs,jxl-integer-codec", codec_worker.JXL_INTEGER_JXR_PROFILE)
ROLLOUT = {
    "x86_64-pc-windows-msvc": V10_SELECTION,
    "aarch64-pc-windows-msvc": V10_SELECTION,
    "aarch64-apple-darwin": V10_SELECTION,
    "x86_64-apple-darwin": V10_SELECTION,
    "x86_64-unknown-linux-gnu": V10_SELECTION,
    "aarch64-unknown-linux-gnu": V10_SELECTION,
}


def selection(target):
    try:
        return ROLLOUT[target]
    except KeyError as error:
        raise ValueError("Target has no exact helper rollout policy") from error


def selector(target):
    """Exact public contract; feature names never imply a fallback profile."""
    return {codec_worker.JXL_INTEGER_PROFILE: "v9",
            codec_worker.JXL_INTEGER_JXR_PROFILE: "v10"}[selection(target)[1]]


def macos_target(explicit=None):
    # Pin --target even when defaulting to rustc's host, so Cargo configuration
    # and Python/Rosetta architecture cannot silently change the selected build.
    target = explicit or os.environ.get("CARGO_BUILD_TARGET")
    if not target:
        result = subprocess.run(["rustc", "--version", "--verbose"], cwd=release.ROOT,
                                check=True, capture_output=True, text=True)
        hosts = [line.removeprefix("host: ") for line in result.stdout.splitlines()
                 if line.startswith("host: ")]
        if len(hosts) != 1:
            raise ValueError("rustc did not identify exactly one host target")
        target = hosts[0]
    if target not in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
        raise ValueError("macOS bundle requires an explicit supported Rust macOS target")
    return target


def verify(binary, version, target):
    _, helper = codec_worker.prepare(binary, version, target)
    profile, _ = codec_worker.verify_build_record(helper, version)
    if profile != selection(target)[1]:
        raise ValueError("Built helper differs from the exact target rollout profile")


def prepare_installation(binary, target, output):
    """Use the real archive producer/installer before installed consumer tests."""
    import install_cli
    import package_cli
    version = release.workspace_version()
    verify(binary, version, target)
    archive = package_cli.package(version, target, binary, output / "archives")
    sums = output / "archives/SHA256SUMS.txt"
    sums.write_bytes(release.checksum_line(archive))
    return install_cli.install(archive, sums, version, target, output / "prefix")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=release.TARGETS)
    parser.add_argument("--github-output", action="store_true")
    parser.add_argument("--prepare-installation", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    feature, profile = selection(args.target)
    if args.github_output:
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as stream:
            stream.write(f"feature={feature}\nprofile={profile}\nselector={selector(args.target)}\n")
    if args.prepare_installation:
        if args.output is None:
            parser.error("--prepare-installation requires --output")
        print(prepare_installation(args.prepare_installation, args.target, args.output))


if __name__ == "__main__":
    main()
