"""Reproduce orientation and negative seeds by explicit owned-fixture edits."""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
BASE = ROOT / "crates/services/paintcraft-formats/tests"
records = []


def save(name, source, result, recipe):
    original = (BASE / source).read_bytes()
    (HERE / name).write_bytes(result)
    records.append({"file": name, "source": str((BASE / source).relative_to(ROOT)), "source_sha256": hashlib.sha256(original).hexdigest(), "recipe": recipe, "bytes": len(result), "sha256": hashlib.sha256(result).hexdigest()})


source = "jpeg-fixtures/rgb-orientation-6-II.jpg"
original = (BASE / source).read_bytes()
payload = b"Exif\0\0II" + struct.pack("<HI", 42, 0xFFFFFFFE)
save("invalid-exif-ifd.jpg", source, original[:2] + b"\xff\xe1" + struct.pack(">H", len(payload) + 2) + payload + original[2:], "Insert APP1 Exif II42 with first IFD offset0xFFFFFFFE after SOI; original JPEG unchanged.")
source = "gif-still-fixtures/literal-three.gif"
result = bytearray((BASE / source).read_bytes())
assert result[40] == 0x50
result[40] = 0
save("missing-lzw-eoi.gif", source, result, "Set byte40 from0x50 to0, removing EOI after all declared samples; preserve all other bytes.")
source = "tiff-fixtures/icc-srgb-rgba8.tif"
result = bytearray((BASE / source).read_bytes())
offset = struct.unpack_from("<I", result, 4)[0]
count = struct.unpack_from("<H", result, offset)[0]
for i in range(count):
    at = offset + 2 + 12 * i
    if struct.unpack_from("<H", result, at)[0] == 274:
        assert struct.unpack_from("<HHII", result, at) == (274, 3, 1, 1)
        struct.pack_into("<I", result, at + 8, 6)
        break
else:
    raise AssertionError("Independent TIFF lacks Orientation tag")
save("orientation6.tif", source, result, "Set original inline SHORT Orientation274 from1 to6; offsets, compressed samples and all other tags unchanged. Qualified normalized positive control.")
struct.pack_into("<I", result, at + 8, 9)
save("invalid-orientation9.tif", source, result, "Set original inline SHORT Orientation274 from1 to9; preserve all offsets, samples and other tags. Invalid orientation must refuse ordinary and selected import.")
(HERE / "provenance.json").write_text(json.dumps({"schema": "paintcraft.stock-fuzz-derived/1", "rights": "Original PaintCraft fixtures; retain parent reference-tool provenance.", "records": records}, indent=2) + "\n")
