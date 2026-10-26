"""Independent 100-digit Decimal policy oracle; NumPy checks the color matrix.

Run from any directory. This is qualification tooling, not a runtime decoder.
RGB primary/white constants: Radiance color.h; Rec.709/sRGB: ASWF/IEC.
Bradford matrix: ICC chromatic adaptation; no claim of an authored viewing look.
"""
from decimal import Decimal as D, getcontext
import json
import math
from pathlib import Path
import struct
import numpy as np

getcontext().prec = 100


def multiply(a, b):
    return [[sum(x * y for x, y in zip(row, col)) for col in zip(*b)] for row in a]


def inverse(a):
    n = len(a)
    a = [row[:] + [D(i == j) for j in range(n)] for i, row in enumerate(a)]
    for i in range(n):
        pivot = a[i][i]
        a[i] = [value / pivot for value in a[i]]
        for j in range(n):
            if i != j:
                scale = a[j][i]
                a[j] = [x - scale * y for x, y in zip(a[j], a[i])]
    return [row[n:] for row in a]


def rgb_matrix(green_x, white):
    xy = [(D(".64"), D(".33")), (green_x, D(".60")), (D(".15"), D(".06"))]
    primaries = [[x / y for x, y in xy], [D(1)] * 3, [(1 - x - y) / y for x, y in xy]]
    white_xyz = [[white[0] / white[1]], [D(1)], [(1 - white[0] - white[1]) / white[1]]]
    scale = multiply(inverse(primaries), white_xyz)
    return [[primaries[i][j] * scale[j][0] for j in range(3)] for i in range(3)], white_xyz


source, source_white = rgb_matrix(D(".29"), (D(1) / 3, D(1) / 3))
target, target_white = rgb_matrix(D(".30"), (D(".3127"), D(".3290")))
bradford = [[D(v) for v in row] for row in [[".8951", ".2664", "-.1614"], ["-.7502", "1.7135", ".0367"], [".0389", "-.0685", "1.0296"]]]
cone_source = multiply(bradford, source_white)
cone_target = multiply(bradford, target_white)
scale = [[cone_target[i][0] / cone_source[i][0] if i == j else D(0) for j in range(3)] for i in range(3)]
matrix = multiply(multiply(multiply(multiply(inverse(target), inverse(bradford)), scale), bradford), source)
# A second numerical implementation checks inversion/multiplication independently.
as_np = lambda value: np.array([[float(v) for v in row] for row in value])
other = np.linalg.solve(as_np(target), np.linalg.solve(as_np(bradford), as_np(scale) @ as_np(bradford) @ as_np(source)))
assert np.max(np.abs(other - as_np(matrix))) < 2e-14


def f32_bits(value):
    return struct.pack(">f", value).hex()


def value(bits):
    return struct.unpack(">f", bytes.fromhex(bits))[0]


def encode(v):
    encoded = D("12.92") * v if v <= D(".0031308") else D("1.055") * v ** (D(5) / 12) - D(".055")
    return int((encoded * 255 + D(".5")).to_integral_value(rounding="ROUND_FLOOR"))


def background(values):
    out = []
    for channel in values:
        v = D(channel) / 255
        out.append(v / D("12.92") if v <= D(".04045") else ((v + D(".055")) / D("1.055")) ** D("2.4"))
    return out


def reference(case):
    space = case.get("assignment") or case["space"]
    if space == "unspecified":
        return {"error": "CapabilityUnsupported"}
    samples = [value(bits) for bits in case["sample_bits"]]
    if not all(math.isfinite(v) for v in samples):
        return {"error": "CapabilityUnsupported"}
    rgb = [D.from_float(v) for v in samples[:3]]
    alpha = D(1) if case["alpha"] == "opaque" else D.from_float(samples[3])
    if not D(0) <= alpha <= D(1):
        return {"error": "CapabilityUnsupported"}
    if case["negative"] == "reject" and any(v < 0 for v in rgb):
        return {"error": "CapabilityUnsupported"}
    effective = case.get("alpha_assignment") or case["alpha"]
    linear = rgb if space == "rec709" else [row[0] for row in multiply(matrix, [[v] for v in rgb])]
    if case.get("background") is None:
        if effective == "premultiplied":
            if alpha == 0:
                if any(v != 0 for v in rgb):
                    return {"error": "CapabilityUnsupported"}
                linear = [D(0)] * 3
            else:
                linear = [v / alpha for v in linear]
        output_alpha = alpha
    else:
        backdrop = background(case["background"])
        linear = [(v * alpha if effective == "straight" else v) + bg * (1 - alpha) for v, bg in zip(linear, backdrop)]
        output_alpha = D(1)
    exposure = D(2) ** (D(case["exposure_milli_ev"]) / 1000)
    result = []
    for v in linear:
        v *= exposure
        if v < 0 and case["negative"] == "reject":
            return {"error": "CapabilityUnsupported"}
        v = max(D(0), v)
        v = min(D(1), v) if case["curve"] == "clip" else v / (1 + v)
        result.append(encode(v))
    result.append(int((output_alpha * 255 + D(".5")).to_integral_value(rounding="ROUND_FLOOR")))
    return {"rgba8": result}


