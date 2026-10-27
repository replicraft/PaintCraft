"""Package the same-build finite decoder contracts without executing foreign binaries."""
import json
import mmap
import shutil
import re

MANIFEST = "paintcraft-codec-worker.json"
NOTICE = "JPEGXR_LICENSE.txt"
AVIF_NOTICE = "DAV1D_LICENSE.txt"
NOTICES = (NOTICE, AVIF_NOTICE)  # Exact historical V4/V5/V6 payloads.
FLOAT_NOTICE = "HDR_EXR_LICENSES_V1.txt"
MANIFEST_MAX = 65536
BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V4\0"
BUILD_SUFFIX = b"\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0END_CODEC_WORKER_BUILD\0"

AVIF_BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V5_AVIF\0"
AVIF_BUILD_SUFFIX = b"\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0END_CODEC_WORKER_BUILD\0"
NATIVE_PROFILE = "internal-heif-gray8-heif-srgb8-jxl-rgba8-jxr-rgb8-private-source1-parent-lease1"
AVIF_PROFILE = "native-heif-gray8-heif-srgb8-jxl-rgba8-jxr-rgb8-avif-srgb8-dav1d154-alloc1-private-source1-parent-lease1"
AVIF_SDR_BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V6_AVIF_SDR\0"
AVIF_SDR_BUILD_SUFFIX = b"\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0END_CODEC_WORKER_BUILD\0"
AVIF_SDR_PROFILE = "native-heif-gray8-heif-srgb8-jxl-rgba8-jxr-rgb8-avif-srgb8-ycbcr8-dav1d154-alloc1-private-source1-parent-lease1"
NATIVE_VARIANTS = ["native-jxl-srgb8-worker-v1", "native-jxr-srgb8-worker-v1", "native-heif-srgb8-worker-v1"]

FLOAT_BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V7_FLOAT\0"
FLOAT_BUILD_SUFFIX = b"\0PCDEC002\0HEAP268435456\0STDIN_PARENT_LEASE1\0JXR_PRIVATE_SOURCE1\0AVIF_DAV1D154_ALLOC1_ASM0_8BIT1\0AVIF_SDR709_YUV444_420_CSP12_WORK1\0FLOAT_SETTINGS32_REPORT96_INPUT16777216_PIXELS1048576\0END_CODEC_WORKER_BUILD\0"
FLOAT_PROFILE = "native-v7-hdr-exr-sdr8-settings32-report96-16mib-1mp-parent-lease1"
FLOAT_VARIANTS = NATIVE_VARIANTS + ["native-avif-srgb8-worker-v1", "native-avif-sdr-ycbcr8-worker-v1", "native-hdr-sdr8-worker-v1", "native-exr-sdr8-worker-v1"]


JXR_BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V8_JXR_RGB8\0"
JXR_BUILD_SUFFIX = FLOAT_BUILD_SUFFIX.removesuffix(b"END_CODEC_WORKER_BUILD\0") + (
    b"PCENC001\0PCJXR001\0JXR_RGB8_LOSSLESS_SEEK_CAP2_INPUT3145728_PIXELS1048576_OUTPUT67108864\0"
    b"JXR_WRITER_RGB8_READ1_INPUT67108864_PIXELS1048576_SIDES65536\0END_CODEC_WORKER_BUILD\0"
)
JXR_PROFILE = "native-v8-jxr-rgb8-lossless-seekcap2-jxrread1-parent-lease1"
JXR_VARIANTS = FLOAT_VARIANTS + ["native-jxr-writer-rgb8-worker-v1"]
JXR_EXPORT_VARIANTS = ["native-jxr-rgb8-lossless-worker-v1"]
PRIVATE_JXR_BUILD_PREFIX = b"PAINTCRAFT_CODEC_WORKER_BUILD_V8_JXR_ENCODE_PRIVATE\0"


def variants(profile):
    known = {NATIVE_PROFILE: NATIVE_VARIANTS,
             AVIF_PROFILE: NATIVE_VARIANTS + ["native-avif-srgb8-worker-v1"],
             AVIF_SDR_PROFILE: FLOAT_VARIANTS[:5], FLOAT_PROFILE: FLOAT_VARIANTS,
             JXR_PROFILE: JXR_VARIANTS}
    if profile not in known:
        raise ValueError("Unknown codec qualification profile")
    return known[profile]


def notices(profile):
    variants(profile)  # Never infer compatibility from a version ordering.
    return NOTICES + ((FLOAT_NOTICE,) if profile in (FLOAT_PROFILE, JXR_PROFILE) else ())


