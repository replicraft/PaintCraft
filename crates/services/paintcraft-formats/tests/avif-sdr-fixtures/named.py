#!/usr/bin/env python3
"""Add owned item-name metadata without altering any coded AV1 bytes."""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).parent


def boxes(data):
    offset = 0
    while offset < len(data):
        size = int.from_bytes(data[offset:offset + 4], "big")
        assert size >= 8 and offset + size <= len(data)
        yield data[offset + 4:offset + 8], data[offset + 8:offset + size]
        offset += size
    assert offset == len(data)


def box(kind, payload):
    return struct.pack(">I", len(payload) + 8) + kind + payload


def names(source, primary, alpha):
    top = list(boxes(source))
    assert [kind for kind, _ in top] == [b"ftyp", b"meta", b"mdat"]
    ftyp, metadata, coded = (value for _, value in top)
    children = list(boxes(metadata[4:]))
    delta = len(primary.encode()) + len(alpha.encode())
    rewritten = []
    for kind, payload in children:
        if kind == b"iinf":
            assert payload[:4] == bytes(4) and payload[4:6] == b"\0\2"
            entries = []
            for entry, body in boxes(payload[6:]):
                assert entry == b"infe" and body[:4] == bytes([2, 0, 0, 0])
                ident = int.from_bytes(body[4:6], "big")
                assert body[6:] == b"\0\0av01\0" and ident in (1, 2)
                name = primary if ident == 1 else alpha
                entries.append(box(entry, body[:-1] + name.encode() + b"\0"))
            payload = payload[:6] + b"".join(entries)
        elif kind == b"iloc":
            assert payload[:8] == bytes([0, 0, 0, 0, 68, 0, 0, 2])
            payload = bytearray(payload)
            for item in range(2):
                offset = 8 + item * 14
                assert payload[offset + 2:offset + 6] == b"\0\0\0\1"
                location = int.from_bytes(payload[offset + 6:offset + 10], "big") + delta
                payload[offset + 6:offset + 10] = location.to_bytes(4, "big")
        rewritten.append(box(kind, payload))
    result = box(b"ftyp", ftyp) + box(b"meta", metadata[:4] + b"".join(rewritten)) + box(b"mdat", coded)
    assert list(boxes(result))[-1][1] == coded
    return result


source_name = "420-vertical-limited-odd-alpha.avif"
source = (ROOT / source_name).read_bytes()
records = []
for name, primary, alpha in [("named-primary.avif", "Color image", ""),
                             ("named-alpha.avif", "", "Alpha mask"),
                             ("named-both.avif", "Color image", "Alpha mask")]:
    data = names(source, primary, alpha)
    (ROOT / name).write_bytes(data)
    records.append({"file": name, "primary_name": primary, "alpha_name": alpha,
                    "sha256": hashlib.sha256(data).hexdigest()})
(ROOT / "named-oracle.json").write_text(json.dumps({"source": source_name,
    "source_sha256": hashlib.sha256(source).hexdigest(),
    "coded_mdat_sha256": hashlib.sha256(list(boxes(source))[-1][1]).hexdigest(),
    "producer_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    "command": "python3 named.py", "fixtures": records}, indent=2) + "\n")
