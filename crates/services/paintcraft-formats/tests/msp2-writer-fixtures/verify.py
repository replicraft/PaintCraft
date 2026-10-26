"""Independent MSP2 packet framing and pinned Pillow visible-pixel oracle."""
import hashlib
import inspect
import json
from pathlib import Path
import struct
import sys
from PIL import Image, MspImagePlugin, __version__

assert __version__ == "13.0.0"
plugin_sha = hashlib.sha256(Path(inspect.getfile(MspImagePlugin)).read_bytes()).hexdigest()
assert plugin_sha == "8e28773c59bae649f034c22b68d64ae4cdc9c26605947e34114e254dd458f8e6"
folder = Path(sys.argv[1])
vectors = [
    ("tail-1", 1, 2, bytes([0x80, 0x7f])),
    ("tail-9", 9, 3, bytes([0x80, 0x81, 0xff, 0xfe, 0x55, 0xaa])),
    ("runs-2048", 2048, 3, bytes(256) + bytes([255]) * 256 + bytes(range(256))),
    ("wide-65535", 65535, 1, bytes(i % 251 for i in range(8192))),
]
results = []
for name, width, height, expected in vectors:
    path = folder / f"{name}.msp"
    data = path.read_bytes()
    words = struct.unpack_from("<16H", data)
    assert data[:4] == b"LinS"
    assert words[2:4] == (width, height)
    assert words[4:10] == (1, 1, 1, 1, width, height)
    checksum = 0
    for word in words:
        checksum ^= word
    assert checksum == 0
    lengths = struct.unpack_from(f"<{height}H", data, 32)
    offset = 32 + height * 2
    packed = bytearray()
    packets = {"literal": 0, "repeat": 0}
    for length in lengths:
        assert 0 < length <= 65535
        end = offset + length
        decoded = bytearray()
        while offset < end:
            count = data[offset]
            offset += 1
            if count:
                assert offset + count <= end
                decoded.extend(data[offset:offset + count])
                offset += count
                packets["literal"] += 1
            else:
                assert offset + 2 <= end
                count, value = data[offset:offset + 2]
                assert count > 0
                decoded.extend([value] * count)
                offset += 2
                packets["repeat"] += 1
        assert offset == end
        assert len(decoded) == (width + 7) // 8
        packed.extend(decoded)
    assert offset == len(data)
    assert packed == expected  # Includes all unused last-byte bits.
    rgba = bytes(v for y in range(height) for x in range(width) for v in (
        (255, 255, 255, 255) if expected[y * ((width + 7) // 8) + x // 8] & (0x80 >> (x % 8)) else (0, 0, 0, 255)))
    with Image.open(path) as image:
        assert image.format == "MSP"
        assert image.size == (width, height)
        assert image.convert("RGBA").tobytes() == rgba
    results.append({"filename": path.name, "bytes": len(data),
                    "sha256": hashlib.sha256(data).hexdigest(),
                    "packed_sha256": hashlib.sha256(packed).hexdigest(),
                    "rgba_sha256": hashlib.sha256(rgba).hexdigest(),
                    "width": width, "height": height, "packets": packets,
                    "row_lengths": list(lengths), "exact_packed_including_tail": True,
                    "exact_pillow_visible_rgba": True})
print(json.dumps({"python": sys.version.split()[0], "pillow": __version__,
                  "plugin_sha256": plugin_sha, "results": results}, indent=4))
