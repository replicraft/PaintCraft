#!/usr/bin/env python3
"""Independent exact Fraction oracle; no PaintCraft/Palette/kernel imports.

Self-contained at any location: output defaults beside this script, --check
verifies existing bytes. Corpus is finite, not an all-byte input-space proof.
"""
import argparse
from fractions import Fraction as F
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ALPHAS = [0, 1, 127, 128, 254, 255]
OPACITIES = [0, 1, 127, 128, 254, 255]


def rounded(q):
    return (2 * q.numerator + q.denominator) // (2 * q.denominator)


def candidate(mode, x, c, t):
    a = F(t, 255)
    b = [F(c), F(x * c, 255), F(x + c) - F(x * c, 255)][mode]
    return (1 - a) * x + a * b


def pixel(mode, rgb, color, t, alpha):
    return [rounded(candidate(mode, x, c, t)) for x, c in zip(rgb, color)] + [alpha]


def pairs():
    values = [
        ([0, 0, 0], [255, 255, 255]),
        ([255, 255, 255], [0, 0, 0]),
        ([20, 80, 200], [220, 40, 0]),
        ([255, 0, 0], [0, 255, 0]),
        ([0, 0, 255], [255, 255, 0]),
        ([0, 255, 255], [255, 0, 255]),
        ([127, 128, 129], [129, 128, 127]),
        ([1, 254, 128], [128, 1, 254]),
        ([64, 16, 128], [200, 100, 100]),
        ([17, 93, 201], [17, 93, 201]),
        ([1, 1, 1], [1, 1, 1]),
        ([254, 254, 254], [254, 254, 254]),
    ]
    state = 0x43504F31
    while len(values) < 64:
        row = []
        for _ in range(6):
            state = (1664525 * state + 1013904223) & 0xFFFFFFFF
            row.append(state >> 24)
        p = (row[:3], row[3:])
        if p not in values:
            values.append(p)
    return values


def build():
    records, rgb_bytes, rgba_bytes = [], bytearray(), bytearray()
    double_round = []
    for pair_id, (rgb, color) in enumerate(pairs()):
        for mode in range(3):
            for t in OPACITIES:
                for alpha in ALPHAS:
                    out = pixel(mode, rgb, color, t, alpha)
                    records.append([pair_id, mode, t, color, rgb + [alpha], out])
                    rgb_bytes.extend(out[:3])
                    rgba_bytes.extend(out)
                if mode:
                    pre = [rounded(candidate(mode, x, c, 255)) for x, c in zip(rgb, color)]
                    wrong = [rounded((1 - F(t, 255)) * x + F(t, 255) * b) for x, b in zip(rgb, pre)]
                    exact = pixel(mode, rgb, color, t, 255)[:3]
                    if wrong != exact:
                        double_round.append([pair_id, mode, t, exact, wrong])
    # Own mask then layer opacity are two separate rounds, never opacity folding.
    stages = []
    for mode in range(3):
        for alpha in ALPHAS:
            for coverage in ALPHAS:
                for layer_opacity in [127, 128]:
                    styled = pixel(mode, [20, 80, 200], [220, 40, 0], 128, alpha)
                    masked = styled[:3] + [rounded(F(alpha * coverage, 255))]
                    final = masked[:3] + [rounded(F(masked[3] * layer_opacity, 255))]
                    stages.append([mode, alpha, coverage, layer_opacity, styled, masked, final])
    assert len(records) == 6912 and len(stages) == 216
    assert len(records) + len(stages) < 8000
    assert pixel(0, [20, 80, 200], [220, 40, 0], 128, 127) == [120, 60, 100, 127]
    assert double_round, "corpus must causally distinguish intermediate rounding"
    return {
        "schema": "paintcraft.color-overlay-independent/v1",
        "modes": ["normal", "multiply", "screen"],
        "rounding": "one exact positive rational final round, ties up; alpha copied",
        "record_fields": ["pair", "mode", "opacity", "color", "input", "expected"],
        "stage_fields": ["mode", "alpha", "coverage", "layerOpacity", "styled", "masked", "layerOpacityApplied"],
        "seed": "LCG32-43504f31; 1664525*x+1013904223; upper byte",
        "records": records,
        "stages": stages,
        "intermediate_rounding_counterexamples": double_round,
        "rgb_sha256": hashlib.sha256(rgb_bytes).hexdigest(),
        "rgba_sha256": hashlib.sha256(rgba_bytes).hexdigest(),
        "semantic_bytes_order": ["red", "green", "blue", "opacity", "effectMode"],
        "scope": "finite independent byte contract; no upstream f32 or product observations",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    raw = (json.dumps(build(), separators=(",", ":")) + "\n").encode()
    path = HERE / "oracle.json"
    if args.check:
        if path.read_bytes() != raw:
            raise SystemExit("oracle drift")
    else:
        path.write_bytes(raw)
    print(hashlib.sha256(raw).hexdigest())


if __name__ == "__main__":
    main()
