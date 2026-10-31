#!/usr/bin/env python3
"""Independent planned PCND6 byte writer; fixtures do not assert a reader exists."""
import sys
sys.dont_write_bytecode = True

import argparse
import copy
import hashlib
import json
from pathlib import Path
import runpy
import struct
import uuid

HERE = Path(__file__).resolve().parent
ROOT = next(p for p in HERE.parents if (p / "crates/services/paintcraft-effects").is_dir())
MAGIC = b"PCND\x06\r\n\x1a"
DOCUMENT = "33333333-3333-4333-8333-333333333333"
STATE = "11111111-1111-4111-8111-111111111111"
LAYER = "22222222-2222-4222-8222-222222222222"
RAW = [64, 16, 128, 0, 16, 64, 255, 255]
LEVELS = "levels-rgb-v1"
ORACLE = "crates/services/paintcraft-effects/tests/levels-fixtures/generate.py"
ORACLE_SHA = "bc53b84508352b7cb3c41eae274f1cc72aa3f16e3bf4d1a99f3f15d3fc3abcaf"
KINDS = ["invert-rgb-v1", "brightness-contrast-v1", "black-white-rgb-v1",
         "threshold-luma-v1", "posterize-rgb-v1", "solarize-rgb-v1", "box-blur-v1", LEVELS]


def u(n):
    assert type(n) is int and n >= 0
    out = bytearray()
    while n >= 128:
        out.append((n & 127) | 128)
        n >>= 7
    return bytes(out + bytes([n]))


def signed(n):
    return u(2 * n if n >= 0 else -2 * n - 1)


def identifier(s):
    raw = s.encode("utf-8")
    return u(len(raw)) + raw


def seal(body):
    return body + hashlib.sha256(body).digest()


def payload(kind, values):
    if kind == LEVELS:
        return struct.pack(">BBHBB", *values)
    if kind == "brightness-contrast-v1":
        return struct.pack(">hh", *values)
    return bytes(values)


def instance(kind=LEVELS, values=(0, 255, 100, 0, 255), enabled=1, mask=None):
    return {"kind": kind, "values": list(values), "enabled": enabled, "mask": mask}


def coverage():
    return {"enabled": 1, "codec": 1, "domain": [-1, 0, 2, 1],
            "bands": [{"y0": 0, "y1": 1, "runs": [
                {"x0": -1, "x1": 1, "coverage": 128},
                {"x0": 1, "x1": 2, "coverage": 255}]}]}


def mask_wire(value):
    if value is None:
        return b"\0"
    left, top, _, _ = value["domain"]
    out = bytes([1, value["enabled"], value["codec"]]) + b"".join(map(signed, value["domain"]))
    out += u(len(value["bands"]))
    for band in value["bands"]:
        out += u(band["y0"] - top) + u(band["y1"] - band["y0"]) + u(len(band["runs"]))
        for run in band["runs"]:
            out += u(run["x0"] - left) + u(run["x1"] - run["x0"]) + bytes([run["coverage"]])
    return out


def mask_digest(value):
    out = b"paintcraft.a8-mask/v1\0" + struct.pack(">qqqqQ", *value["domain"], len(value["bands"]))
    for band in value["bands"]:
        out += struct.pack(">qqQ", band["y0"], band["y1"], len(band["runs"]))
        for run in band["runs"]:
            out += struct.pack(">qqB", run["x0"], run["x1"], run["coverage"])
    return hashlib.sha256(out).digest()


def stack_digest(operations):
    out = b"paintcraft.filter-stack/v3\0" + struct.pack(">H", len(operations))
    for i, op in enumerate(operations):
        name, params = op["kind"].encode("ascii"), payload(op["kind"], op["values"])
        out += (i + 1).to_bytes(16, "big") + bytes([op["enabled"]])
        out += struct.pack(">H", len(name)) + name + struct.pack(">I", len(params)) + params
        out += bytes([op["mask"] is not None])
        if op["mask"] is not None:
            out += bytes([op["mask"]["enabled"]]) + mask_digest(op["mask"])
    return hashlib.sha256(out).hexdigest()


