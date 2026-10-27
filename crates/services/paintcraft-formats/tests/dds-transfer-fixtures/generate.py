"""Author deterministic sRGB8-to-linear8 nearest/ties-up table and proof.

W3C CSS Color4 10.2, pinned 2026-10-26:
https://www.w3.org/TR/2026/CRD-css-color-4-20261026/#predefined-sRGB
For sample s<=10 the scaled linear value is 25*s/323. Otherwise it is
255*((200*s+2805)/53805)^(12/5). For every half-integer rounding threshold,
raise positive values to the fifth power and compare arbitrary integers.
This requires no float/pow approximation. A separate Decimal120 calculation
checks every result. No runtime generator or product Python dependency.
"""
import argparse
from decimal import Decimal, localcontext
import hashlib
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    table, exact = [], []
    for sample in range(256):
        if sample <= 10:
            value = (50 * sample + 323) // 646
            unchanged = value * 323 == 25 * sample
        else:
            numerator, denominator = 200 * sample + 2805, 53805
            value = sum(510**5 * numerator**12 >= (2 * threshold - 1)**5 * denominator**12
                        for threshold in range(1, 256))
            unchanged = value**5 * denominator**12 == 255**5 * numerator**12
        table.append(value)
        if unchanged:
            exact.append(sample)
    with localcontext() as context:
        context.prec = 120
        for sample, expected in enumerate(table):
            code = Decimal(sample) / 255
            linear = (code / Decimal("12.92") if code <= Decimal("0.04045") else
                      ((code + Decimal(".055")) / Decimal("1.055"))**Decimal("2.4"))
            assert int(linear * 255 + Decimal(".5")) == expected
    result = {
        "profile": "srgb8-to-linear8-nearest-ties-up-v1",
        "reference": "https://www.w3.org/TR/2026/CRD-css-color-4-20261026/#predefined-sRGB",
        "method": "Exact raised-integer threshold comparisons and independent Decimal120 check",
        "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "exact_continuous_codes": exact,
        "table_sha256": hashlib.sha256(bytes(table)).hexdigest(),
        "table": table,
    }
    with args.output.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, indent=4)
        stream.write("\n")


if __name__ == "__main__":
    main()
