"""Owned ABR resources, using only explicit standard-library wire packing."""
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).parent


def text(value):
    data = (value + "\0").encode("utf-16be")
    return struct.pack(">I", len(data) // 2) + data


def key(value):
    data = value.encode("ascii")
    return struct.pack(">I", 0 if len(data) == 4 else len(data)) + data


def descriptor(class_id, fields):
    result = text("") + key(class_id) + struct.pack(">I", len(fields))
    for name, kind, payload in fields:
        result += key(name) + kind.encode("ascii") + payload
    return result


def unit(name, code, value):
    return name, "UntF", code.encode("ascii") + struct.pack(">d", value)


def preset(class_id, name, extra=()):
    shape = descriptor(class_id, [unit("Dmtr", "#Pxl", 30), unit("Angl", "#Ang", 15),
                                 unit("Rndn", "#Prc", 80), unit("Spcn", "#Prc", 25),
                                 ("Intr", "bool", b"\1"), *extra])
    return "Objc".encode() + descriptor("brush", [("Nm  ", "TEXT", text(name)),
                   ("Brsh", "Objc", shape), unit("Spcn", "#Prc", 100),
                   ("useBrushSize", "bool", b"\1"),
                   ("futureOwnedField", "tdta", struct.pack(">I", 3) + b"abc")])


def section(kind, payload):
    return b"8BIM" + kind.encode("ascii") + struct.pack(">I", len(payload)) + payload + b"\0" * (-len(payload) % 4)


def sample(identifier, depth, compressed, values, minor):
    identifier = identifier.encode("ascii")
    body = bytes([len(identifier)]) + identifier + b"\0" * (10 if minor == 1 else 264)
    body += struct.pack(">4iHB", -2, -1, 0, 2, depth, compressed)
    if compressed:
        # Two rows: three literal samples, then a replicated three-sample run.
        body += struct.pack(">HH", 4, 2) + b"\2" + bytes(values[:3]) + b"\xfe" + bytes([values[3]])
    elif depth == 8:
        body += bytes(values)
    else:
        body += struct.pack(">6H", *values)
    return struct.pack(">I", len(body)) + body + b"\0" * (-len(body) % 4)


def main():
    brushes = [preset("computedBrush", "Owned computed 🚀", [unit("Hrdn", "#Prc", 60)]),
               preset("sampledBrush", "Owned sampled", [("sampledData", "TEXT", text("raw-8")),
                       ("Nm  ", "TEXT", text("Mask name"))])]
    desc = struct.pack(">I", 16) + descriptor("null", [("Brsh", "VlLs", struct.pack(">I", len(brushes)) + b"".join(brushes))])
    hierarchy = struct.pack(">I", 16) + descriptor("null", [("ownedMetadata", "TEXT", text("Kept verbatim"))])
    files = {
        "samples-v6.abr": struct.pack(">HH", 6, 1) + section("samp", sample("raw-8", 8, 0, [0, 1, 127, 128, 254, 255], 1)
                + sample("rle-8", 8, 1, [0, 1, 255, 127, 127, 127], 1)
                + sample("raw-16", 16, 0, [0, 1, 255, 256, 32769, 65535], 1)) + section("desc", desc) + section("phry", hierarchy),
        "samples-v10.abr": struct.pack(">HH", 10, 2) + section("samp", sample("raw-8", 8, 0, [0, 1, 127, 128, 254, 255], 2)),
        "empty.abr": struct.pack(">HH", 6, 1),
    }
    manifest = {"generator": "generate.py, Python standard library", "license": "MIT, repository-authored", "files": {}}
    for name, data in files.items():
        (ROOT / name).write_bytes(data)
        manifest["files"][name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    (ROOT / "sources.json").write_text(json.dumps(manifest, indent=4) + "\n")


if __name__ == "__main__":
    main()
