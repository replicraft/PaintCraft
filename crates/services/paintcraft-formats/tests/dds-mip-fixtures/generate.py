"""Run from repository root with the pinned independent Pillow oracle."""
import hashlib
import json
import struct
from pathlib import Path

import PIL
from PIL import Image

ROOT = Path("crates/services/paintcraft-formats/tests/dds-mip-fixtures")
ROOT.mkdir(exist_ok=True)
records = []
for dx10 in (False, True):
    profiles = [("rgba", 29), ("bgra", 91), ("bc1", 71), ("bc2", 74), ("bc3", 77)]
    if dx10:
        profiles += [("bc1-srgb", 72), ("bc2-srgb", 75), ("bc3-srgb", 78)]
    for name_kind, fmt in profiles:
        kind = name_kind.split("-")[0]
        header = bytearray(148 if dx10 else 128)
        header[:4] = b"DDS "

        def put(offset, value):
            struct.pack_into("<I", header, offset, value)

        compressed = kind.startswith("bc")
        for offset, value in [
            (4, 124), (8, 0x21007 | (0x80000 if compressed else 8)),
            (12, 5), (16, 7), (28, 3), (76, 32), (108, 0x401008),
        ]:
            put(offset, value)
        if dx10 or compressed:
            put(80, 4)
            header[84:88] = b"DX10" if dx10 else {
                "bc1": b"DXT1", "bc2": b"DXT3", "bc3": b"DXT5"
            }[kind]
        else:
            for offset, value in [
                (80, 0x41), (88, 32),
                (92, 0xFF0000 if kind == "bgra" else 0xFF), (96, 0xFF00),
                (100, 0xFF if kind == "bgra" else 0xFF0000), (104, 0xFF000000),
            ]:
                put(offset, value)
        if dx10:
            for offset, value in [(128, fmt), (132, 3), (140, 1), (144, 1)]:
                put(offset, value)
        payload = b""
        lengths = []
        for width, height, color, endpoint in [
            (7, 5, (255, 0, 0, 255), 0xF800),
            (3, 2, (0, 255, 0, 255), 0x07E0),
            (1, 1, (0, 0, 255, 255), 0x001F),
        ]:
            if compressed:
                block = struct.pack("<HHI", endpoint, 0, 0)
                if kind == "bc2":
                    block = b"\xff" * 8 + block
                if kind == "bc3":
                    block = b"\xff\x00" + b"\x00" * 6 + block
                data = block * (((width + 3) // 4) * ((height + 3) // 4))
            else:
                channels = color if kind == "rgba" else (color[2], color[1], color[0], color[3])
                data = bytes(channels) * (width * height)
            lengths.append(len(data))
            payload += data
        put(20, lengths[0] if compressed else 28)
        name = f'{"dx10" if dx10 else "legacy"}-{name_kind}.dds'
        blob = bytes(header) + payload
        (ROOT / name).write_bytes(blob)
        expected = bytes((255, 0, 0, 255)) * 35
        try:
            actual = Image.open(ROOT / name).convert("RGBA").tobytes()
            assert actual == expected, name
            oracle_hash = hashlib.sha256(actual).hexdigest()
        except NotImplementedError:
            assert dx10 and (kind == "bgra" or fmt in (72, 75, 78))
            oracle_hash = "unsupported-by-Pillow; independent-format-layout-expected-only"
        records.append(dict(
            file=name, sha256=hashlib.sha256(blob).hexdigest(),
            level_lengths=lengths, oracle_rgba_sha256=oracle_hash,
        ))
(ROOT / "top.rgba").write_bytes(bytes((255, 0, 0, 255)) * 35)
(ROOT / "provenance.json").write_text(json.dumps(dict(
    generator="generate.py; Python struct, no dds crate or PaintCraft decoder",
    oracle="Pillow " + PIL.__version__, dimensions=[[7, 5], [3, 2], [1, 1]],
    colors=["red", "green", "blue"], files=records,
), indent=2) + "\n")

print("13 authored fixtures; 9 exact Pillow top-pixel matches; DXGI91/72/75/78 explicitly unsupported")
