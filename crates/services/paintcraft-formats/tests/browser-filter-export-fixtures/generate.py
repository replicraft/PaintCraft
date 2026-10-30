"""Independent PCND3/identity fixtures: Python stdlib only, no PaintCraft codec."""
import hashlib
import json
import struct
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DOC = uuid.UUID("33333333-3333-4333-8333-333333333333").bytes
STATE = uuid.UUID("11111111-1111-4111-8111-111111111111").bytes
LAYER = uuid.UUID("22222222-2222-4222-8222-222222222222").bytes
RAW = bytes([0, 0, 0, 255, 255, 255, 255, 255])
PARENT = ROOT.parent / "pcnd-metadata-fixtures/rgb-profile.pcnd"
parent = PARENT.read_bytes()
assert parent[73:81] == b"PCSMETA1"
profile_size = int.from_bytes(parent[185:193], "big")
attachment = parent[73:73 + 192 + profile_size]
assert len(attachment) == 192 + profile_size and parent[-33] == 0


def sha(data):
    return hashlib.sha256(data).digest()


def make(enabled, retain):
    # One opaque 2x1 layer, transparent implicit default, one nil-ID Invert.
    body = b"PCND\x03\r\n\x1a" + DOC + STATE + b"\x02\x01\x00"
    body += bytes([retain]) + (attachment if retain else b"")
    body += b"\x01" + LAYER + b"\x05Image\x01\xff\x00\x00\x02\x01\x00\x00\x00\x00"
    body += b"\x01" + bytes(16) + bytes([enabled, 1])
    body += b"\x01\x00\x00\x08" + RAW + b"\x00"
    stack = sha(b"paintcraft.filter-stack/v1\0" + b"\x00\x01" + bytes(16) + bytes([enabled]) + b"\x00\x0d" + b"invert-rgb-v1")
    inventory = sha(b"paintcraft.filter-bake-witness/v1\0" + DOC + STATE + struct.pack(">IIQ", 2, 1, 1) + LAYER + stack + b"\x01\xff" + bytes(16) + struct.pack(">II", 2, 1) + bytes(4))
    return body + sha(body), inventory


rows = []
for enabled in (False, True):
    for retain in (False, True):
        name = ("invert" if enabled else "disabled") + ("-metadata" if retain else "") + ".pcnd"
        data, inventory = make(enabled, retain)
        expected = bytes([255, 255, 255, 255, 0, 0, 0, 255]) if enabled else RAW
        (ROOT / name).write_bytes(data)
        rows.append(dict(file=name, sha256=sha(data).hex(), bytes=len(data), enabled=enabled,
                         metadata=retain, filter_inventory_sha256=inventory.hex(),
                         expected_rgba=list(expected)))
(ROOT / "oracle.json").write_text(json.dumps(dict(
    generator="Python standard-library PCND3 and published canonical digest byte writer",
    source_parent_sha256=sha(parent).hex(),
    document=str(uuid.UUID(bytes=DOC)), state=str(uuid.UUID(bytes=STATE)),
    width=2, height=1, raw_rgba=list(RAW), fixtures=rows), indent=4) + "\n")
