"""Package the same-build, still-internal decoder helper without enabling readers."""
import json
import mmap
import shutil

MANIFEST = "paintcraft-codec-worker.json"
BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V1\0"
BUILD_SUFFIX = b"\0PCDEC002\0HEAP268435456\0END_CODEC_WORKER_BUILD\0"


def verify_build_record(helper, version):
    # No foreign executable is launched. The referenced Cargo-version record
    # survives stripping; a sibling filename alone proves no version identity.
    expected = BUILD_PREFIX + version.encode("ascii") + BUILD_SUFFIX
    with helper.open("rb") as stream:
        with mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ) as image:
            offset = image.find(BUILD_PREFIX)
            if offset < 0:
                raise ValueError("Codec helper lacks its embedded build record")
            while offset >= 0:
                if image[offset:offset + len(expected)] != expected:
                    raise ValueError("Codec helper compiled version/protocol/heap record mismatch")
                offset = image.find(BUILD_PREFIX, offset + len(expected))


def filename(target):
    return "paintcraft-codec-worker" + (".exe" if "windows" in target else "")


def prepare(binary, version, target=None):
    import package_release as release
    if target is None:
        matches = []
        for candidate in release.TARGETS:
            try:
                release.validate_package_input(version, candidate, binary)
                matches.append(candidate)
            except ValueError:
                pass
        if len(matches) != 1:
            raise ValueError("Cannot identify exactly one native package architecture")
        target = matches[0]
    release.validate_package_input(version, target, binary)
    helper = binary.parent / filename(target)
    release.validate_package_input(version, target, helper)
    verify_build_record(helper, version)
    return target, helper


def metadata(directory, version, target):
    import package_release as release
    helper = directory / filename(target)
    release.validate_package_input(version, target, helper)
    verify_build_record(helper, version)
    return {
        "schema_version": 1,
        "embedded_build_record_verified": True,
        "application_version": version,
        "target": target,
        "executable": helper.name,
        "sha256": release.digest(helper),
        "protocol_version": 2,
        "profile": "internal-heif-gray8-jxl-rgba8",
        "requested_heap_ceiling_bytes": 256 << 20,
        "public_reader_enabled": False,
    }


def write_manifest(directory, version, target):
    manifest = metadata(directory, version, target)
    path = directory / MANIFEST
    if path.is_symlink():
        raise ValueError("Refusing symlink helper manifest")
    path.write_text(json.dumps(manifest, indent=4) + "\n", encoding="utf-8")
    return path


def copy(binary, directory, version, target=None):
    target, helper = prepare(binary, version, target)
    destination = directory / filename(target)
    if destination.is_symlink():
        raise ValueError("Refusing symlink helper destination")
    shutil.copy2(helper, destination)
    destination.chmod(0o755)
    write_manifest(directory, version, target)
    return destination


def verify_manifest(directory, version, target):
    path = directory / MANIFEST
    if not path.is_file() or path.is_symlink():
        raise ValueError("Codec helper manifest must be a regular file")
    if json.loads(path.read_text(encoding="utf-8")) != metadata(directory, version, target):
        raise ValueError("Codec helper manifest does not match packaged executable")
