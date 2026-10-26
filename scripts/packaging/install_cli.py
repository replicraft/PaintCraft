#!/usr/bin/env python3
"""Install the complete verified native CLI distribution into a private prefix."""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile

import codec_worker
import package_cli
import package_release as release

REPOSITORY = "https://github.com/replicraft/PaintCraft"
API = "https://api.github.com/repos/replicraft/PaintCraft/releases"
ARCHIVE_MAX = 256 << 20
PAYLOAD_MAX = 384 << 20
FILE_MAX = 192 << 20
METADATA_MAX = 2 << 20


def native_target():
    system, machine = platform.system(), platform.machine().lower()
    arch = {"arm64": "aarch64", "aarch64": "aarch64", "amd64": "x86_64", "x86_64": "x86_64"}.get(machine)
    suffix = {"Darwin": "apple-darwin", "Linux": "unknown-linux-gnu", "Windows": "pc-windows-msvc"}.get(system)
    if not arch or not suffix:
        raise ValueError("Prebuilt CLI downloads support macOS, Windows and Linux on x64 or ARM64")
    if system == "Linux" and platform.libc_ver()[0] != "glibc":
        raise ValueError("The prebuilt Linux CLI requires glibc; musl is not qualified")
    return f"{arch}-{suffix}"


def checked_version(version):
    if not release.VERSION.fullmatch(version) or len(version) > 100:
        raise ValueError("Version must be a release version such as 0.1.0")
    return version


def archive_name(version, target):
    suffix = ".zip" if "windows" in target else ".tar.gz"
    return f"paintcraft-cli-{checked_version(version)}-{release.TARGETS[target]}{suffix}"


def executable(target):
    return "paintcraft-cli.exe" if "windows" in target else "paintcraft-cli"


def payload_names(target):
    return {executable(target), codec_worker.filename(target), codec_worker.MANIFEST,
            "LICENSE", *codec_worker.NOTICES}


def download(url, destination, limit):
    if not url.startswith("https://"):
        raise ValueError("Release downloads require HTTPS")
    request = urllib.request.Request(url, headers={"User-Agent": "PaintCraft-CLI-installer", "Accept": "application/vnd.github+json"})
    with urllib.request.urlopen(request, timeout=60) as response:
        if not response.url.startswith("https://"):
            raise ValueError("Refusing a non-HTTPS download redirect")
        length = response.headers.get("Content-Length")
        if length is not None and int(length) > limit:
            raise ValueError("Release download exceeds its byte allowance")
        total = 0
        with destination.open("xb") as output:
            while chunk := response.read(1 << 20):
                total += len(chunk)
                if total > limit:
                    raise ValueError("Release download exceeds its byte allowance")
                output.write(chunk)


def fetch(temporary, requested, target):
    metadata = temporary / "release.json"
    endpoint = f"{API}/tags/v{checked_version(requested)}" if requested else f"{API}/latest"
    download(endpoint, metadata, METADATA_MAX)
    info = json.loads(metadata.read_bytes())
    if not isinstance(info, dict) or not isinstance(info.get("assets"), list):
        raise ValueError("Malformed release metadata")
    tag = info.get("tag_name", "")
    if not isinstance(tag, str) or not tag.startswith("v"):
        raise ValueError("Release response has no version tag")
    version = checked_version(tag[1:])
    if requested and version != requested:
        raise ValueError("Release response does not match the requested version")
    if info.get("draft") is not False or not isinstance(info.get("prerelease"), bool) or info["prerelease"] and not requested:
        raise ValueError("Latest download must be a published stable release")
    if any(not isinstance(asset, dict) for asset in info["assets"]):
        raise ValueError("Malformed release asset metadata")
    name = archive_name(version, target)
    names = [asset.get("name") for asset in info.get("assets", [])]
    if names.count(name) != 1 or names.count("SHA256SUMS.txt") != 1:
        raise ValueError("Release has no complete CLI archive and checksum set for this platform")
    base = f"{REPOSITORY}/releases/download/v{version}"
    archive, sums = temporary / name, temporary / "SHA256SUMS.txt"
    download(f"{base}/{name}", archive, ARCHIVE_MAX)
    download(f"{base}/SHA256SUMS.txt", sums, METADATA_MAX)
    return archive, sums, version


