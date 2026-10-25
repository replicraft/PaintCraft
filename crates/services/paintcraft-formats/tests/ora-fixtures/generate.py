"""Independent standard-library ZIP/XML/PNG fixture, with arithmetic sample data.

Run with Python 3. No PaintCraft parser, writer or pixel code is used.
"""
import binascii
import hashlib
import json
from pathlib import Path
import struct
import zipfile
import zlib

ROOT = Path(__file__).parent


def png(width, height, pixels):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", binascii.crc32(kind + data))

    rows = b"".join(b"\0" + pixels[y * width * 4:(y + 1) * width * 4] for y in range(height))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


bottom = bytes([17, 51, 93, 255, 21, 52, 94, 255, 27, 53, 95, 255] * 2)
top = bytes([201, 31, 71, 255, 11, 12, 13, 0])
hidden = bytes([0, 255, 0, 255])
merged = bytearray(bottom)
merged[16:20] = top[:4]  # Full opacity Normal layer at (1, 1).
xml = '''<?xml version="1.0" encoding="UTF-8"?>
<image version="0.0.6" w="3" h="2" xres="144" yres="144">
 <stack>
  <layer name="Hidden" src="data/hidden.png" x="-1" visibility="hidden"/>
  <layer name="日本語 &amp; paint" src="data/top.png" x="1" y="1" composite-op="svg:src-over"/>
  <layer name="Background" src="data/bottom.png"/>
 </stack>
</image>'''
entries = [
    ("mimetype", b"image/openraster"),
    ("stack.xml", xml.encode()),
    ("data/bottom.png", png(3, 2, bottom)),
    ("data/top.png", png(2, 1, top)),
    ("data/hidden.png", png(1, 1, hidden)),
    ("mergedimage.png", png(3, 2, merged)),
    ("Thumbnails/thumbnail.png", png(3, 2, merged)),
]
for method, name in [(zipfile.ZIP_STORED, "stored"), (zipfile.ZIP_DEFLATED, "deflated")]:
    with zipfile.ZipFile(ROOT / f"{name}.ora", "w") as archive:
        for path, data in entries:
            info = zipfile.ZipInfo(path, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_STORED if path == "mimetype" else method
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)
manifest = {
    "generator": "Python standard library zipfile, zlib and arithmetic RGBA; no third-party artwork",
    "layers_bottom_to_top": [
        {"name": name, "rgba_sha256": hashlib.sha256(raw).hexdigest()}
        for name, raw in [("Background", bottom), ("日本語 & paint", top), ("Hidden", hidden)]
    ],
    "merged_rgba_sha256": hashlib.sha256(merged).hexdigest(),
    "archives": {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
                 for name in ["stored.ora", "deflated.ora"]},
}
(ROOT / "oracle.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=4) + "\n")
