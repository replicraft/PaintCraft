"""Owned CSH specimens and expected geometry, independent of Rust codecs."""
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).parent
OPERATIONS = [None, "exclude", "combine", "subtract", "intersect"]


def record(selector, payload=b""):
    assert len(payload) <= 24
    return struct.pack(">H", selector) + payload.ljust(24, b"\0")


def generate():
    data = bytearray(b"cush" + struct.pack(">II", 2, 5))
    expected = []
    for index, operation in enumerate(OPERATIONS):
        name = "形 / Shape 🚀" if index < 2 else f"Shape {index}"
        encoded = (name + "\0").encode("utf-16be")
        data.extend(struct.pack(">I", len(encoded) // 2) + encoded)
        while len(data) % 4:
            data.append(0)
        identifier = b"duplicate-id" if index < 2 else f"shape-{index}".encode()
        width, height = 200 + index, 100 + index
        top, left = 10 + index, 20 + index
        body = bytearray(bytes([len(identifier)]) + identifier)
        body.extend(struct.pack(">IIII", top, left, top + height, left + width))
        body.extend(record(6))
        body.extend(record(8, struct.pack(">H", index % 2)))
        clip = [0, 1 << 22, 1 << 24, 1 << 24, 72 << 24]
        body.extend(record(7, struct.pack(">5i", *clip)))
        paths = []
        for opened in [False, True]:
            flag = index % 3
            body.extend(record(3 if opened else 0, struct.pack(">HhH", 2, index - 1, flag)))
            knots = []
            for linked in [True, False]:
                # Wire order is y0,x0,y1,x1,y2,x2; negative/fractional values
                # prove that the adapter does not round to integer pixels.
                wire = [-1, 1 << 24, 1 << 23, 1, -(1 << 24), 0]
                body.extend(record((4 if opened else 1) + (not linked), struct.pack(">6i", *wire)))
                points = [wire[i] / (1 << 24) * (width if n % 2 == 0 else height)
                          for n, i in enumerate([1, 0, 3, 2, 5, 4])]
                knots.append({"linked": linked, "points": points})
            path = {"open": opened, "knots": knots,
                    "fillRule": "non-zero" if flag == 2 else "even-odd"}
            if operation is not None:
                path["operation"] = operation
            paths.append(path)
        data.extend(struct.pack(">II", 1, len(body)) + body)
        expected.append({"name": name, "id": identifier.decode(), "width": width,
                         "height": height, "paths": paths,
                         "fillStartsWithAllPixels": bool(index % 2),
                         "clipboard": dict(zip(["top", "left", "bottom", "right", "resolution"],
                                               [value / (1 << 24) for value in clip]))})
    return bytes(data), {"shapes": expected}


if __name__ == "__main__":
    mixed, expected = generate()
    files = {"paths.csh": mixed, "empty.csh": b"cush" + struct.pack(">II", 2, 0)}
    manifest = {"generator": "generate.py, Python standard library",
                "license": "MIT, repository-authored fixtures", "files": {}}
    for name, data in files.items():
        (ROOT / name).write_bytes(data)
        manifest["files"][name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    (ROOT / "expected.json").write_text(json.dumps(expected, ensure_ascii=False, indent=4) + "\n")
    (ROOT / "sources.json").write_text(json.dumps(manifest, indent=4) + "\n")