def attachment():
    parent = HERE.parent / "pcnd-metadata-fixtures/rgb-profile.pcnd"
    data = parent.read_bytes()
    assert data[:8] == b"PCND\x02\r\n\x1a" and hashlib.sha256(data[:-32]).digest() == data[-32:]
    offset = 40

    def number():
        nonlocal offset
        value = shift = 0
        while True:
            byte = data[offset]
            offset += 1
            value |= (byte & 127) << shift
            if byte < 128:
                return value
            shift += 7

    number()
    number()
    present = data[offset]
    offset += 1
    if present:
        length = number()
        offset += length
    assert data[offset:offset + 8] == b"PCSMETA1"
    size = int.from_bytes(data[offset + 112:offset + 120], "big")
    result = data[offset:offset + 192 + size]
    return result, {"parent": str(parent.relative_to(ROOT)), "parent_sha256": hashlib.sha256(data).hexdigest(),
                    "bytes": len(result), "sha256": hashlib.sha256(result).hexdigest()}


def file(operations, source=False, declarations=None, prelude=None, prelude_length=None,
         indexes=None, parameter_bytes=None, parameter_lengths=None, mask_bytes=None,
         enabled=None, version=6, extra=b"", nonminimal_length=False):
    declarations = declarations if declarations is not None else sorted({(op["kind"], "params-v1") for op in operations})
    if prelude is None:
        prelude = u(len(declarations)) + b"".join(identifier(k) + identifier(c) for k, c in declarations)
    length = u(len(prelude) if prelude_length is None else prelude_length)
    if nonminimal_length:
        length = length[:-1] + bytes([length[-1] | 128, 0])
    out = MAGIC[:4] + bytes([version]) + MAGIC[5:] + length + prelude
    out += uuid.UUID(DOCUMENT).bytes + uuid.UUID(STATE).bytes + u(2) + u(1) + b"\0" + bytes([source])
    if source:
        out += attachment()[0]
    out += u(1) + uuid.UUID(LAYER).bytes + identifier("x") + b"\x01\xff"
    out += signed(0) + signed(0) + u(2) + u(1) + bytes(4) + u(len(operations))
    for i, op in enumerate(operations):
        index = indexes[i] if indexes is not None else next(j for j, (k, _) in enumerate(declarations) if k == op["kind"])
        params = parameter_bytes[i] if parameter_bytes is not None else payload(op["kind"], op["values"])
        size = parameter_lengths[i] if parameter_lengths is not None else len(params)
        mask = mask_bytes[i] if mask_bytes is not None else mask_wire(op["mask"])
        flag = enabled[i] if enabled is not None else op["enabled"]
        out += (i + 1).to_bytes(16, "big") + bytes([flag]) + u(index) + u(size) + params + mask
    out += b"\x01\0\0" + u(len(RAW)) + bytes(RAW) + b"\0" + extra
    return seal(out)


def nearest(n, d):
    return (2 * n + d) // (2 * d)


def apply_box(pixels, radius):
    if radius == 0:
        return copy.deepcopy(pixels)
    result = []
    for x in range(2):
        taps = [pixels[min(1, max(0, x + dx))] for _ in range(-radius, radius + 1)
                for dx in range(-radius, radius + 1)]
        alpha = sum(p[3] for p in taps)
        result.append([nearest(sum(p[c] * p[3] for p in taps), alpha) if alpha else 0
                       for c in range(3)] + [nearest(alpha, len(taps))])
    return result