def verify_checksum(archive, checksums):
    if archive.is_symlink() or not archive.is_file() or archive.stat().st_size > ARCHIVE_MAX:
        raise ValueError("CLI archive must be a bounded regular file")
    if checksums.is_symlink() or not checksums.is_file() or checksums.stat().st_size > METADATA_MAX:
        raise ValueError("Checksums must be a bounded regular file")
    matches = []
    for line in checksums.read_text(encoding="ascii").splitlines():
        match = re.fullmatch(r"([0-9a-fA-F]{64})  ([^/\\]+)", line)
        if not match:
            raise ValueError("Malformed release checksum line")
        if match[2] == archive.name:
            matches.append(match[1].lower())
    if len(matches) != 1 or release.digest(archive) != matches[0]:
        raise ValueError("CLI archive SHA-256 does not match exactly one release checksum")


def extract(archive, directory, target):
    expected, seen, total = payload_names(target), set(), 0

    def member(name, size, regular, source):
        nonlocal total
        if name not in expected or name in seen or not regular or size < 0 or size > FILE_MAX:
            raise ValueError("CLI archive contains an unexpected, duplicate or unsafe entry")
        if name not in (executable(target), codec_worker.filename(target)) and size > METADATA_MAX:
            raise ValueError("CLI archive metadata exceeds its allowance")
        total += size
        if total > PAYLOAD_MAX:
            raise ValueError("CLI archive exceeds its expanded byte allowance")
        seen.add(name)
        copied = 0
        with (directory / name).open("xb") as output:
            while chunk := source.read(min(1 << 20, size - copied + 1)):
                copied += len(chunk)
                if copied > size:
                    raise ValueError("CLI archive entry exceeds its declared size")
                output.write(chunk)
        if copied != size:
            raise ValueError("Truncated CLI archive entry")
        (directory / name).chmod(0o755 if name in (executable(target), codec_worker.filename(target)) else 0o644)

    if "windows" in target:
        with zipfile.ZipFile(archive) as stream:
            for entry in stream.infolist():
                mode = entry.external_attr >> 16
                regular = not entry.is_dir() and stat.S_IFMT(mode) in (0, stat.S_IFREG)
                with stream.open(entry) as source:
                    member(entry.filename, entry.file_size, regular, source)
    else:
        with tarfile.open(archive, "r:gz") as stream:
            for entry in stream:
                if not entry.isfile():
                    raise ValueError("CLI archive entries must be regular files")
                with stream.extractfile(entry) as source:
                    member(entry.name, entry.size, True, source)
    if seen != expected:
        raise ValueError("CLI archive is missing required helper, manifest or license files")


def is_link(path):
    return path.is_symlink() or getattr(path, "is_junction", lambda: False)()


def verify_payload(directory, version, target):
    if is_link(directory) or not directory.is_dir():
        raise ValueError("CLI installation must be a regular directory")
    if {entry.name for entry in directory.iterdir()} != payload_names(target):
        raise ValueError("Refusing to replace a directory containing foreign or incomplete files")
    for entry in directory.iterdir():
        if is_link(entry) or not entry.is_file():
            raise ValueError("CLI installation may contain only regular files")
    release.validate_binary(target, directory / executable(target))
    codec_worker.verify_installed_manifest(directory, version, target)
    if (directory / "LICENSE").read_bytes() != (release.ROOT / "LICENSE").read_bytes():
        raise ValueError("CLI license is missing or changed")


def private_prefix(prefix):
    prefix = prefix.expanduser().absolute()
    for path in (prefix, *prefix.parents):
        if is_link(path):
            raise ValueError("Install prefix must not pass through a symlink or junction")
    prefix.mkdir(parents=True, exist_ok=True, mode=0o700)
    if not prefix.is_dir():
        raise ValueError("Install prefix must be a directory")
    if os.name != "nt" and (prefix.stat().st_uid != os.getuid() or prefix.stat().st_mode & 0o022):
        raise ValueError("Install prefix must be owned by this user and not writable by others")
    return prefix


