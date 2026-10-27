"""Independent complete MSP1/MSP2 framing and untouched Pillow pixel checks."""
import hashlib
import inspect
import json
from pathlib import Path
import re
import struct
import sys
from PIL import Image, MspImagePlugin, __version__

assert __version__ == "13.0.0"
plugin = Path(inspect.getfile(MspImagePlugin))
plugin_sha = hashlib.sha256(plugin.read_bytes()).hexdigest()
assert plugin_sha == "8e28773c59bae649f034c22b68d64ae4cdc9c26605947e34114e254dd458f8e6"
root = Path(sys.argv[1])
report = Path(sys.argv[2])
cases = []
for path in sorted(root.glob("*/*.msp")):
    match = re.fullmatch(r"(runs|odd)-(default|v1|v2|after-alias)\.msp", path.name)
    if not match:
        continue
    odd = match[1] == "odd"
    width, height = (9, 3) if odd else (2048, 3)
    want = bytes([0x80, 0x80, 0xFF, 0x80, 0x55, 0]) if odd else bytes(256) + bytes([255] * 256) + bytes(range(256))
    raw = path.read_bytes()
    words = struct.unpack("<16H", raw[:32])
    checksum = 0
    for word in words:
        checksum ^= word
    assert checksum == 0
    assert words[2:4] == (width, height)
    row_bytes = (width + 7) // 8
    if match[2] in ("default", "v1"):
        assert raw[:4] == b"DanM"
        actual = raw[32:]
    else:
        assert raw[:4] == b"LinS"
        lengths = struct.unpack(f"<{height}H", raw[32:32 + height * 2])
        assert all(lengths)
        offset = 32 + height * 2
        actual = bytearray()
        for size in lengths:
            row = raw[offset:offset + size]
            assert len(row) == size
            offset += size
            decoded = bytearray()
            cursor = 0
            while cursor < len(row):
                count = row[cursor]
                cursor += 1
                if count:
                    chunk = row[cursor:cursor + count]
                    assert len(chunk) == count
                    decoded.extend(chunk)
                    cursor += count
                else:
                    assert cursor + 2 <= len(row)
                    count, value = row[cursor:cursor + 2]
                    assert count
                    cursor += 2
                    decoded.extend([value] * count)
                assert len(decoded) <= row_bytes
            assert len(decoded) == row_bytes
            actual.extend(decoded)
        assert offset == len(raw)
    assert actual == want, path
    rgba = bytearray()
    for y in range(height):
        for x in range(width):
            sample = 255 if want[y * row_bytes + x // 8] & (128 >> (x % 8)) else 0
            rgba.extend([sample, sample, sample, 255])
    with Image.open(path) as picture:
        assert picture.size == (width, height)
        assert picture.convert("RGBA").tobytes() == rgba
    cases.append({"file": str(path.relative_to(root)), "file_sha256": hashlib.sha256(raw).hexdigest(),
                  "packed_sha256": hashlib.sha256(want).hexdigest(),
                  "rgba_sha256": hashlib.sha256(rgba).hexdigest(), "size": [width, height],
                  "version": "v1" if raw[:4] == b"DanM" else "v2"})
assert len(cases) == 14, len(cases)
value = {"scope": "14 complete actual CLI/MCP files; two owned nonuniform monochrome sources; no original Windows Paint proof",
         "pillow": __version__, "python": sys.version, "plugin_sha256": plugin_sha,
         "imaging_sha256": hashlib.sha256(Path(Image.core.__file__).read_bytes()).hexdigest(),
         "verifier_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), "cases": cases}
with report.open("x") as output:
    json.dump(value, output, indent=2)
    output.write("\n")
print("14 whole MSP files: independent packets/packed bytes and Pillow RGBA exact")
