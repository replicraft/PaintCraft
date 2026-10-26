"""Owned ABR resources, using only explicit standard-library wire packing."""
import hashlib
import json
import struct
from pathlib import Path
from patterns import fixtures as pattern_fixtures

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
    opaque_size = 10 if minor == 1 else 264
    opaque = bytes((i * 13 + 7) % 256 for i in range(opaque_size)) if depth == 16 and compressed else b"\0" * opaque_size
    body = bytes([len(identifier)]) + identifier + opaque
    body += struct.pack(">4iHB", -2, -1, 0, 2, depth, compressed)
    if compressed and depth == 16:
        assert values == [0, 1, 255, 256, 32769, 65535]
        # Byte packets may cross 16-bit sample boundaries. No-op packets are
        # legal; rows still expand to exactly six big-endian component bytes.
        rows = [b"\x80\xfe\0\2\1\0\xff\x80", b"\5\1\0\x80\1\xff\xff"]
        body += struct.pack(">HH", *(len(row) for row in rows)) + b"".join(rows)
    elif compressed:
        # Two rows: three literal samples, then a replicated three-sample run.
        body += struct.pack(">HH", 4, 2) + b"\2" + bytes(values[:3]) + b"\xfe" + bytes([values[3]])
    elif depth == 8:
        body += bytes(values)
    else:
        body += struct.pack(">6H", *values)
    return struct.pack(">I", len(body)) + body + b"\0" * (-len(body) % 4)


def native_presets():
    def brush(name, changes=(), extra=(), sample_id="raw-8"):
        fields = {name: (name, kind, value) for name, kind, value in [
            unit("Dmtr", "#Pxl", 3), unit("Angl", "#Ang", 0),
            unit("Rndn", "#Prc", 100), unit("Spcn", "#Prc", 100),
            ("Intr", "bool", b"\1"), ("sampledData", "TEXT", text(sample_id))]}
        fields.update({name: (name, kind, value) for name, kind, value in changes})
        return b"Objc" + descriptor("brush", [("Nm  ", "TEXT", text(name)),
            ("Brsh", "Objc", descriptor("sampledBrush", list(fields.values()))),
            unit("Spcn", "#Prc", 100), ("useBrushSize", "bool", b"\1"), *extra])
    dynamics = descriptor("brVr", [("bVTy", "long", struct.pack(">i", 2)),
        ("fStp", "long", struct.pack(">i", 25)), unit("jitter", "#Prc", 0),
        unit("Mnm ", "#Prc", 0)])
    presets = [brush("Native exact"), brush("Q8 spacing", [unit("Spcn", "#Prc", 10)]),
        brush("High depth", sample_id="raw-16"), brush("Missing", sample_id="absent"),
        brush("Size override", [unit("Dmtr", "#Pxl", 6)]),
        brush("Rotation", [unit("Angl", "#Ang", 15)]),
        brush("Roundness", [unit("Rndn", "#Prc", 80)]),
        brush("Flip", [("flipX", "bool", b"\1")]),
        brush("No spacing", [("Intr", "bool", b"\0")]),
        brush("Wet", extra=[("Wtdg", "bool", b"\1")]),
        brush("Noise", extra=[("Nose", "bool", b"\1")]),
        brush("Texture flag without descriptor", extra=[("useTexture", "bool", b"\1")]),
        brush("Future", extra=[("futureOwnedField", "bool", b"\1")]),
        brush("Pressure", extra=[("useTipDynamics", "bool", b"\1"),
            ("szVr", "Objc", dynamics), ("angleDynamics", "Objc", dynamics),
            ("roundnessDynamics", "Objc", dynamics), unit("tiltScale", "#Prc", 0),
            unit("minimumDiameter", "#Prc", 0), unit("minimumRoundness", "#Prc", 0)]),
        brush("Empty native", sample_id="empty-8")]
    desc = struct.pack(">I", 16) + descriptor("null", [("Brsh", "VlLs", struct.pack(">I", len(presets)) + b"".join(presets))])
    samples = sample("raw-8", 8, 0, [0, 1, 127, 128, 254, 255], 1)
    samples += sample("raw-16", 16, 0, [0, 1, 255, 256, 32769, 65535], 1)
    samples += sample("empty-8", 8, 0, [0] * 6, 1)
    return struct.pack(">HH", 6, 1) + section("samp", samples) + section("desc", desc)


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
        "native-presets.abr": native_presets(),
    }
    for major, minor in [(6, 1), (10, 2)]:
        files[f"rle16-v{major}.abr"] = struct.pack(">HH", major, minor) + section("samp", sample("rle-16", 16, 1,
                [0, 1, 255, 256, 32769, 65535], minor)) + section("desc", desc) + section("samp",
                sample("raw-8", 8, 0, [0, 1, 127, 128, 254, 255], minor) + sample("raw-16", 16, 0,
                [0, 1, 255, 256, 32769, 65535], minor)) + section("phry", hierarchy)
    for name, records in pattern_fixtures().items():
        files[f"{name}.abr"] = struct.pack(">HH", 6, 1) + section("samp", sample("raw-8", 8, 0,
                [0, 1, 127, 128, 254, 255], 1)) + section("patt", records) + section("desc", desc) + section("phry", hierarchy)
    manifest = {"generator": "generate.py, Python standard library", "license": "MIT, repository-authored", "files": {}}
    for name, data in files.items():
        (ROOT / name).write_bytes(data)
        manifest["files"][name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    (ROOT / "sources.json").write_text(json.dumps(manifest, indent=4) + "\n")


if __name__ == "__main__":
    main()
