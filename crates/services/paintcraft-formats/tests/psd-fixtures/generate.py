"""Arithmetic RGB8 PSD/PSB fixtures, written without the selected Rust codec."""
import hashlib
import json
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).parent


def u16(value):
    return struct.pack(">H", value)


def u32(value):
    return struct.pack(">I", value)


def section(data, large=False):
    return struct.pack(">Q" if large else ">I", len(data)) + data


def channel(raw, width, compression, large):
    if compression == 0:
        encoded = raw
    elif compression == 1:
        # Literal PackBits rows, deliberately split into at most 128 samples.
        rows = []
        for offset in range(0, len(raw), width):
            row = raw[offset:offset + width]
            rows.append(b"".join(bytes([len(row[i:i + 128]) - 1]) + row[i:i + 128]
                                 for i in range(0, len(row), 128)))
        encoded = b"".join((u32 if large else u16)(len(row)) for row in rows) + b"".join(rows)
    elif compression in (2, 3):
        if compression == 3:
            raw = bytes(value if i % width == 0 else (value - raw[i - 1]) % 256
                        for i, value in enumerate(raw))
        encoded = zlib.compress(raw)
    return u16(compression) + encoded


def tag(key, payload):
    return b"8BIM" + key + section(payload) + b"\0" * (len(payload) % 2)


def rgba(width, height, seed):
    return bytes((seed + i * 29 + (i // 4) * 7) % 256 for i in range(width * height * 4))


LAYERS = [
    dict(name="日本語 duplicate", origin=[-1, 1], width=3, height=2, opacity=127, visible=False, source_id=71, seed=9),
    dict(name="日本語 duplicate", origin=[0, 0], width=5, height=3, opacity=255, visible=True, source_id=72, seed=51),
]


def fixture(version, compression, layered):
    large = version == 2
    records, payloads = [], []
    for layer in LAYERS if layered else []:
        width, height = layer["width"], layer["height"]
        pixels = rgba(width, height, layer["seed"])
        channels = [(index if index < 3 else -1,
                     channel(pixels[index::4], width, compression, large)) for index in range(4)]
        left, top = layer["origin"]
        name = layer["name"].encode("utf-16-be")
        extra = section(b"") + section(bytes([0, 0, 255, 255]) * 8) + b"\x04test\0\0\0"
        extra += tag(b"luni", u32(len(name) // 2) + name)
        extra += tag(b"lyid", u32(layer["source_id"]))
        extra += tag(b"iOpa", b"\xff\0\0\0")
        record = struct.pack(">iiiiH", top, left, top + height, left + width, 4)
        for ident, payload in channels:
            record += struct.pack(">h", ident) + struct.pack(">Q" if large else ">I", len(payload))
            payloads.append(payload)
        record += b"8BIMnorm" + bytes([layer["opacity"], 0, 0 if layer["visible"] else 2, 0])
        records.append(record + section(extra))
    info = struct.pack(">h", len(records)) + b"".join(records) + b"".join(payloads) if layered else b""
    info += b"\0" * (len(info) % 2)
    layer_mask = section(info, large) + section(b"")
    composite = bytes((i * 13 + 17) % 256 for i in range(5 * 3 * 3))
    resources = b"8BIM" + u16(1060) + b"\0\0" + section(b"<xmp/>")
    header = b"8BPS" + u16(version) + b"\0" * 6 + u16(3) + u32(3) + u32(5) + u16(8) + u16(3)
    return header + section(b"") + section(resources) + section(layer_mask, large) + channel(composite, 5, compression, large)


manifest = {"generator": "Python struct, zlib, arithmetic samples; no third-party artwork", "layers_bottom_to_top": []}
for layer in LAYERS:
    expected = {key: value for key, value in layer.items() if key != "seed"}
    expected["rgba_sha256"] = hashlib.sha256(rgba(layer["width"], layer["height"], layer["seed"])).hexdigest()
    manifest["layers_bottom_to_top"].append(expected)
manifest["files"] = {}
for version, suffix in [(1, "psd"), (2, "psb")]:
    for compression in range(4):
        for layered in [False, True]:
            name = f"{'layers' if layered else 'composite'}-{compression}.{suffix}"
            raw = fixture(version, compression, layered)
            (ROOT / name).write_bytes(raw)
            manifest["files"][name] = hashlib.sha256(raw).hexdigest()
(ROOT / "oracle.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=4) + "\n")
