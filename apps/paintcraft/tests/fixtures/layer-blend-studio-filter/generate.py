#!/usr/bin/env python3
"""Independent opaque Fraction blend literals; no PaintCraft imports."""
from fractions import Fraction
from pathlib import Path
import argparse
import hashlib
import json


def rounded(x):
    return (2*x.numerator+x.denominator)//(2*x.denominator)


def build():
    back = [31, 127, 241]
    front = [204, 77, 13]
    rows = []
    expected = {
        ("multiply", "initial"): [25, 38, 12],
        ("screen", "initial"): [210, 166, 242],
        ("multiply", "invert-top"): [6, 89, 229],
        ("screen", "invert-top"): [76, 216, 254],
        ("multiply", "invert-bottom"): [179, 39, 1],
        ("screen", "invert-bottom"): [249, 166, 26],
    }
    for mode in ["multiply", "screen"]:
        for stage in ["initial", "invert-top", "invert-bottom"]:
            b = [255-x for x in back] if stage == "invert-bottom" else back
            s = [255-x for x in front] if stage == "invert-top" else front
            rgb = [rounded(Fraction(x*y, 255) if mode == "multiply"
                else x+y-Fraction(x*y, 255)) for x, y in zip(b, s)]
            assert rgb == expected[(mode, stage)]
            rows.append({"mode": mode, "stage": stage, "back": b+[255],
                "front": s+[255], "expected": rgb+[255]})
    return {"schema": "paintcraft.native-studio-filter-blends-independent/v1",
        "rounding": "exact Fraction final nearest ties-up; opaque Normal backdrop",
        "sourceBack": back+[255], "sourceFront": front+[255], "records": rows}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    raw = (json.dumps(build(), indent="\t")+"\n").encode()
    output = Path(__file__).with_name("oracle.json")
    if args.check:
        assert output.read_bytes() == raw, "independent fixture drift"
    else:
        output.write_bytes(raw)
    print(hashlib.sha256(raw).hexdigest())


if __name__ == "__main__":
    main()