def layer_plane(operations, levels_channel):
    pixels = [RAW[:4], RAW[4:]]
    for op in operations:
        if not op["enabled"]:
            continue
        name, values = op["kind"], op["values"]
        incoming = copy.deepcopy(pixels)
        if name == "box-blur-v1":
            pixels = apply_box(pixels, values[0])
        else:
            for p in pixels:
                rgb = p[:3]
                if name == LEVELS:
                    p[:3] = [levels_channel(v, values) for v in rgb]
                elif name == "invert-rgb-v1":
                    p[:3] = [255 - v for v in rgb]
                elif name == "brightness-contrast-v1":
                    b, c = values
                    d = 200 * (100 - c)
                    p[:3] = [nearest(min(255 * d, max(0, (2 * v - 255) * 100 * (100 + c)
                              + 255 * 100 * (100 - c) + 510 * b * (100 - c))), d) for v in rgb]
                elif name == "black-white-rgb-v1":
                    luma = sum(v * weight for v, weight in zip(rgb, [2126, 7152, 722]))
                    p[:3] = [nearest((100 - values[0]) * 10000 * v + values[0] * luma, 1000000) for v in rgb]
                elif name == "threshold-luma-v1":
                    luma = sum(v * weight for v, weight in zip(rgb, [2126, 7152, 722]))
                    p[:3] = [255 if luma >= 10000 * values[0] else 0] * 3
                elif name == "posterize-rgb-v1":
                    p[:3] = [nearest(255 * nearest(v * (values[0] - 1), 255), values[0] - 1) for v in rgb]
                elif name == "solarize-rgb-v1":
                    p[:3] = [255 - v if v >= 128 else v for v in rgb]
        mask = op["mask"]
        if mask and mask["enabled"]:
            assert name != "box-blur-v1", "This small fixture has point masks only"
            for x, p in enumerate(pixels):
                m = next((r["coverage"] for band in mask["bands"] if band["y0"] <= 0 < band["y1"]
                          for r in band["runs"] if r["x0"] <= x < r["x1"]), 0)
                p[:3] = [nearest((255 - m) * a + m * b, 255) for a, b in zip(incoming[x][:3], p[:3])]
    return [v for p in pixels for v in p]


