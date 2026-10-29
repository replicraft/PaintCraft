#!/usr/bin/env python3
"""Verify the selected JXL decoder archive and its metadata-only source patch."""
import argparse
import difflib
import hashlib
import json
import tarfile
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
VENDOR = "vendor/jxl-0.8.0-paintcraft1"
ARCHIVE_SHA256 = "b8379fc83d79477900c9aed78cb8370a556d834081bebcb771c6010381c4e230"
ARCHIVE_BYTES = 401217
COMMIT = "9a08ca643fd2d01f803dd0be6a5bebc9cc11e597"
PATCH_SHA256 = "71becc38063e0e9e9195fcf80696de7354a89805d4aa159db87d2065c4cc735e"
INVENTORY_SHA256 = "b56a4f7b858d6ea45ff4bad02439f93e4dedb82ac68efdac15a34d05309f66dd"
PROVENANCE_SHA256 = "8b240d1cb68346c205e90573f8347800bef5f7a5221505b4d984669b7a442082"
README_SHA256 = "45075a5434637cfc912281cb915d1c910a37f70645a6c26643a197a6eddc8ec6"
LICENSE_SHA256 = "8405932022a556380c2d8c272eff154a923feb197233f348ce5f7334fb0a5ede"
CHANGED_FILES = ["src/api/codestream_parser/image_info.rs", "src/api/data_types.rs",
                 "src/headers/extra_channels.rs"]
LONG_FILES = {
    "src/api/box_parser.rs", "src/api/color.rs", "src/color/tf.rs",
    "src/entropy_coding/decode.rs", "src/features/blending.rs", "src/features/patches.rs",
    "src/features/spline.rs", "src/frame/decode.rs", "src/frame/group.rs",
    "src/frame/modular/mod.rs", "src/frame/modular/transforms/squeeze.rs",
    "src/frame/modular/transforms/step.rs", "src/frame/quant_weights.rs",
    "src/frame/render.rs", "src/headers/frame_header.rs", "src/render/channels.rs",
    "src/tests/api.rs", "src/util/xorshift128plus.rs",
}
PRESERVATION = b"disable_all_formatting = true\n"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def regular(path, label):
    require(not path.is_symlink() and path.is_file(), f"JXL decoder {label} is missing or a symlink")


def safe_name(name):
    require(isinstance(name, str) and name and "\\" not in name and ":" not in name
            and "\0" not in name, "JXL decoder unsafe inventory path")
    value = PurePosixPath(name)
    require(not value.is_absolute() and ".." not in value.parts
            and value.as_posix() == name and name != ".", "JXL decoder unsafe inventory path")
    return value


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "JXL decoder duplicate provenance field")
        result[key] = value
    return result


