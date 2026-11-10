#!/usr/bin/env python3
"""Sign every finalized release file with minisign; verify without signing secrets."""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile

import package_release as release

SUFFIX = ".minisig"
MAX_TEXT = 8192


def regular(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
        raise ValueError("Release signing requires nonempty regular files")


def read_small(path):
    regular(path)
    with path.open("rb") as stream:
        data = stream.read(MAX_TEXT + 1)
    if len(data) > MAX_TEXT:
        raise ValueError("Signing text exceeds its fixed limit")
    return data


def finalized(version, assets, signed):
    if version != release.workspace_version():
        raise ValueError("Signing version must match workspace version")
    if assets.is_symlink() or not assets.is_dir():
        raise ValueError("Release assets must be a regular directory")
    names = release.expected_assets(version) | {"SHA256SUMS.txt"}
    expected = names | ({name + SUFFIX for name in names} if signed else set())
    if {p.name for p in assets.iterdir()} != expected:
        raise ValueError("Release signing file set is incomplete or contains unexpected files")
    for name in expected:
        regular(assets / name)
    checksums = b"".join(release.checksum_line(assets / name)
                         for name in sorted(names - {"SHA256SUMS.txt"}))
    if (assets / "SHA256SUMS.txt").read_bytes() != checksums:
        raise ValueError("Finalized release checksums do not match payloads")
    return sorted(names)


def trusted_comment(version, name):
    return f"paintcraft-update {version} {name}"


def command(argv, password=b"", environment=None):
    # Even a failed CLI must not echo key/password/process output into CI logs.
    env = dict(os.environ) if environment is None else dict(environment)
    env.pop("UPDATE_SIGNING_KEY", None)
    env.pop("UPDATE_SIGNING_PASSWORD", None)
    try:
        result = subprocess.run(argv, input=password, stdout=subprocess.DEVNULL,
                                stderr=subprocess.DEVNULL, env=env, timeout=300,
                                check=False)
    except (OSError, subprocess.TimeoutExpired):
        raise ValueError("Minisign could not complete; subprocess diagnostics were suppressed") from None
    if result.returncode != 0:
        raise ValueError("Minisign rejected signing or verification; subprocess diagnostics were suppressed")


def verify_one(version, asset, signature, public_key, minisign):
    lines = read_small(signature).splitlines()
    expected = ("trusted comment: " + trusted_comment(version, asset.name)).encode("utf-8")
    if len(lines) != 4 or lines[2] != expected:
        raise ValueError("Signature trusted comment does not bind this release asset")
    command([minisign, "-V", "-q", "-p", str(public_key), "-m", str(asset), "-x", str(signature)])


def verify(version, assets, public_key, minisign="minisign"):
    names = finalized(version, assets, signed=True)
    public_before = read_small(public_key)
    for name in names:
        verify_one(version, assets / name, assets / (name + SUFFIX), public_key, minisign)
    finalized(version, assets, signed=True)
    if read_small(public_key) != public_before:
        raise ValueError("Update public key changed during verification")
    return len(names)


def sign(version, assets, public_key, minisign="minisign", environment=None):
    names = finalized(version, assets, signed=False)
    public_bytes = read_small(public_key)
    before = {name: release.digest(assets / name) for name in names}
    env = os.environ if environment is None else environment
    secret = env.get("UPDATE_SIGNING_KEY", "")
    password = env.get("UPDATE_SIGNING_PASSWORD", "").encode("utf-8")
    if not secret or len(secret.encode("utf-8")) > MAX_TEXT or "\0" in secret:
        raise ValueError("UPDATE_SIGNING_KEY must contain a bounded minisign secret key")
    # minisign0.12 get_password uses fgets(...,1024,stdin): include a newline.
    if len(password) > 1022 or any(c in password for c in (0, 10, 13)):
        raise ValueError("UPDATE_SIGNING_PASSWORD must be one bounded UTF-8 line")
    with tempfile.TemporaryDirectory(prefix="paintcraft-sign-") as directory:
        temporary = Path(directory)
        temporary.chmod(0o700)
        key = temporary / "secret.key"
        fd = os.open(key, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "wb") as stream:
            stream.write(secret.encode("utf-8"))
        public = temporary / "update.pub"
        public.write_bytes(public_bytes)
        for name in names:
            asset, signature = assets / name, temporary / (name + SUFFIX)
            command([minisign, "-S", "-s", str(key), "-m", str(asset), "-x", str(signature),
                     "-t", trusted_comment(version, name)], password + b"\n", env)
            verify_one(version, asset, signature, public, minisign)
        # Refuse any input/key drift before publishing even one signature.
        finalized(version, assets, signed=False)
        if read_small(public_key) != public_bytes or any(
                release.digest(assets / name) != before[name] for name in names):
            raise ValueError("Release inputs changed during signing")
        for name in names:
            os.replace(temporary / (name + SUFFIX), assets / (name + SUFFIX))
    return verify(version, assets, public_key, minisign)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--assets", required=True, type=Path)
    parser.add_argument("--public-key", type=Path, default=release.ROOT / "packaging/paintcraft-update-key.pub")
    parser.add_argument("--minisign", default="minisign")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    try:
        operation = verify if args.verify_only else sign
        count = operation(args.version, args.assets.absolute(), args.public_key, args.minisign)
    except (OSError, ValueError):
        # No key strings, password, command output, or exception payload is printed.
        parser.exit(1, "Release signature operation failed; no subprocess diagnostics are disclosed.\n")
    print(f"Verified {count} release signatures")


if __name__ == "__main__":
    main()
