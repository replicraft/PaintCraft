"""Author density metadata only; verify unchanged entropy through original JXRLib."""
import hashlib
import json
import platform
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).parent
if len(sys.argv) != 2:
    raise SystemExit(
        "Usage: generate_resolution.py <pinned independently built jxr-reference>"
    )
reference = Path(sys.argv[1]).resolve()
source = (ROOT / "rgb8-4x3.jxr").read_bytes()
output = bytearray(source)
ifd = struct.unpack_from("<I", source, 4)[0]
count = struct.unpack_from("<H", source, ifd)[0]
changes = []
image_offset = None
for index in range(count):
    position = ifd + 2 + index * 12
    tag, kind, size, value = struct.unpack_from("<HHII", source, position)
    if tag == 0xbcc0:
        image_offset = value
    if tag in (0xbc82, 0xbc83):
        assert (kind, size, value) == (11, 1, 0)
        dpi = 96.0 if tag == 0xbc82 else 144.0
        struct.pack_into("<f", output, position + 8, dpi)
        changes.append({
            "tag": f"{tag:04x}",
            "original_f32_bits": f"{value:08x}",
            "authored_f32_bits": f"{struct.unpack('<I', struct.pack('<f', dpi))[0]:08x}",
        })
assert image_offset is not None and len(changes) == 2
assert source[image_offset:] == output[image_offset:]
fixture = ROOT / "rgb8-dpi96x144.jxr"
fixture.write_bytes(output)
with tempfile.TemporaryDirectory(prefix="paintcraft-jxr-density-oracle-") as directory:
    decoded = Path(directory) / "rgb.raw"
    subprocess.run(
        [str(reference), "decode", "4", "3", str(fixture.resolve()), str(decoded)],
        check=True,
    )
    actual = decoded.read_bytes()
expected = bytes(
    (x * 47 + y * 13 + channel * 71) % 256
    for y in range(3)
    for x in range(4)
    for channel in range(3)
)
assert actual == expected
manifest = {
    "license": "MIT, repository-authored metadata and original owned gradient",
    "reference": "original C JXRLib, pinned source/provenance in oracle.json",
    "reference_executable_sha256": hashlib.sha256(reference.read_bytes()).hexdigest(),
    "host": platform.platform(),
    "source": "rgb8-4x3.jxr",
    "source_sha256": hashlib.sha256(source).hexdigest(),
    "fixture": fixture.name,
    "encoded_sha256": hashlib.sha256(output).hexdigest(),
    "unchanged_entropy_sha256": hashlib.sha256(source[image_offset:]).hexdigest(),
    "metadata_changes": changes,
    "independent_c_decoded_rgb_sha256": hashlib.sha256(actual).hexdigest(),
    "independent_c_pixels_exact": True,
    "primary_resolution_semantics": (
        "ITU-T T.832(2012) A.7.25/26, printed page 172: "
        "explicit positive pixels/inch; zero/absent infer 96"
    ),
}
(ROOT / "resolution-oracle.json").write_text(json.dumps(manifest, indent=4) + "\n")