cases = []


def add(name, values, **overrides):
    case = {"id": name, "space": "rec709", "alpha": "opaque", "curve": "clip", "negative": "reject", "exposure_milli_ev": 0, "sample_bits": [f32_bits(v) for v in values], **overrides}
    case.update(reference(case))
    cases.append(case)


add("clip-white-is-white", [1, 1, 1])
add("explicit-reinhard-white-is-188", [1, 1, 1], curve="reinhard")
add("srgb-transfer-boundary", [0, .0031308, .18])
add("clip-highlights", [.18, 2, 4])
add("reinhard-highlights", [.18, 2, 4], curve="reinhard")
add("radiance-neutral-adaptation", [.18, .18, .18], space="radiance")
for i, name in enumerate(["red", "green", "blue"]):
    add("radiance-" + name, [int(c == i) for c in range(3)], space="radiance", negative="clip")
add("radiance-user-overrides-source", [0, 1, 0], space="radiance", assignment="rec709")
add("unlabelled-refuses-guess", [.18, .18, .18], space="unspecified")
add("unlabelled-explicit-user-assignment", [.18, .18, .18], space="unspecified", assignment="rec709")
add("premultiplied-unassociation", [.25, .125, 0, .5], alpha="premultiplied")
add("zero-alpha-additive-refuses-preserve", [.25, 0, 0, 0], alpha="premultiplied")
add("zero-alpha-additive-black", [.25, 0, 0, 0], alpha="premultiplied", background=[0, 0, 0])
add("zero-alpha-additive-white", [.25, 0, 0, 0], alpha="premultiplied", background=[255, 255, 255])
add("premultiplied-transparent-black", [0, 0, 0, 0], alpha="premultiplied")
add("straight-hidden-rgb", [.25, .5, 1, 0], alpha="straight")
add("user-declares-straight", [.25, .125, 0, .5], alpha="premultiplied", alpha_assignment="straight")
add("user-declares-premultiplied", [.25, .125, 0, .5], alpha="straight", alpha_assignment="premultiplied")
add("negative-source-refused", [-.1, .25, 0])
add("negative-source-explicit-clip", [-.1, .25, 0], negative="clip")
add("exposure-minus-16", [1, .5, .25], exposure_milli_ev=-16000)
add("exposure-plus-16", [1, .5, .25], exposure_milli_ev=16000)
add("fractional-exposure", [.1, .18, .25], exposure_milli_ev=500)
add("float-range-does-not-overflow-reinhard", [3e38, 1e30, 1], curve="reinhard", exposure_milli_ev=16000)
add("sub-byte-alpha-is-explicitly-quantized", [1e30, .25, 0, 1e-40], alpha="premultiplied", curve="reinhard")
for name, v in [("nan", math.nan), ("positive-infinity", math.inf), ("negative-infinity", -math.inf)]:
    add("nonfinite-rgb-" + name, [v, 0, 0])
    add("nonfinite-alpha-" + name, [0, 0, 0, v], alpha="premultiplied")
for name, v in [("negative", -.01), ("above-one", 1.01)]:
    add("out-of-range-alpha-" + name, [.1, 0, 0, v], alpha="straight")

document = {"oracle": "Python Decimal100, separate NumPy matrix solve", "numpy_version": np.__version__, "radiance_to_rec709_d65": [[str(v) for v in row] for row in matrix], "cases": cases}
Path(__file__).with_name("vectors.json").write_text(json.dumps(document, indent=4, allow_nan=False) + "\n")
print(f"Wrote {len(cases)} independent precision vectors")
