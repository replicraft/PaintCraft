"""Independent psd-tools pattern/channel check for owned ABR specimens."""
import hashlib
import importlib.metadata
import io
import json
import struct
from pathlib import Path

from psd_tools.psd.patterns import Patterns

ROOT = Path(__file__).parent
assert importlib.metadata.version("psd-tools") == "1.26.0"
BOX = (5, 7, 7, 9)


def plane(depth, compression, hex_data, bounds=BOX):
    return (1, depth, bounds, depth, compression, hex_data)


ABSENT = (0, None, None, None, 0, None)
EMPTY = (1, None, None, None, 0, None)
RED = plane(8, 0, "0a141e28")
GREEN = plane(8, 0, "646e7882")
BLUE = plane(8, 0, "c8d2dce6")
ALPHA = plane(8, 0, "004080ff")
EXPECTED = {
    "rgb8": ("Pattern 色 🚀", 3, BOX, [RED, plane(8, 1, "646e7882"), BLUE, ABSENT, plane(8, 1, "004080ff")]),
    "gray16": ("Gray16", 1, BOX, [plane(16, 1, "000000ff8001ffff"), EMPTY, plane(16, 0, "000000018000ffff")]),
    "indexed8": ("Indexed", 2, BOX, [plane(8, 1, "00010100"), ABSENT, plane(8, 0, "ff804000")]),
    "float32": ("Float32", 3, BOX, [plane(32, 0, "800000003f0000003fc00000be800000"),
        plane(32, 1, "3e8000003f4000003f80000000000000"), plane(32, 0, "3f800000000000003e0000003f600000"),
        ABSENT, plane(8, 1, "ff804000")]),
    "partial": ("Partial", 3, (5, 7, 7, 10), [plane(8, 0, "01020304", (5, 8, 7, 10)),
        plane(8, 0, "0a141e28323c", (5, 7, 7, 10)), plane(8, 1, "646e78828c96", (5, 7, 7, 10)),
        ABSENT, plane(8, 0, "004080ff", (5, 8, 7, 10))]),
    "indexed-extra3": ("Indexed extra three", 2, BOX, [plane(8, 1, "00010100"), ABSENT, plane(8, 0, "ff804000")]),
    "auxiliary": ("Auxiliary retained", 3, BOX, [RED, GREEN, BLUE, plane(16, 1, "000100ff8001ffff"), ABSENT, ALPHA]),
    "missing-color": ("Missing red", 3, BOX, [ABSENT, GREEN, BLUE, ABSENT, ALPHA]),
    "empty-color": ("Empty red", 3, BOX, [EMPTY, GREEN, BLUE, ABSENT, ALPHA]),
    "user-mask": ("User mask retained", 3, BOX, [RED, GREEN, BLUE, plane(8, 0, "ff804000"), ALPHA]),
}


def sections(data):
    pos = 4
    while pos < len(data):
        assert data[pos:pos + 4] == b"8BIM"
        size = struct.unpack_from(">I", data, pos + 8)[0]
        yield data[pos + 4:pos + 8], data[pos + 12:pos + 12 + size]
        pos += 12 + (size + 3) // 4 * 4
    assert pos == len(data)


def main():
    manifest = json.loads((ROOT / "sources.json").read_text())
    for filename in ["patterns.abr", "indexed-extra3.abr", "auxiliary.abr", "missing-color.abr", "empty-color.abr", "user-mask.abr"]:
        data = (ROOT / filename).read_bytes()
        assert len(data) == manifest["files"][filename]["bytes"]
        assert hashlib.sha256(data).hexdigest() == manifest["files"][filename]["sha256"]
        patterns = [p for kind, payload in sections(data) if kind == b"patt" for p in Patterns.read(io.BytesIO(payload))]
        assert len(patterns) == (5 if filename == "patterns.abr" else 1)
        count = 0
        for pattern in patterns:
            name, mode, bounds, expected = EXPECTED[pattern.pattern_id]
            assert pattern.name == name + "\0"
            assert pattern.image_mode.value == mode
            assert pattern.point == (-3, 4)
            assert pattern.data.rectangle == bounds
            if mode == 2:
                assert pattern.color_table[:2] == [(11, 22, 33), (200, 100, 50)]
                assert len(pattern.color_table) == 256
            actual = []
            for channel in pattern.data.channels:
                raw = channel.get_data(max_output_bytes=16 << 20) if channel.depth is not None else None
                actual.append((channel.is_written, channel.depth, channel.rectangle, channel.pixel_depth,
                               channel.compression.value, raw.hex() if raw is not None else None))
                count += 1
            assert actual == expected, (filename, pattern.pattern_id, actual, expected)
        print(json.dumps({"file": filename, "patterns": len(patterns), "exact_slots": count,
                          "oracle": "psd-tools 1.26.0", "scope": "owned exact-channel reader check"}))


if __name__ == "__main__":
    main()