def install(archive, checksums, version, target, prefix):
    checked_version(version)
    if archive.name != archive_name(version, target):
        raise ValueError("Archive filename does not match its version and platform")
    verify_checksum(archive, checksums)
    prefix = private_prefix(prefix)
    lock = prefix / ".install-lock"
    try:
        lock.mkdir(mode=0o700)
    except FileExistsError as error:
        raise ValueError("Another install is in progress; inspect .install-lock before retrying") from error
    try:
        with tempfile.TemporaryDirectory(prefix=".install-", dir=prefix) as temporary:
            temporary = Path(temporary)
            payload, destination = temporary / "payload", prefix / "bin"
            # Keep recovery bytes outside TemporaryDirectory's automatic cleanup.
            backup = prefix / (temporary.name + "-previous")
            payload.mkdir()
            extract(archive, payload, target)
            verify_payload(payload, version, target)
            if destination.exists() or is_link(destination):
                manifest = destination / codec_worker.MANIFEST
                if is_link(destination) or is_link(manifest) or not manifest.is_file() or manifest.stat().st_size > METADATA_MAX:
                    raise ValueError("Existing CLI directory is not a verified installation")
                old = json.loads(manifest.read_bytes())
                verify_payload(destination, checked_version(old["application_version"]), target)
                os.replace(destination, backup)
            try:
                os.replace(payload, destination)
            except BaseException:
                if backup.exists():
                    try:
                        os.replace(backup, destination)
                    except OSError as error:
                        raise ValueError(f"Previous installation is preserved for recovery at {backup}") from error
                raise
            if backup.exists():
                try:
                    shutil.rmtree(backup)
                except OSError as error:
                    print(f"Installation succeeded. Previous files retained for cleanup at {backup}: {error}", file=sys.stderr)
            return destination
    finally:
        lock.rmdir()


def source_archive(temporary, target):
    version = release.workspace_version()
    subprocess.run(["cargo", "build", "--locked", "--release", "--target", target,
                    "-p", "paintcraft-cli", "-p", "paintcraft-codec-worker",
                    "--features", "paintcraft-codec-worker/avif-codec"], cwd=release.ROOT, check=True)
    result = subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
                            cwd=release.ROOT, check=True, capture_output=True, text=True)
    binary = Path(json.loads(result.stdout)["target_directory"]) / target / "release" / executable(target)
    profile, _ = codec_worker.verify_build_record(binary.parent / codec_worker.filename(target), version)
    if profile not in (codec_worker.AVIF_PROFILE, codec_worker.AVIF_SDR_PROFILE):
        raise ValueError("Source install requires the AVIF-enabled helper")
    archive = package_cli.package(version, target, binary, temporary)
    sums = temporary / "SHA256SUMS.txt"
    sums.write_bytes(release.checksum_line(archive))
    return archive, sums, version


def default_prefix():
    if os.name == "nt":
        return Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData/Local")) / "PaintCraft/cli"
    return Path.home() / ".local/share/paintcraft-cli"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, default=default_prefix(), help="Owned installation prefix; all files are kept under its bin directory")
    parser.add_argument("--version", help="Published version; default is latest stable")
    parser.add_argument("--source", action="store_true", help="Build this checkout's CLI and AVIF-enabled helper (requires Rust, a C compiler, Meson and Ninja)")
    parser.add_argument("--archive", type=Path, help="Install a previously downloaded complete archive")
    parser.add_argument("--checksums", type=Path, help="SHA256SUMS.txt for --archive")
    args = parser.parse_args(argv)
    if args.source and (args.archive or args.checksums or args.version):
        parser.error("--source builds the checkout version and cannot be combined with archive/version options")
    if bool(args.archive) != bool(args.checksums) or args.archive and not args.version:
        parser.error("--archive requires --checksums and --version")
    try:
        target = native_target()
        with tempfile.TemporaryDirectory(prefix="paintcraft-cli-download-") as temporary:
            if args.source:
                archive, sums, version = source_archive(Path(temporary), target)
            elif args.archive:
                archive, sums, version = args.archive, args.checksums, args.version
            else:
                archive, sums, version = fetch(Path(temporary), args.version, target)
            destination = install(archive, sums, version, target, args.prefix)
        print(f"Installed PaintCraft CLI {version} into {destination}")
        print(f"Keep every file together and add this directory to PATH: {destination}")
    except (OSError, ValueError, KeyError, tarfile.TarError, zipfile.BadZipFile, subprocess.CalledProcessError) as error:
        parser.exit(1, f"CLI installation failed: {error}\n")


if __name__ == "__main__":
    main()
