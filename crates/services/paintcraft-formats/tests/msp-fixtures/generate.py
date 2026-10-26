"""Owned MSP wire specimens; Python standard-library packing only."""
import hashlib
import json
import struct
from pathlib import Path
ROOT = Path(__file__).parent


def header(version, width, height, unusual=False):
    words = list(struct.unpack("<HH", b"DanM" if version == 1 else b"LinS"))
    words += [width, height, 2, 3, 5, 7, 640, 480, 0x1234, 0x5678, 0, 0, 0, 0] if unusual else [width, height, 1, 1, 1, 1, width, height, 0, 0, 0, 0, 0, 0]
    checksum = 0
    for word in words:
        checksum ^= word
    words[12] = checksum
    return struct.pack("<16H", *words)


def expected(width, height, packed):
    row = (width + 7) // 8
    return bytes(channel for y in range(height) for x in range(width)
                 for channel in ([255] * 4 if packed[y * row + x // 8] & (128 >> (x % 8)) else [0, 0, 0, 255]))


def main():
    files = {}
    for width, rows in [(1, [b"\x80", b"\x00"]), (7, [b"\xaa", b"\x55", b"\xfe"]),
                        (9, [b"\x80\x80", b"\xff\xff", b"\x55\x55"]),
                        (257, [bytes([0xAA] * 32 + [0x80]), bytes([0x55] * 32 + [0x01])])]:
        packed = b"".join(rows)
        files[f"raw-{width}.msp"] = (header(1, width, len(rows), width == 9) + packed, width, len(rows), packed, "standard-library owned wire packing")
    rows = [b"\x02\x80\x80", b"\x00\x02\xff", b"\x02\x55\x55"]
    files["rle-9.msp"] = (header(2, 9, 3, True) + struct.pack("<3H", *(len(row) for row in rows)) + b"".join(rows), 9, 3, b"\x80\x80\xff\xff\x55\x55", "owned literal/repeat packets")
    rows = [b"\x00\xff\x55\x01\x80", b"\x00\xff\0\x00\x01\0"]
    files["rle-2048.msp"] = (header(2, 2048, 2) + struct.pack("<2H", *(len(row) for row in rows)) + b"".join(rows), 2048, 2, bytes([0x55] * 255 + [0x80] + [0] * 256), "owned 255-byte repeats and adjacent row boundaries")
    # This specimen is generated separately by the installed independent
    # Pillow writer; keep it when regenerating the standard-library specimens.
    independent = ROOT / "pillow-7.msp"
    if independent.exists():
        files[independent.name] = (independent.read_bytes(), 7, 3, bytes([0xAA, 0x54, 0xFE]), "Pillow 13.0.0 fresh writer, owned 7x3 pixels; see pillow_fixture.py")
    manifest = {"license": "MIT, repository-authored pixel content and wire specimens", "files": {}}
    for name, (data, width, height, packed, provenance) in files.items():
        (ROOT / name).write_bytes(data)
        manifest["files"][name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "width": width, "height": height,
            "packed_hex": packed.hex(), "rgba_sha256": hashlib.sha256(expected(width, height, packed)).hexdigest(), "provenance": provenance}
    (ROOT / "sources.json").write_text(json.dumps(manifest, indent=4) + "\n")


if __name__ == "__main__":
    main()
