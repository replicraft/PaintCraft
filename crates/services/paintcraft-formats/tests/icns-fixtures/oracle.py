"""Pinned independent Pillow container/PNG readback, no product dependency."""
import hashlib
import importlib.metadata
import io
import json
import struct
import sys
from pathlib import Path
from PIL import Image
ROOT = Path(__file__).parent
assert importlib.metadata.version("Pillow") == "13.0.0"
manifest = json.loads((ROOT / "sources.json").read_text())


def records(data):
    assert data[:4] == b"icns" and struct.unpack_from(">I", data, 4)[0] == len(data)
    pos = 8
    while pos < len(data):
        tag, length = struct.unpack_from(">4sI", data, pos)
        assert length >= 8 and pos + length <= len(data)
        yield tag.decode(), data[pos+8:pos+length]
        pos += length
    assert pos == len(data)


for name, receipt in manifest["files"].items():
    data = (ROOT / name).read_bytes()
    assert len(data) == receipt["bytes"] and hashlib.sha256(data).hexdigest() == receipt["sha256"]
    images = {item["tag"] for item in receipt["entries"]}
    image_records = [(tag, payload) for tag, payload in records(data) if tag in images]
    assert len(image_records) == len(receipt["entries"])
    with Image.open(io.BytesIO(data)) as family:
        sizes = family.icns.itersizes()
        for (tag, payload), expected in zip(image_records, receipt["entries"]):
            if not expected["qualified"]:
                continue
            assert tag == expected["tag"]
            # Duplicate types remain ordered in PaintCraft, while Pillow's
            # convenience lookup chooses the last. Verify each PNG separately.
            with Image.open(io.BytesIO(payload)) as image:
                rgba = image.convert("RGBA").tobytes()
                assert image.size == (expected["side"], expected["side"])
                assert hashlib.sha256(rgba).hexdigest() == expected["rgba_sha256"]
            size = (expected["logical"], expected["logical"], expected["scale"])
            assert size in sizes
            if name != "duplicates.icns":
                image = family.icns.getimage(size).convert("RGBA")
                assert hashlib.sha256(image.tobytes()).hexdigest() == expected["rgba_sha256"]
    print(json.dumps({"name": name, "entries": len(image_records), "oracle": "Pillow 13.0.0", "qualification": "explicit density/size and exact PNG pixels; legacy/JP2 not decoded; duplicate types use independent payload checks"}))
if len(sys.argv) == 2:
    output = Path(sys.argv[1])
    data = (output / "modern.icns").read_bytes()
    with Image.open(io.BytesIO(data)) as family:
        written = {tag for tag, _ in records(data) if tag != "TOC "}
        assert written == {"ic07","ic08","ic09","ic10","ic11","ic12","ic13","ic14"}
        for expected in manifest["files"]["modern.icns"]["entries"]:
            if expected["tag"] not in written:
                continue
            image = family.icns.getimage((expected["logical"], expected["logical"], expected["scale"])).convert("RGBA")
            assert hashlib.sha256(image.tobytes()).hexdigest() == expected["rgba_sha256"]
    print(json.dumps({"name": str(output / "modern.icns"), "oracle": "Pillow 13.0.0", "qualification": "fresh selected-library container output, all8 writable typed PNG entries"}))
