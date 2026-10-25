"""Independent byte-layout and Pillow oracle for freshly encoded ICO/CUR sets."""
import hashlib
import io
import json
from pathlib import Path
import struct
import sys

import PIL
from PIL import Image

folder = Path(sys.argv[1])
results = []
for profile, sizes, prefix in [("png", [1, 17, 32, 48, 256], ""), ("bitmap", [1, 17, 32, 48, 256], ""), ("special", [16, 32], "special-")]:
    for suffix in ["ico", "cur"]:
        data = (folder / f"{profile}.{suffix}").read_bytes()
        assert struct.unpack_from("<HHH", data) == (0, 1 if suffix == "ico" else 2, len(sizes))
        previous = 6 + 16 * len(sizes)
        for index, size in enumerate(sizes):
            width, height, colors, reserved, first, second, length, offset = struct.unpack_from("<BBBBHHII", data, 6 + index * 16)
            assert (width or 256, height or 256, colors, reserved) == (size, size, 0, 0)
            assert offset == previous
            previous = offset + length
            payload = data[offset:previous]
            assert len(payload) == length
            assert (first, second) == ((1, 32) if suffix == "ico" else (size - 1, 0))
            expected = (folder / f"{prefix}{size}.rgba").read_bytes()
            if profile != "bitmap":
                assert payload[:8] == b"\x89PNG\r\n\x1a\n"
                assert payload[24:26] == bytes((8, 6)), "must explicitly emit RGBA PNG, even when opaque"
            else:
                assert struct.unpack_from("<IiiHHI", payload) == (40, size, size * 2, 1, 32, 0)
                stride = ((size + 31) // 32) * 4
                mask = payload[40 + size * size * 4:]
                assert len(mask) == stride * size
                for y in range(size):
                    for x in range(size):
                        alpha = expected[((size - y - 1) * size + x) * 4 + 3]
                        assert bool(mask[y * stride + x // 8] & (0x80 >> (x % 8))) == (alpha == 0)
            # Pillow's CUR reader lacks PNG cursor support. Re-envelope each
            # retained payload as ICO solely for independent pixel decoding.
            single = struct.pack("<HHHBBBBHHII", 0, 1, 1, size % 256, size % 256, 0, 0, 1, 32, length, 22) + payload
            with Image.open(io.BytesIO(single)) as image:
                assert image.size == (size, size)
                assert image.convert("RGBA").tobytes() == expected
            results.append({"container": suffix, "encoding": profile, "size": size, "rgba_sha256": hashlib.sha256(expected).hexdigest()})
        assert previous == len(data)
print(json.dumps({"pillow": PIL.__version__, "exact_entries": results, "cursor_hotspots_and_masks": "passed", "windows_application": "not run"}))
