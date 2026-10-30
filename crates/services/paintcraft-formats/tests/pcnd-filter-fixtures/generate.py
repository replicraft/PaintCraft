"""Independent PCND3 byte writer using only Python standard-library primitives."""
import hashlib
import json
from pathlib import Path
import uuid

ROOT = Path(__file__).resolve().parent
PARENT = ROOT.parent / "pcnd-metadata-fixtures/rgb-profile.pcnd"
RAW = bytes([17, 32, 255, 0, 0, 127, 200, 255])
DOC = uuid.UUID("33333333-3333-4333-8333-333333333333").bytes
STATE = uuid.UUID("11111111-1111-4111-8111-111111111111").bytes
LAYER = uuid.UUID("22222222-2222-4222-8222-222222222222").bytes


def number(n):
    out = bytearray()
    while n >= 128:
        out.append((n & 127) | 128)
        n >>= 7
    out.append(n)
    return bytes(out)


def signed(n):
    return number(n * 2 if n >= 0 else -n * 2 - 1)


def instance(identity, enabled, tag, parameters=()):
    return identity.to_bytes(16, "big") + bytes([enabled, tag]) + b"".join(map(signed, parameters))


def make(version, filters=b"", presence=0, attachment=b"", count=None):
    body = b"PCND" + bytes([version]) + b"\r\n\x1a" + DOC + STATE + b"\x02\x01\x00"
    if version == 3:
        body += bytes([presence])
    body += attachment
    body += b"\x01" + LAYER + b"\x05Image\x01\xff\x00\x00\x02\x01\x00\x00\x00\x00"
    if version == 3:
        body += number(3) if count is None else count
        body += filters
    body += b"\x01\x00\x00\x08" + RAW + b"\x00"
    return body + hashlib.sha256(body).digest()


first = instance(0, 0, 2, (-100, 99))
second = instance(1, 1, 1)
third = instance(2, 1, 2, (17, -33))
stack = first + second + third
parent = PARENT.read_bytes()
# Parent has a29-byte title; its independently authored source header starts73.
assert parent[73:81] == b"PCSMETA1" and parent[-33] == 0
profile_length = int.from_bytes(parent[73 + 112:73 + 120], "big")
assert 73 + 192 + profile_length == len(parent) - 33
attachment = parent[73:-33]
fixtures = {
    "legacy-v1.pcnd": (make(1), True),
    "legacy-v2.pcnd": (make(2, attachment=attachment), True),
    "stack.pcnd": (make(3, stack), True),
    "source-stack.pcnd": (make(3, stack, 1, attachment), True),
    "disabled.pcnd": (make(3, instance(0, 0, 1), count=b"\x01"), True),
    "empty-v3.pcnd": (make(3, count=b"\x00"), False),
    "unknown-disabled.pcnd": (make(3, instance(0, 0, 9), count=b"\x01"), False),
    "unknown-enabled.pcnd": (make(3, instance(0, 1, 9), count=b"\x01"), False),
    "duplicate.pcnd": (make(3, first + first, count=b"\x02"), False),
    "bad-enabled.pcnd": (make(3, instance(0, 2, 1), count=b"\x01"), False),
    "bad-presence.pcnd": (make(3, stack, 2), False),
    "bad-brightness.pcnd": (make(3, instance(0, 0, 2, (101, 0)), count=b"\x01"), False),
    "bad-contrast.pcnd": (make(3, instance(0, 1, 2, (0, 100)), count=b"\x01"), False),
    "noncanonical-count.pcnd": (make(3, stack, count=b"\x83\x00"), False),
    "count257.pcnd": (make(3, count=number(257)), False),
    "future-v5.pcnd": (make(5), False),
}
rows = []
for name, (data, accepted) in fixtures.items():
    (ROOT / name).write_bytes(data)
    rows.append({"file": name, "accepted": accepted, "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)})
(ROOT / "oracle.json").write_text(json.dumps({
    "generator": "Independent Python standard-library byte writer, no PaintCraft serializer",
    "source_attachment_parent": str(PARENT.relative_to(ROOT.parent)),
    "source_attachment_parent_sha256": hashlib.sha256(parent).hexdigest(),
    "pixels": list(RAW), "size": [2, 1], "fixtures": rows,
}, indent=2) + "\n")