def verify(root=ROOT, archive=None):
    root = Path(root)
    source = root / VENDOR
    require(not (root / "vendor").is_symlink() and not source.is_symlink()
            and source.is_dir(), "JXL decoder source directory is missing or a symlink")
    provenance = Path(str(source) + ".provenance.json")
    regular(provenance, "provenance")
    require(provenance.stat().st_size <= 256 << 10, "JXL decoder provenance exceeds allowance")
    raw = provenance.read_bytes()
    metadata = json.loads(raw, object_pairs_hook=unique)
    files, authored = metadata["files"], metadata["authored_files"]
    require(isinstance(files, dict) and isinstance(authored, dict), "JXL decoder invalid inventory")
    for name in files.keys() | authored.keys():
        safe_name(name)
    require(digest(raw) == PROVENANCE_SHA256, "JXL decoder reviewed provenance mismatch")
    require((metadata["schema"], metadata["package"], metadata["version"], metadata["local_revision"])
            == (1, "jxl", "0.8.0", "paintcraft1"), "JXL decoder identity mismatch")
    require(metadata["upstream_commit"] == COMMIT and metadata["upstream_vcs_dirty"] is False,
            "JXL decoder upstream VCS declaration mismatch")
    require(metadata["archive_url"] == "https://static.crates.io/crates/jxl/jxl-0.8.0.crate"
            and metadata["archive_sha256"] == ARCHIVE_SHA256
            and metadata["archive_bytes"] == ARCHIVE_BYTES, "JXL decoder archive identity mismatch")
    require(len(files) == 179 and digest(json.dumps(files, sort_keys=True, separators=(",", ":")).encode())
            == INVENTORY_SHA256, "JXL decoder reviewed inventory mismatch")
    require(metadata["changed_files"] == CHANGED_FILES
            and metadata["line_exemptions"] == sorted(LONG_FILES), "JXL decoder patch/line scope mismatch")
    require(authored == {"rustfmt.toml": {"sha256": digest(PRESERVATION), "bytes": len(PRESERVATION),
            "lines": 1}}, "JXL decoder authored config inventory mismatch")
    require(metadata["licenses"] == {"LICENSE": LICENSE_SHA256}, "JXL decoder license inventory mismatch")
    actual, directories = set(), set()
    for path in source.rglob("*"):
        require(not path.is_symlink(), f"JXL decoder source symlink: {path}")
        name = path.relative_to(source).as_posix()
        if path.is_file():
            actual.add(name)
        else:
            require(path.is_dir(), "JXL decoder source contains non-file entry")
            directories.add(name)
    expected = set(files) | set(authored)
    allowed_dirs = {str(parent) for name in expected for parent in PurePosixPath(name).parents
                    if str(parent) != "."}
    require(actual == expected and directories == allowed_dirs,
            "JXL decoder missing/unexpected source file or directory")
    changed, inherited_long = set(), set()
    for name, row in (files | authored).items():
        data = (source / name).read_bytes()
        require(len(data) == row["bytes"], f"JXL decoder source size mismatch: {name}")
        require(digest(data) == row["sha256"] and len(data.splitlines()) == row["lines"],
                f"JXL decoder source bytes mismatch: {name}")
        if name in authored:
            require(data == PRESERVATION, "JXL decoder package-local formatting policy mismatch")
            continue
        if row["sha256"] != row["upstream_sha256"]:
            changed.add(name)
            require(row["lines"] <= 800, "JXL decoder modified source exceeds authored line limit")
        else:
            require((row["bytes"], row["lines"]) == (row["upstream_bytes"], row["upstream_lines"]),
                    "JXL decoder inherited file metadata differs")
            if name.endswith(".rs") and row["lines"] > 800:
                inherited_long.add(name)
    require(changed == set(CHANGED_FILES) and inherited_long == LONG_FILES,
            "JXL decoder inherited/modified source scope mismatch")
    require(digest((source / "LICENSE").read_bytes()) == LICENSE_SHA256,
            "JXL decoder original license bytes mismatch")
    vcs = json.loads((source / ".cargo_vcs_info.json").read_bytes())
    require(vcs["git"] == {"sha1": COMMIT} and vcs["path_in_vcs"] == "jxl",
            "JXL decoder original VCS bytes differ")
    patch = Path(str(source) + ".patch")
    regular(patch, "patch")
    patch_bytes = patch.read_bytes()
    require(metadata["patch_file"] == patch.name and metadata["patch_sha256"] == PATCH_SHA256
            and digest(patch_bytes) == PATCH_SHA256, "JXL decoder reviewed patch mismatch")
    added = sum(line.startswith(b"+") and not line.startswith(b"+++") for line in patch_bytes.splitlines())
    require(added <= 800, "JXL decoder authored patch exceeds source line limit")
    readme = Path(str(source) + ".README.md")
    regular(readme, "README")
    require(digest(readme.read_bytes()) == README_SHA256, "JXL decoder reviewed README mismatch")
    attributes = root / "vendor/.gitattributes"
    regular(attributes, "checkout attributes")
    rules = attributes.read_text().splitlines()
    for rule in [f"{source.name}/** -text", f"{source.name}.patch -text -whitespace",
                 f"{source.name}.provenance.json -text", f"{source.name}.README.md -text",
                 f"{source.name}/Cargo.toml.orig -whitespace"]:
        require(rule in rules, "JXL decoder exact-byte checkout attributes missing")
    if archive is not None:
        verify_archive(Path(archive), source, metadata, patch_bytes)
    return {f"{VENDOR}/{name}" for name in inherited_long}


def verify_archive(archive, source, metadata, patch_bytes):
    regular(archive, "archive")
    require(archive.stat().st_size == ARCHIVE_BYTES, "JXL decoder archive size mismatch")
    require(digest(archive.read_bytes()) == ARCHIVE_SHA256, "JXL decoder archive checksum mismatch")
    files, seen, changed = metadata["files"], set(), {}
    directories = {str(parent) for name in files for parent in PurePosixPath(name).parents
                   if str(parent) != "."}
    with tarfile.open(archive) as upstream:
        for member in upstream:
            path = safe_name(member.name)
            require(path.parts[0] == "jxl-0.8.0", "JXL decoder unsafe archive root")
            require(member.isdir() or member.isfile(), "JXL decoder unexpected archive entry")
            if member.isdir():
                require(path == PurePosixPath("jxl-0.8.0")
                        or path.relative_to("jxl-0.8.0").as_posix() in directories,
                        "JXL decoder unexpected archive directory")
                continue
            name = path.relative_to("jxl-0.8.0").as_posix()
            require(name in files and name not in seen, "JXL decoder archive inventory mismatch")
            seen.add(name)
            row = files[name]
            require(member.size == row["upstream_bytes"], "JXL decoder upstream size mismatch")
            before = upstream.extractfile(member).read(member.size + 1)
            require(len(before) == row["upstream_bytes"] and len(before.splitlines()) == row["upstream_lines"]
                    and digest(before) == row["upstream_sha256"], "JXL decoder upstream bytes mismatch")
            if name in CHANGED_FILES:
                changed[name] = before
    require(seen == set(files), "JXL decoder archive missing source file")
    delta = []
    for name in CHANGED_FILES:
        delta.extend(difflib.unified_diff(changed[name].decode().splitlines(True),
            (source / name).read_bytes().decode().splitlines(True),
            fromfile="a/" + name, tofile="b/" + name))
    require("".join(delta).encode() == patch_bytes, "JXL decoder actual archive-to-vendor delta mismatch")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, help="Verify original archive and complete patch delta too")
    args = parser.parse_args()
    try:
        exemptions = verify(archive=args.archive)
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError) as error:
        parser.exit(1, f"JXL decoder vendor verification failed: {error}\n")
    print(f"JXL decoder release/patch/licenses verified; {len(exemptions)} exact inherited exceptions")


if __name__ == "__main__":
    main()
