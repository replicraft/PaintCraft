#!/usr/bin/env python3
"""Verify the exact reviewed JPEG XR source, patch and bounded line exceptions."""

import argparse
import difflib
import hashlib
import json
import tarfile
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
VENDOR = "vendor/jpegxr-pure-rs-0.1.0-paintcraft1"
ARCHIVE_SHA256 = "be1bfea78bcd73aa67dfce7cdd22902e4e6ac06b387a5dea4f225f52525e8590"
COMMIT = "cd626cd555e944c6cdcf273fac52434ca1fedd61"
CHANGED = {"src/api.rs", "src/image/sys/strcodec.rs"}
# Fixed inherited source identities, never a directory-wide source exemption.
INHERITED_LONG_FILES = {
    "src/image/decode/jxrtranscode.rs",
    "src/image/decode/segdec.rs",
    "src/image/decode/str_inv_transform.rs",
    "src/image/decode/strdec.rs",
    "src/image/encode/segenc.rs",
    "src/image/encode/str_fwd_transform.rs",
    "src/image/encode/strenc.rs",
    "src/image/sys/strcodec.rs",
    "src/jxrgluelib/jxrglue.rs",
    "src/jxrgluelib/jxrglue_jxr.rs",
    "src/jxrgluelib/jxrglue_pfc.rs",
    "src/jxrgluelib/jxrmeta.rs",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(root=ROOT, archive=None):
    """Return only the twelve hash-verified inherited source exceptions."""
    root = Path(root)
    source = root / VENDOR
    record = Path(str(source) + ".provenance.json")
    require(not record.is_symlink(), "JPEG XR provenance is a symlink")
    metadata = json.loads(record.read_text("utf-8"))
    require(metadata["schema"] == 1, "JPEG XR provenance schema mismatch")
    require(metadata["package"] == "jpegxr-pure-rs", "JPEG XR package mismatch")
    require(metadata["version"] == "0.1.0", "JPEG XR version mismatch")
    require(metadata["local_revision"] == "paintcraft1", "JPEG XR revision mismatch")
    require(metadata["archive_sha256"] == ARCHIVE_SHA256, "JPEG XR archive mismatch")
    require(metadata["upstream_commit"] == COMMIT, "JPEG XR commit mismatch")
    require(set(metadata["changed_files"]) == CHANGED, "Unexpected patched JPEG XR file")
    require(set(metadata["line_exemptions"]) == INHERITED_LONG_FILES,
            "Unexpected JPEG XR source-line exception")
    require(not source.is_symlink(), "JPEG XR source directory is a symlink")
    actual = set()
    for path in source.rglob("*"):
        require(not path.is_symlink(), f"JPEG XR symlink: {path}")
        if path.is_file():
            actual.add(path.relative_to(source).as_posix())
        else:
            require(path.is_dir(), f"Unexpected JPEG XR filesystem entry: {path}")
    files = metadata["files"]
    require(len(files) == 50, "JPEG XR upstream inventory must contain exactly 50 files")
    require(actual == set(files), "JPEG XR source inventory contains missing/unexpected files")
    inherited = set()
    changed = set()
    for name, expected in files.items():
        relative = PurePosixPath(name)
        require(not relative.is_absolute() and ".." not in relative.parts,
                "Unsafe JPEG XR inventory path")
        require((source / name).stat().st_size == expected["bytes"],
                f"JPEG XR source size mismatch: {name}")
        data = (source / name).read_bytes()
        require(len(data) == expected["bytes"] and digest(data) == expected["sha256"],
                f"JPEG XR source bytes mismatch: {name}")
        require(len(data.splitlines()) == expected["lines"],
                f"JPEG XR source line inventory mismatch: {name}")
        if expected["upstream_sha256"] != expected["sha256"]:
            changed.add(name)
        else:
            require(expected["upstream_bytes"] == expected["bytes"],
                    f"Unexpected upstream JPEG XR size: {name}")
            require(expected["upstream_lines"] == expected["lines"],
                    f"Unexpected upstream JPEG XR lines: {name}")
        if name.endswith(".rs") and expected["upstream_lines"] > 800:
            inherited.add(name)
    require(changed == CHANGED, "Unexpected JPEG XR source changes")
    require(inherited == INHERITED_LONG_FILES, "JPEG XR inherited line inventory mismatch")
    require(metadata["patch_file"] == "jpegxr-pure-rs-0.1.0-paintcraft1.patch",
            "Unexpected JPEG XR patch path")
    patch = root / "vendor" / metadata["patch_file"]
    require(not patch.is_symlink(), "JPEG XR patch is a symlink")
    require(digest(patch.read_bytes()) == metadata["patch_sha256"],
            "JPEG XR patch checksum mismatch")
    added_lines = sum(line.startswith(b"+") and not line.startswith(b"+++")
                      for line in patch.read_bytes().splitlines())
    require(added_lines <= 800, "PaintCraft JPEG XR API patch exceeds 800 added lines")
    license_bytes = (source / "LICENSE.md").read_bytes()
    require(metadata["license_file"] == "LICENSE.md"
            and digest(license_bytes) == metadata["license_sha256"],
            "JPEG XR distributed license mismatch")
    require(license_bytes == (root / "packaging/JPEGXR_LICENSE.txt").read_bytes(),
            "JPEG XR packaged notice differs from upstream license")
    if archive is not None:
        verify_archive(Path(archive), source, files, metadata, patch)
    return {f"{VENDOR}/{name}" for name in INHERITED_LONG_FILES}


def verify_archive(archive, source, files, metadata, patch):
    require(archive.stat().st_size == metadata["archive_bytes"],
            "Original JPEG XR crate archive size mismatch")
    data = archive.read_bytes()
    require(len(data) == metadata["archive_bytes"] and digest(data) == ARCHIVE_SHA256,
            "Original JPEG XR crate archive checksum mismatch")
    inventory = set()
    expected_patch = []
    with tarfile.open(archive) as upstream:
        for member in upstream:
            require(member.isdir() or member.isfile(), "Unexpected JPEG XR archive entry")
            path = PurePosixPath(member.name)
            require(not path.is_absolute() and ".." not in path.parts,
                    "Unsafe JPEG XR archive path")
            require(path.parts[0] == "jpegxr-pure-rs-0.1.0", "JPEG XR archive root mismatch")
            if member.isdir():
                continue
            name = str(path.relative_to("jpegxr-pure-rs-0.1.0"))
            require(name not in inventory and name in files, "JPEG XR archive inventory mismatch")
            inventory.add(name)
            before = upstream.extractfile(member).read()
            expected = files[name]
            require(len(before) == expected["upstream_bytes"]
                    and digest(before) == expected["upstream_sha256"]
                    and len(before.splitlines()) == expected["upstream_lines"],
                    f"JPEG XR upstream bytes mismatch: {name}")
            after = (source / name).read_bytes()
            if before != after:
                expected_patch.extend(difflib.unified_diff(
                    before.decode("utf-8").splitlines(keepends=True),
                    after.decode("utf-8").splitlines(keepends=True),
                    fromfile="a/" + name, tofile="b/" + name,
                ))
    require(inventory == set(files), "JPEG XR original archive has missing files")
    require("".join(expected_patch).encode("utf-8") == patch.read_bytes(),
            "JPEG XR reviewed patch differs from actual archive-to-vendor delta")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path,
                        help="Additionally verify the exact original .crate and complete diff")
    args = parser.parse_args()
    try:
        exceptions = verify(archive=args.archive)
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError) as error:
        parser.exit(1, f"JPEG XR vendor verification failed: {error}\n")
    print(f"JPEG XR source/patch/license verified; {len(exceptions)} exact inherited line exceptions.")


if __name__ == "__main__":
    main()