def parse_manifest(data):
    import package_release as release
    def unique(pairs):
        value = {}
        for key, item in pairs:
            if key in value:
                raise ValueError("Duplicate helper manifest field")
            value[key] = item
        return value
    if len(data) > MANIFEST_MAX:
        raise ValueError("Codec helper manifest exceeds its allowance")
    value = json.loads(data, object_pairs_hook=unique)
    fields = {"schema_version", "embedded_build_record_verified", "application_version", "target", "executable", "sha256", "protocol_version", "profile", "requested_heap_ceiling_bytes", "qualified_import_variants"}
    if not isinstance(value, dict):
        raise ValueError("Codec helper manifest must be an object")
    profile = value.get("profile")
    if profile == JXR_PROFILE:
        fields |= {"encode_protocol_version", "qualified_export_variants"}
    if set(value) != fields:
        raise ValueError("Codec helper manifest fields differ from exact schema")
    if not isinstance(profile, str):
        raise ValueError("Invalid codec helper profile")
    expected = variants(profile)
    version = value["application_version"]
    target = value["target"]
    schema = 3 if profile == JXR_PROFILE else 2
    if profile == JXR_PROFILE and (
            type(value["encode_protocol_version"]) is not int
            or value["encode_protocol_version"] != 1
            or value["qualified_export_variants"] != JXR_EXPORT_VARIANTS):
        raise ValueError("Codec helper export contract differs from exact qualification")
    if (type(value["schema_version"]) is not int or value["schema_version"] != schema
            or value["embedded_build_record_verified"] is not True
            or not isinstance(version, str) or not version or len(version) > 64 or not version.isascii()
            or not isinstance(target, str) or target not in release.TARGETS
            or value["executable"] != filename(target)
            or not isinstance(value["sha256"], str) or not re.fullmatch("[0-9a-f]{64}", value["sha256"])
            or type(value["protocol_version"]) is not int or value["protocol_version"] != 2
            or type(value["requested_heap_ceiling_bytes"]) is not int or value["requested_heap_ceiling_bytes"] != 256 << 20
            or value["qualified_import_variants"] != expected):
        raise ValueError("Codec helper manifest differs from exact qualification contract")
    return value


def read_manifest(directory):
    path = directory / MANIFEST
    if not path.is_file() or path.is_symlink() or path.stat().st_size > MANIFEST_MAX:
        raise ValueError("Codec helper manifest must be a bounded regular file")
    return parse_manifest(path.read_bytes())


def verify_build_record(helper, version):
    # No foreign executable is launched. The referenced Cargo-version record
    # survives stripping; a sibling filename alone proves no version identity.
    contracts = [(BUILD_PREFIX, BUILD_SUFFIX, NATIVE_PROFILE, NATIVE_VARIANTS),
                 (AVIF_BUILD_PREFIX, AVIF_BUILD_SUFFIX, AVIF_PROFILE,
                  NATIVE_VARIANTS + ["native-avif-srgb8-worker-v1"]),
                 (AVIF_SDR_BUILD_PREFIX, AVIF_SDR_BUILD_SUFFIX, AVIF_SDR_PROFILE,
                  NATIVE_VARIANTS + ["native-avif-srgb8-worker-v1", "native-avif-sdr-ycbcr8-worker-v1"]),
                 (FLOAT_BUILD_PREFIX, FLOAT_BUILD_SUFFIX, FLOAT_PROFILE, FLOAT_VARIANTS),
                 (JXR_BUILD_PREFIX, JXR_BUILD_SUFFIX, JXR_PROFILE, JXR_VARIANTS)]
    with helper.open("rb") as stream:
        with mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ) as image:
            if image.find(PRIVATE_JXR_BUILD_PREFIX) >= 0:
                raise ValueError("Private JXR worker is not a packaging contract")
            found = [contract for contract in contracts if image.find(contract[0]) >= 0]
            if len(found) != 1:
                raise ValueError("Codec helper must contain exactly one known build contract")
            prefix, suffix, profile, variants = found[0]
            expected = prefix + version.encode("ascii") + suffix
            offset = image.find(prefix)
            if profile == JXR_PROFILE and image.find(prefix, offset + len(prefix)) >= 0:
                raise ValueError("JXR helper must contain exactly one public build record")
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
    return installed_metadata(directory, version, target)


def installed_metadata(directory, version, target):
    import package_release as release
    helper = directory / filename(target)
    release.validate_binary(target, helper)
    profile, variants = verify_build_record(helper, version)
    value = {
        "schema_version": 3 if profile == JXR_PROFILE else 2,
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
    if profile == JXR_PROFILE:
        value["encode_protocol_version"] = 1
        value["qualified_export_variants"] = JXR_EXPORT_VARIANTS
    return value


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
    profile, _ = verify_build_record(helper, version)
    for name in notices(profile):
        notice = directory / name
        if notice.is_symlink():
            raise ValueError("Refusing symlink codec notice destination")
        shutil.copy2(release.ROOT / "packaging" / name, notice)
    shutil.copy2(helper, destination)
    destination.chmod(0o755)
    write_manifest(directory, version, target)
    return destination


def verify_manifest(directory, version, target):
    # Packaging retains its checkout-version requirement.
    import package_release as release
    release.validate_package_input(version, target, directory / filename(target))
    verify_installed_manifest(directory, version, target)


def verify_installed_manifest(directory, version, target):
    import package_release as release
    expected = installed_metadata(directory, version, target)
    if read_manifest(directory) != expected:
        raise ValueError("Codec helper manifest does not match packaged executable")
    for name in notices(expected["profile"]):
        notice = directory / name
        if not notice.is_file() or notice.is_symlink() or notice.read_bytes() != (release.ROOT / "packaging" / name).read_bytes():
            raise ValueError("Codec dependency notice is missing or changed")


def installed_notices(directory):
    return notices(read_manifest(directory)["profile"])
