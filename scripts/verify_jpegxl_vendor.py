#!/usr/bin/env python3
"""Verify the selected JPEG XL release, single-file patch and inherited line limit."""
import argparse
import difflib
import hashlib
import json
import tarfile
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
VENDOR = "vendor/zune-jpegxl-0.5.2-paintcraft1"
ARCHIVE_SHA256 = "cba11ecd07cc23351500e840ae03841b7e4997923ec8e4832ee3ae199ea0b6b4"
PATCH_SHA256 = "ee275a9b4ded66ae0f5eb53b644af59e21025969e882ad82fddd9055b5c0f667"
INVENTORY_SHA256 = "6130d3c64ee55d434de8e1f0f25e1114c571582a873b4a2d3327978c564c4f5a"
COMMIT = "a019244c4a8a4eb713e85fa77f9e18f841d6271b"
LONG_FILE = "src/encoder.rs"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(root=ROOT, archive=None):
    root = Path(root)
    source = root / VENDOR
    provenance = Path(str(source) + ".provenance.json")
    require(not provenance.is_symlink(), "JPEG XL provenance is a symlink")
    metadata = json.loads(provenance.read_text())
    require((metadata["schema"], metadata["package"], metadata["version"],
             metadata["local_revision"]) == (1, "zune-jpegxl", "0.5.2", "paintcraft1"),
            "JPEG XL identity mismatch")
    require(metadata["upstream_commit"] == COMMIT and metadata["upstream_vcs_dirty"] is True,
            "JPEG XL upstream VCS declaration mismatch")
    require(metadata["archive_sha256"] == ARCHIVE_SHA256, "JPEG XL archive identity mismatch")
    require(metadata["changed_files"] == [LONG_FILE]
            and metadata["line_exemptions"] == [LONG_FILE], "JPEG XL patch/line scope mismatch")
    files = metadata["files"]
    require(len(files) == 14 and digest(json.dumps(
        files, sort_keys=True, separators=(",", ":")).encode()) == INVENTORY_SHA256,
        "JPEG XL reviewed inventory mismatch")
    require(not source.is_symlink(), "JPEG XL source directory is a symlink")
    authored = metadata["authored_files"]
    preservation = b"disable_all_formatting = true\n"
    require(authored == {"rustfmt.toml": {"sha256": digest(preservation),
            "bytes": len(preservation), "lines": 1}}, "JPEG XL authored config inventory mismatch")
    require((source / "rustfmt.toml").read_bytes() == preservation,
            "JPEG XL package-local formatting policy mismatch")
    actual = set()
    for path in source.rglob("*"):
        require(not path.is_symlink(), f"JPEG XL source symlink: {path}")
        if path.is_file():
            actual.add(path.relative_to(source).as_posix())
        else:
            require(path.is_dir(), "JPEG XL source contains non-file entry")
    require(actual == set(files) | set(authored), "JPEG XL missing/unexpected source file")
    for name, row in files.items():
        relative = PurePosixPath(name)
        require(not relative.is_absolute() and ".." not in relative.parts,
                "JPEG XL unsafe inventory path")
        path = source / name
        require(path.stat().st_size == row["bytes"], f"JPEG XL source size mismatch: {name}")
        data = path.read_bytes()
        require(digest(data) == row["sha256"] and len(data.splitlines()) == row["lines"],
                f"JPEG XL source bytes mismatch: {name}")
    require(metadata["patch_file"] == "zune-jpegxl-0.5.2-paintcraft1.patch"
            and metadata["patch_sha256"] == PATCH_SHA256, "JPEG XL reviewed patch identity mismatch")
    patch = root / "vendor" / metadata["patch_file"]
    require(not patch.is_symlink() and digest(patch.read_bytes()) == PATCH_SHA256,
            "JPEG XL patch bytes mismatch")
    added = sum(line.startswith(b"+") and not line.startswith(b"+++")
                for line in patch.read_bytes().splitlines())
    require(added <= 800, "JPEG XL authored patch exceeds source line limit")
    licenses = {name: files[name]["sha256"] for name in
                ["LICENSE-APACHE", "LICENSE-MIT", "LICENSE-ZLIB"]}
    require(metadata["licenses"] == licenses, "JPEG XL original license inventory mismatch")
    attributes = (root / "vendor/.gitattributes").read_text().splitlines()
    for rule in [f"{source.name}/** -text", f"{source.name}.patch -text -whitespace",
                 f"{source.name}.provenance.json -text"]:
        require(rule in attributes, "JPEG XL exact-byte checkout attributes missing")
    if archive is not None:
        verify_archive(Path(archive), source, metadata, patch)
    return {f"{VENDOR}/{LONG_FILE}"}


def verify_archive(archive, source, metadata, patch):
    require(archive.stat().st_size == metadata["archive_bytes"], "JPEG XL archive size mismatch")
    require(digest(archive.read_bytes()) == ARCHIVE_SHA256, "JPEG XL archive checksum mismatch")
    files = metadata["files"]
    seen = set()
    delta = []
    with tarfile.open(archive) as upstream:
        for member in upstream:
            path = PurePosixPath(member.name)
            require(not path.is_absolute() and ".." not in path.parts
                    and path.parts[0] == "zune-jpegxl-0.5.2", "JPEG XL unsafe archive path")
            require(member.isdir() or member.isfile(), "JPEG XL unexpected archive entry")
            if member.isdir():
                continue
            name = path.relative_to("zune-jpegxl-0.5.2").as_posix()
            require(name in files and name not in seen, "JPEG XL archive inventory mismatch")
            seen.add(name)
            before = upstream.extractfile(member).read()
            row = files[name]
            require(len(before) == row["upstream_bytes"]
                    and len(before.splitlines()) == row["upstream_lines"]
                    and digest(before) == row["upstream_sha256"], "JPEG XL upstream bytes mismatch")
            after = (source / name).read_bytes()
            if before != after:
                delta.extend(difflib.unified_diff(before.decode().splitlines(True),
                    after.decode().splitlines(True), fromfile="a/" + name, tofile="b/" + name))
    require(seen == set(files), "JPEG XL archive missing source file")
    require("".join(delta).encode() == patch.read_bytes(), "JPEG XL actual patch delta mismatch")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, help="Also verify original release and complete delta")
    args = parser.parse_args()
    try:
        verify(archive=args.archive)
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError) as error:
        parser.exit(1, f"JPEG XL vendor verification failed: {error}\n")
    print("JPEG XL exact release/patch/licenses verified; one inherited source exception")


if __name__ == "__main__":
    main()