def build():
    oracle_path = ROOT / ORACLE
    assert hashlib.sha256(oracle_path.read_bytes()).hexdigest() == ORACLE_SHA
    channel = runpy.run_path(str(oracle_path), run_name="frozen_levels_numeric")["channel"]
    positives = [
        ("levels-default", [instance()], False),
        ("levels-disabled-default", [instance(enabled=0)], False),
        ("levels-gamma-min", [instance(values=(0, 255, 10, 0, 255))], False),
        ("levels-gamma-max-inverted", [instance(values=(17, 201, 999, 240, 16))], False),
        ("levels-equal-output", [instance(values=(64, 192, 200, 73, 73))], False),
        ("levels-single-step", [instance(values=(127, 128, 999, 0, 255))], False),
        ("box-levels-mask-source", [instance("box-blur-v1", (1,)), instance(values=(0, 255, 200, 0, 255), mask=coverage())], True),
        ("all-eight-profiles", [instance("invert-rgb-v1", ()), instance("brightness-contrast-v1", (10, -20)),
          instance("black-white-rgb-v1", (50,)), instance("threshold-luma-v1", (128,)),
          instance("posterize-rgb-v1", (6,)), instance("solarize-rgb-v1", ()),
          instance("box-blur-v1", (0,)), instance()], False),
    ]
    fixtures = []

    def add(name, data, accepted=False, code="codec.failed", **facts):
        assert hashlib.sha256(data[:-32]).digest() == data[-32:] or name == "bad-outer-sha"
        fixtures.append({"name": name, "accepted_planned": accepted, "code_planned": None if accepted else code,
                         "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "hex": data.hex(), **facts})

    for name, operations, source in positives:
        add(name, file(operations, source), True, operations=operations,
            manifest=[{"profile": k, "codec": "params-v1"} for k in sorted({op["kind"] for op in operations})],
            parameter_payload_hex=[payload(op["kind"], op["values"]).hex() for op in operations],
            mask_digests=[mask_digest(op["mask"]).hex() if op["mask"] else None for op in operations],
            raw_layer_rgba=RAW, evaluated_layer_rgba=layer_plane(operations, channel),
            stack_digest_v3=stack_digest(operations), source_attachment=attachment()[1] if source else None)
    base = [instance()]
    add("unknown-profile", file([instance("future-filter-v1", ())]), code="capability.unsupported")
    add("unknown-disabled-profile", file([instance("future-filter-v1", (), enabled=0)]), code="capability.unsupported")
    add("unknown-codec", file(base, declarations=[(LEVELS, "params-v2")]), code="capability.unsupported")
    add("unknown-disabled-codec", file([instance(enabled=0)], declarations=[(LEVELS, "params-v2")]), code="capability.unsupported")
    add("unused-declaration", file(base, declarations=[("invert-rgb-v1", "params-v1"), (LEVELS, "params-v1")]))
    add("duplicate-declaration", file(base, declarations=[(LEVELS, "params-v1")] * 2))
    mixed = [instance(), instance("invert-rgb-v1", ())]
    add("unsorted-declarations", file(mixed, declarations=[(LEVELS, "params-v1"), ("invert-rgb-v1", "params-v1")]))
    add("out-of-range-index", file(base, indexes=[1]))
    add("zero-declarations", file(base, prelude=u(0), indexes=[0]))
    add("too-many-declarations", file(base, prelude=u(257), indexes=[0]))
    add("nonminimal-prelude-length", file(base, nonminimal_length=True))
    add("nonminimal-profile-count", file(base, prelude=b"\x81\0" + identifier(LEVELS) + identifier("params-v1")))
    add("prelude-tail", file(base, prelude=u(1) + identifier(LEVELS) + identifier("params-v1") + b"\0"))
    add("prelude-over-cap", file(base, prelude_length=65537))
    add("profile-id-over-cap", file(base, declarations=[("a" * 94 + "-v1", "params-v1")], indexes=[0]))
    add("codec-id-over-cap", file(base, declarations=[(LEVELS, "a" * 62 + "-v1")]))
    for name, text in [("uppercase-id", "Levels-rgb-v1"), ("unicode-id", "l\u00e9vels-rgb-v1"),
                       ("zero-version", "levels-rgb-v0"), ("leading-zero-version", "levels-rgb-v01")]:
        add(name, file(base, declarations=[(text, "params-v1")], indexes=[0]))
    for name, values in [("input-equal", (64, 64, 100, 0, 255)), ("input-reversed", (65, 64, 100, 0, 255)),
                         ("gamma-zero", (0, 255, 0, 0, 255)), ("gamma-low", (0, 255, 9, 0, 255)),
                         ("gamma-high", (0, 255, 1000, 0, 255))]:
        add(name, file([instance(values=values)]))
    add("brightness-domain", file(base + [instance("brightness-contrast-v1", (101, 0))]))
    good = payload(LEVELS, base[0]["values"])
    add("parameter-short", file(base, parameter_bytes=[good[:-1]]))
    add("parameter-tail", file(base, parameter_bytes=[good + b"\0"]))
    add("parameter-over-cap", file(base, parameter_lengths=[65537]))
    add("bad-enabled", file(base, enabled=[2]))
    add("bad-mask-presence", file(base, mask_bytes=[b"\x02"]))
    bad_mask = coverage()
    bad_mask["codec"] = 9
    bad_mask["enabled"] = 0
    add("unknown-disabled-mask-codec", file([instance(enabled=0, mask=bad_mask)]), code="capability.unsupported")
    bad_mask = coverage()
    bad_mask["domain"] = [2, 0, 2, 1]
    bad_mask["bands"] = []
    add("empty-mask-domain", file(base, mask_bytes=[mask_wire(bad_mask)]))
    add("legacy-only-noncanonical-v6", file([instance("invert-rgb-v1", ())]))
    add("trailing-body", file(base, extra=b"\0"))
    add("future-v7", file(base, version=7), code="capability.unsupported")
    data = bytearray(file(base))
    data[-1] ^= 1
    add("bad-outer-sha", bytes(data))
    for fixture in fixtures:
        assert fixture["bytes"] < 4096
    assert len({f["name"] for f in fixtures}) == len(fixtures)
    return {"schema": "paintcraft.planned-pcnd6-fixtures/v1", "status": "planned-format-no-reader-claim",
            "document": DOCUMENT, "state": STATE, "layer": LAYER,
            "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "numeric_oracle": {"path": ORACLE, "sha256": ORACLE_SHA},
            "coverage_hash": "existing paintcraft.a8-mask/v1 independent canonical field bytes",
            "fixtures": fixtures}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    output = (json.dumps(build(), indent=2) + "\n").encode()
    target = HERE / "oracle.json"
    if args.write:
        target.write_bytes(output)
    elif not target.exists() or target.read_bytes() != output:
        raise SystemExit("planned fixtures differ; review contract before --write")
    print(f"{len(output)} bytes sha256={hashlib.sha256(output).hexdigest()}")


if __name__ == "__main__":
    main()
