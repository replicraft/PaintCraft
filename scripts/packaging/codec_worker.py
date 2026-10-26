"""Package the same-build finite decoder contracts without executing foreign binaries."""
import json
import mmap
import shutil

MANIFEST = "paintcraft-codec-worker.json"
NOTICE = "JPEGXR_LICENSE.txt"
AVIF_NOTICE = "DAV1D_LICENSE.txt"
NOTICES = (NOTICE, AVIF_NOTICE)
BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V4\0"
BUILD_SUFFIX = b"\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0END_CODEC_WORKER_BUILD\0"

AVIF_BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V5_AVIF\0"
AVIF_BUILD_SUFFIX = b"\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0END_CODEC_WORKER_BUILD\0"
NATIVE_PROFILE = "internal-heif-gray8-heif-srgb8-jxl-rgba8-jxr-rgb8-private-source1-parent-lease1"
AVIF_PROFILE = "native-heif-gray8-heif-srgb8-jxl-rgba8-jxr-rgb8-avif-srgb8-dav1d154-alloc1-private-source1-parent-lease1"
NATIVE_VARIANTS = ["native-jxl-srgb8-worker-v1", "native-jxr-srgb8-worker-v1", "native-heif-srgb8-worker-v1"]


def verify_build_record(helper, version):
    # No foreign executable is launched. The referenced Cargo-version record
    # survives stripping; a sibling filename alone proves no version identity.
    contracts = [(BUILD_PREFIX, BUILD_SUFFIX, NATIVE_PROFILE, NATIVE_VARIANTS),
                 (AVIF_BUILD_PREFIX, AVIF_BUILD_SUFFIX, AVIF_PROFILE,
                  NATIVE_VARIANTS + ["native-avif-srgb8-worker-v1"])]
    with helper.open("rb") as stream:
        with mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ) as image:
            found = [contract for contract in contracts if image.find(contract[0]) >= 0]
            if len(found) != 1:
                raise ValueError("Codec helper must contain exactly one known build contract")
            prefix, suffix, profile, variants = found[0]
            expected = prefix + version.encode("ascii") + suffix
            offset = image.find(prefix)
            while offset >= 0:
                if image[offset:offset + len(expected)] != expected:
                    raise ValueError("Codec helper compiled version/protocol/heap record mismatch")
                offset = image.find(prefix, offset + len(expected))
            return profile, variants


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
    profile, variants = verify_build_record(helper, version)
    return {
        "schema_version": 2,
        "embedded_build_record_verified": True,
        "application_version": version,
        "target": target,
        "executable": helper.name,
        "sha256": release.digest(helper),
        "protocol_version": 2,
        "profile": profile,
        "requested_heap_ceiling_bytes": 256 << 20,
        "qualified_import_variants": variants,
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
    import package_release as release
    for name in NOTICES:
        notice = directory / name
        if notice.is_symlink():
            raise ValueError("Refusing symlink codec notice destination")
        shutil.copy2(release.ROOT / "packaging" / name, notice)
    shutil.copy2(helper, destination)
    destination.chmod(0o755)
    write_manifest(directory, version, target)
    return destination


def verify_manifest(directory, version, target):
    import package_release as release
    for name in NOTICES:
        notice = directory / name
        if not notice.is_file() or notice.is_symlink() or notice.read_bytes() != (release.ROOT / "packaging" / name).read_bytes():
            raise ValueError("Codec dependency notice is missing or changed")
    path = directory / MANIFEST
    if not path.is_file() or path.is_symlink():
        raise ValueError("Codec helper manifest must be a regular file")
    if json.loads(path.read_text(encoding="utf-8")) != metadata(directory, version, target):
        raise ValueError("Codec helper manifest does not match packaged executable")
