"""Read-only PAINT writer qualification: independent boxes/zlib + pinned C# reader."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import zlib

ROOT = Path(__file__).resolve().parent
PIXELS = bytes([10, 20, 30, 0, 40, 50, 60, 1, 70, 80, 90, 127,
                100, 110, 120, 128, 130, 140, 150, 254, 160, 170, 180, 255])
SECOND = bytes([9, 8, 7, 255, 6, 5, 4, 254, 3, 2, 1, 128,
                200, 180, 160, 127, 140, 120, 100, 1, 80, 60, 40, 0])
FILL = [0x1234, 0xABCD, 0x7F00, 0xFFFF]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def boxes(data, start=0, end=None):
    end = len(data) if end is None else end
    result = []
    while start < end:
        assert end - start >= 8
        size, kind = struct.unpack_from(">I4s", data, start)
        assert size >= 8 and start + size <= end
        result.append((kind.decode("ascii"), start, start + 8, start + size))
        start += size
    assert start == end
    return result


def one(items, kind):
    selected = [item for item in items if item[0] == kind]
    assert len(selected) == 1, kind
    return selected[0]


def payload(data, item):
    return data[item[2]:item[3]]


def full(value, version=0):
    assert value[:4] == bytes([version, 0, 0, 0])
    return value[4:]


def csharp(executable, path):
    result = subprocess.run(["dotnet", str(executable), str(path)], check=True,
                            capture_output=True, text=True, timeout=30)
    return json.loads(result.stdout)


def verify(folder, executable, name, canvas, origin, wide):
    path = folder / f"{name}.paint"
    assert path.stat().st_size <= 64 << 20
    data = path.read_bytes()
    before = sha(data)
    top = boxes(data)
    assert [item[0] for item in top] == ["ftyp", "meta", "free", "mdat"]
    assert payload(data, one(top, "ftyp")) == b"mif1\0\0\0\0mif1gcmiisoamiaf"
    meta = one(top, "meta")
    full(payload(data, meta))
    children = boxes(data, meta[2] + 4, meta[3])
    assert {item[0] for item in children} == {"hdlr", "pitm", "iinf", "iloc", "iprp", "iref", "idat"}
    assert full(payload(data, one(children, "pitm"))) == b"\0\3"
    mdat = one(top, "mdat")
    idat = one(children, "idat")
    iinf = one(children, "iinf")
    assert full(payload(data, iinf))[:2] == b"\0\3"
    info = boxes(data, iinf[2] + 6, iinf[3])
    names = {}
    for item in info:
        assert item[0] == "infe"
        value = full(payload(data, item), 2)
        item_id, protection = struct.unpack_from(">HH", value)
        assert protection == 0 and item_id not in names
        assert value[4:8] == (b"iovl" if item_id == 3 else b"unci")
        assert value[-1] == 0 and b"\0" not in value[8:-1]
        names[item_id] = value[8:-1].decode("utf-8")
    assert names == {1: "literal 日本語🙂", 2: "second literal", 3: ""}
    locations = {}
    value = full(payload(data, one(children, "iloc")), 1)
    assert value[:4] == b"\x44\0\0\3" and len(value) == 4 + 16 * 3
    for cursor in range(4, len(value), 16):
        item_id, method, reference, count, offset, length = struct.unpack_from(">HHHHII", value, cursor)
        assert item_id not in locations and count == 1 and reference == 0 and length > 0
        assert method == (1 if item_id == 3 else 0)
        start = idat[2] + offset if method == 1 else offset
        scope = idat if method == 1 else mdat
        assert scope[2] <= start < start + length <= scope[3]
        locations[item_id] = (start, start + length)
    ranges = sorted(locations.values())
    assert all(a[1] <= b[0] for a, b in zip(ranges, ranges[1:]))
    assert locations[1][0] == mdat[2] and locations[1][1] == locations[2][0] and locations[2][1] == mdat[3]
    iprp = one(children, "iprp")
    property_boxes = boxes(data, iprp[2], iprp[3])
    ipco = one(property_boxes, "ipco")
    properties = boxes(data, ipco[2], ipco[3])
    assert len(properties) == 8
    associations = {}
    value = full(payload(data, one(property_boxes, "ipma")))
    assert struct.unpack_from(">I", value)[0] == 3
    cursor = 4
    for _ in range(3):
        item_id, count = struct.unpack_from(">HB", value, cursor)
        cursor += 3
        indexes = value[cursor:cursor + count]
        cursor += count
        assert len(indexes) == count and all(index & 128 and 0 < (index & 127) <= len(properties) for index in indexes)
        selected = [properties[(index & 127) - 1] for index in indexes]
        assert len({item[0] for item in selected}) == count
        associations[item_id] = {item[0]: payload(data, item) for item in selected}
    assert cursor == len(value) and set(associations) == {1, 2, 3}
    iref = one(children, "iref")
    full(payload(data, iref))
    references = boxes(data, iref[2] + 4, iref[3])
    assert [item[0] for item in references] == ["dimg"]  # No prem reference.
    assert payload(data, references[0]) == struct.pack(">HHHH", 3, 2, 1, 2)
    overlay = data[slice(*locations[3])]
    assert overlay[:2] == bytes([0, int(wide)])
    assert list(struct.unpack_from(">4H", overlay, 2)) == FILL
    pair_type = "II" if wide else "HH"
    assert struct.unpack_from(">" + pair_type, overlay, 10) == canvas
    cursor = 18 if wide else 14
    pair_type = "ii" if wide else "hh"
    origins = [origin, (-3, 4)]
    for expected in origins:
        assert struct.unpack_from(">" + pair_type, overlay, cursor) == expected
        cursor += 8 if wide else 4
    assert cursor == len(overlay)
    assert set(associations[3]) == {"ispe", "pixi"}
    assert struct.unpack(">II", full(associations[3]["ispe"])) == canvas
    assert full(associations[3]["pixi"]) == b"\x04\x08\x08\x08\x08"
    layers = []
    for item_id, geometry, expected_rgba, expected_origin in [(1, (3, 2), PIXELS, origin), (2, (2, 3), SECOND, (-3, 4))]:
        props = associations[item_id]
        assert set(props) == {"cmpC", "ispe", "colr", "cmpd", "uncC", "pixi"}
        assert full(props["cmpC"]) == b"defl\0"
        assert struct.unpack(">II", full(props["ispe"])) == geometry
        assert props["colr"] == b"nclx\0\1\0\x0d\0\0\x80"
        assert full(props["pixi"]) == b"\x04\x08\x08\x08\x08"
        assert struct.unpack_from(">I", props["cmpd"])[0] == 4
        components = struct.unpack_from(">4H", props["cmpd"], 4)
        assert set(components) == {4, 5, 6, 7}
        config = full(props["uncC"])
        assert config[:8] == b"gene\0\0\0\4"
        inverse = [None] * 4
        for position in range(4):
            index, depth, kind, alignment = struct.unpack_from(">HBBB", config, 8 + position * 5)
            assert index < 4 and (depth, kind, alignment) == (7, 0, 0)
            slot = components[index] - 4
            assert inverse[slot] is None
            inverse[slot] = position
        assert config[28:] == b"\0\1\0\0" + bytes(20)
        compressed = data[slice(*locations[item_id])]
        inflater = zlib.decompressobj(-15)
        raw = inflater.decompress(compressed, geometry[0] * geometry[1] * 4 + 1)
        assert inflater.eof and not inflater.unused_data and not inflater.unconsumed_tail
        assert len(raw) == len(expected_rgba)
        rgba = bytes(raw[pixel + source] for pixel in range(0, len(raw), 4) for source in inverse)
        assert rgba == expected_rgba
        layers.append({"name": names[item_id], "width": geometry[0], "height": geometry[1],
                       "origin": list(expected_origin), "rgba_sha256": sha(rgba)})
    reference = csharp(executable, path)
    assert (reference["width"], reference["height"]) == canvas
    assert reference["canvas_fill16"] == FILL and reference["overlay_flags"] == int(wide)
    assert reference["canvas_fill8"] == [value >> 8 for value in FILL]
    # Preserve the reference reader's ASCII-name bug; never mutate wire/source to match it.
    for actual, expected in zip(reference["layers"], layers):
        assert actual == {**expected, "name": expected["name"].encode("utf-8").decode("latin1")}
    assert sha(path.read_bytes()) == before
    return {"filename": path.name, "sha256": before, "canvas": list(canvas), "fill16": FILL,
            "wide_overlay": wide, "layers": layers, "csharp": reference,
            "reference_limitations": ["high-level fill loses low eight bits", "UTF-8 name bytes decoded as ASCII characters"]}


def main():
    assert len(sys.argv) == 3, "usage: verify_writer.py <vectors-dir> <private-reference.dll>"
    folder, executable = map(Path, sys.argv[1:])
    baseline_path = ROOT / "mspaintlib-sample.paint"
    baseline_sha = sha(baseline_path.read_bytes())
    expected = json.loads((ROOT / "sample-oracle.json").read_text())
    assert baseline_sha == expected["source_sha256"]
    baseline = csharp(executable, baseline_path)
    assert sha(baseline_path.read_bytes()) == baseline_sha
    assert (baseline["width"], baseline["height"]) == (expected["width"], expected["height"])
    assert baseline["canvas_fill16"] == expected["canvas_fill"]
    assert [layer["rgba_sha256"] for layer in baseline["layers"]] == [layer["raw_rgba_sha256"] for layer in expected["layers"]]
    results = [verify(folder, executable, "narrow", (9, 7), (1, -2), False),
               verify(folder, executable, "wide", (65536, 1), (60000, -70000), True),
               verify(folder, executable, "extreme", (9, 7), (-2147483648, 2147483647), True)]
    print(json.dumps({"python": sys.version.split()[0], "zlib": zlib.ZLIB_RUNTIME_VERSION,
                      "upstream_commit": "e35312bec8e8a09c6a209d2dc0110b885e48c945",
                      "upstream_archive_sha256": "6661e83b496d8f7606bf4b96a7e7a51e405a09d82924ba2e2270518b6b7dc503",
                      "reference_target": "unchanged net8 source compiled for installed net10 runtime; not Windows Paint",
                      "baseline_source_sha256": baseline_sha, "baseline": baseline, "results": results}, indent=4, ensure_ascii=False))


if __name__ == "__main__":
    main()
