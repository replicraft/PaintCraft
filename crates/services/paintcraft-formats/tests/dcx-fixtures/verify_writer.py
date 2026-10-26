"""Independent directory/PCX oracle, with Pillow's DCX palette limitation retained."""
import hashlib
import io
import json
from pathlib import Path
import struct
import sys

from PIL import Image, __version__

assert __version__ == "13.0.0", __version__
folder = Path(sys.argv[1])
shapes = [(3, 2), (65, 2), (1, 5)]
results = []
for kind in ["rgb", "indexed", "mixed"]:
    data = (folder / f"{kind}.dcx").read_bytes()
    magic, *directory = struct.unpack_from("<1025I", data)
    assert magic == 987654321
    assert directory[0] == 4100
    assert directory[3:] == [0] * 1021
    assert directory[:3] == sorted(set(directory[:3]))
    whole = Image.open(io.BytesIO(data))
    assert whole.format == "DCX" and whole.n_frames == 3
    pages = []
    for page, (w, h) in enumerate(shapes):
        indexed = kind == "indexed" or kind == "mixed" and page != 1
        palette = [(i + page * 31 & 255, i * 43 + page * 11 & 255,
                    255 - i + page * 7 & 255) for i in range(256)]
        palette[17] = palette[3]
        indices = bytes([3, 17, 192, 255, 0, 7][i % 6] for i in range(w * h))
        expected = bytes(v for i in range(w * h) for v in (
            palette[indices[i]] if indexed else
            (193, 255, 192) if i % 5 == 0 else (17, 51, i + page * 43 & 255)))
        end = directory[page + 1] if page < 2 else len(data)
        image = Image.open(io.BytesIO(data[directory[page]:end]))
        image.load()
        assert image.size == (w, h)
        assert image.info["dpi"] == (72 + page, 144 + page)
        assert image.convert("RGB").tobytes() == expected
        if indexed:
            assert image.tobytes() == indices
            assert image.getpalette() == [v for color in palette for v in color]
        whole.seek(page)
        actual = whole.convert("RGB").tobytes()
        mismatched = sum(a != b for a, b in zip(actual, expected))
        # Pillow's indexed PCX decoder seeks the palette relative to container EOF.
        assert (mismatched > 0) == (indexed and page < 2)
        pages.append({"page": page, "bounded_pcx_exact": True,
                      "whole_dcx_rgb_mismatched_channels": mismatched,
                      "expected_rgb_sha256": hashlib.sha256(expected).hexdigest()})
    results.append({"filename": f"{kind}.dcx",
                    "sha256": hashlib.sha256(data).hexdigest(), "pages": pages})
print(json.dumps({"python": sys.version.split()[0], "pillow": __version__,
                  "results": results}, indent=4))
