#!/usr/bin/env python3
"""Independent proposed style-stack framing oracle; no product code imports."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import struct
import uuid

DOMAIN = b"paintcraft.layer-style-stack/v1\0"
PROFILE = b"color-overlay-rgb-v1"


def record(identity, color, opacity, blend, enabled=True):
    return dict(id=str(uuid.UUID(int=identity)), color=color, opacity=opacity,
                blend=blend, enabled=enabled)


def frame(entries):
    if len(entries) > 256:
        raise ValueError("count")
    result = bytearray(DOMAIN + struct.pack(">H", len(entries)))
    used = set()
    for entry in entries:
        identity = uuid.UUID(entry["id"])
        if identity.int == 0:
            raise ValueError("nil")
        if identity in used:
            raise ValueError("duplicate")
        used.add(identity)
        if type(entry["enabled"]) is not bool:
            raise ValueError("enabled")
        parameters = entry["color"] + [entry["opacity"], entry["blend"]]
        if len(entry["color"]) != 3 or any(type(v) is not int or not 0 <= v <= 255
                                          for v in parameters):
            raise ValueError("byte")
        if entry["blend"] > 2:
            raise ValueError("mode")
        result += identity.bytes + bytes([int(entry["enabled"])])
        result += struct.pack(">H", len(PROFILE)) + PROFILE
        result += struct.pack(">I", len(parameters)) + bytes(parameters)
    return bytes(result)


def vectors():
    a = record(1, [13, 127, 241], 128, 0)
    b = record(2, [211, 77, 0], 255, 1, False)
    cases = [("empty", []), ("one", [a]), ("ordered", [a, b]),
             ("reordered", [b, a])]
    for field, value in [("id", str(uuid.UUID(int=3))), ("enabled", False),
                         ("color", [14, 127, 241]), ("opacity", 0),
                         ("opacity", 255), ("blend", 1), ("blend", 2)]:
        changed = copy.deepcopy(a)
        changed[field] = value
        cases.append((f"change-{field}-{str(value).lower()}", [changed]))
    for blend in range(3):
        cases.append((f"zero-opacity-mode-{blend}",
                      [record(1, [13, 127, 241], 0, blend)]))
    cases.append(("max256", [record(i + 1, [i, 255 - i, i ^ 127], i, i % 3,
                                       bool(i % 2)) for i in range(256)]))
    negatives = [("nil", [record(0, [1, 2, 3], 0, 0)]),
                 ("duplicate", [a, a]),
                 ("count", [record(i + 1, [1, 2, 3], 0, 0) for i in range(257)]),
                 ("mode", [record(1, [1, 2, 3], 0, 3)]),
                 ("byte", [record(1, [1, 2, 256], 0, 0)])]
    refused = []
    for reason, entries in negatives:
        try:
            frame(entries)
        except ValueError as error:
            assert str(error) == reason
            refused.append(reason)
        else:
            raise AssertionError(reason)
    result = []
    for name, entries in cases:
        encoded = frame(entries)
        result.append(dict(name=name, entries=entries, frame_hex=encoded.hex(),
                           frame_bytes=len(encoded), sha256=hashlib.sha256(encoded).hexdigest()))
    assert len({v["sha256"] for v in result}) == len(result) - 1
    return dict(schema="paintcraft.style-stack-oracle/v1", domain_hex=DOMAIN.hex(),
                cases=result, generator_refusals=refused)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    target = Path(__file__).with_name("vectors.json")
    encoded = json.dumps(vectors(), indent=4) + "\n"
    if args.check:
        assert target.read_text() == encoded, "style-stack vectors differ"
    else:
        target.write_text(encoded)
    print("15 independent style-stack frames; 5 generator refusals")


if __name__ == "__main__":
    main()
