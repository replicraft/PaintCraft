#!/usr/bin/env python3
"""Independent exact rational-power Levels oracle; stdlib only, no product calls."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

PARAMETER_ORDER = ["input_black", "input_white", "gamma_hundredths", "output_black", "output_white"]


def validate(parameters):
    if len(parameters) != 5 or any(type(v) is not int for v in parameters):
        raise ValueError("five strict integers required")
    ib, iw, gamma, ob, ow = parameters
    if not (0 <= ib < iw <= 255 and 10 <= gamma <= 999 and 0 <= ob <= 255 and 0 <= ow <= 255):
        raise ValueError("invalid Levels domain")
    return ib, iw, gamma, ob, ow


def channel(value, parameters):
    """Binary-classify final byte half thresholds with exact integer powers.

    This is intentionally not a monotone sweep over source values and has no
    product fixed-limb arithmetic or float approximation.
    """
    ib, iw, gamma, ob, ow = validate(parameters)
    if type(value) is not int or not 0 <= value <= 255:
        raise ValueError("byte required")
    q, d = min(iw - ib, max(0, value - ib)), iw - ib
    if q == 0 or ob == ow:
        return ob
    if q == d:
        return ow
    divisor = math.gcd(100, gamma)
    numerator_exponent, denominator_exponent = 100 // divisor, gamma // divisor
    q_power, d_power = pow(q, numerator_exponent), pow(d, numerator_exponent)
    slope = ow - ob

    def reaches(k):
        # y >= k-1/2; equality rounds upward even for inverted output.
        if slope > 0:
            a, b = 2 * k - 1 - 2 * ob, 2 * slope
            if a <= 0:
                return True
            if a > b:
                return False
            return q_power * pow(b, denominator_exponent) >= d_power * pow(a, denominator_exponent)
        a, b = 2 * ob - (2 * k - 1), -2 * slope
        if a < 0:
            return False
        if a >= b:
            return True
        return q_power * pow(b, denominator_exponent) <= d_power * pow(a, denominator_exponent)

    low, high = 0, 255
    while low < high:
        middle = (low + high + 1) // 2
        if reaches(middle):
            low = middle
        else:
            high = middle - 1
    return low


def rgba(pixel, parameters, coverage=255):
    if len(pixel) != 4 or any(type(v) is not int or not 0 <= v <= 255 for v in pixel):
        raise ValueError("RGBA byte tuple required")
    if type(coverage) is not int or not 0 <= coverage <= 255:
        raise ValueError("coverage byte required")
    candidate = [channel(v, parameters) for v in pixel[:3]]
    return [((255 - coverage) * p + coverage * q + 127) // 255
            for p, q in zip(pixel[:3], candidate)] + [pixel[3]]


def every_gamma_records():
    samples = [0, 1, 2, 63, 64, 127, 128, 191, 192, 253, 254, 255]
    endpoints = [(0, 255, 0, 255), (0, 255, 255, 0), (64, 192, 16, 240)]
    for gamma in range(10, 1000):
        for ib, iw, ob, ow in endpoints:
            parameters = [ib, iw, gamma, ob, ow]
            for value in samples:
                yield parameters, value, channel(value, parameters)


def full_byte_parameters():
    intervals = [(0, 255), (0, 2), (1, 255), (17, 201), (64, 192), (127, 128)]
    gammas = [10, 11, 25, 50, 99, 100, 101, 125, 200, 333, 999]
    outputs = [(0, 255), (255, 0), (16, 240), (73, 73)]
    for ib, iw in intervals:
        for gamma in gammas:
            for ob, ow in outputs:
                yield [ib, iw, gamma, ob, ow]


def all_byte_records():
    for parameters in full_byte_parameters():
        for value in range(256):
            yield parameters, value, channel(value, parameters)


def input_edge_records():
    # Every legal interval with a nontrivial output range. Gamma100 also has
    # a separately checked direct rational oracle below.
    for ib in range(255):
        for iw in range(ib + 1, 256):
            parameters = [ib, iw, 100, 19, 237]
            values = sorted({max(0, ib - 1), ib, min(255, ib + 1), (ib + iw) // 2,
                             max(0, iw - 1), iw, min(255, iw + 1)})
            for value in values:
                yield parameters, value, channel(value, parameters)


def stream(records):
    digest = hashlib.sha256()
    count = 0
    for parameters, value, result in records:
        # u8 ib, u8 iw, u16le gamma, u8 ob, u8 ow, u8 input, u8 expected.
        digest.update(struct.pack("<BBHBBBB", *parameters, value, result))
        count += 1
    return {"records": count, "bytes": count * 8, "sha256": digest.hexdigest()}


def self_checks():
    for value in range(256):
        assert channel(value, [0, 255, 100, 0, 255]) == value
        assert channel(value, [0, 255, 100, 255, 0]) == 255 - value
    for ib, iw, gamma, ob, ow in full_byte_parameters():
        if gamma != 100:
            continue
        for value in range(256):
            d, q = iw - ib, min(iw - ib, max(0, value - ib))
            n = (d - q) * ob + q * ow
            assert channel(value, [ib, iw, gamma, ob, ow]) == (2 * n + d) // (2 * d)
    assert channel(1, [0, 2, 100, 0, 255]) == 128
    assert channel(1, [0, 2, 100, 255, 0]) == 128
    assert channel(1, [0, 4, 200, 0, 255]) == 128  # sqrt(1/4)=1/2
    assert channel(1, [0, 4, 200, 255, 0]) == 128
    assert channel(1, [0, 2, 50, 0, 255]) == 64  # (1/2)^2=1/4
    assert channel(1, [0, 2, 50, 0, 2]) == 1
    assert channel(1, [0, 2, 50, 2, 0]) == 2


def build():
    self_checks()
    literal_parameters = [
        ("identity", [0, 255, 100, 0, 255]),
        ("invert", [0, 255, 100, 255, 0]),
        ("linear-compressed", [64, 192, 100, 16, 240]),
        ("half-tie", [0, 2, 100, 0, 255]),
        ("inverted-half-tie", [0, 2, 100, 255, 0]),
        ("gamma-square", [0, 255, 50, 0, 255]),
        ("gamma-root", [0, 255, 200, 0, 255]),
        ("gamma-min", [17, 201, 10, 5, 249]),
        ("gamma-max", [17, 201, 999, 249, 5]),
        ("constant", [127, 128, 999, 73, 73]),
    ]
    pixels = [[0, 127, 255, 0], [1, 64, 200, 1], [31, 129, 250, 128], [255, 128, 0, 255]]
    rgba_cases = []
    for name, parameters in literal_parameters:
        for pixel in pixels:
            for coverage in [0, 1, 128, 254, 255]:
                rgba_cases.append({"name": name, "parameters": parameters, "input": pixel,
                                   "coverage": coverage, "expected": rgba(pixel, parameters, coverage)})
    alpha_stream = hashlib.sha256()
    for alpha in range(256):
        pixel = [17, 128, 239, alpha]
        for coverage in [0, 128, 255]:
            for parameters in [[16, 240, 200, 255, 0], [0, 255, 50, 0, 255]]:
                alpha_stream.update(bytes(rgba(pixel, parameters, coverage)))
    invalid = [
        [-1, 255, 100, 0, 255], [255, 255, 100, 0, 255], [0, 0, 100, 0, 255],
        [0, 256, 100, 0, 255], [64, 64, 100, 0, 255], [65, 64, 100, 0, 255],
        [0, 255, 9, 0, 255], [0, 255, 1000, 0, 255], [0, 255, 100, -1, 255],
        [0, 255, 100, 256, 255], [0, 255, 100, 0, -1], [0, 255, 100, 0, 256],
        [False, 255, 100, 0, 255], [0, 255, 100.0, 0, 255],
    ]
    for parameters in invalid:
        try:
            validate(parameters)
        except ValueError:
            continue
        raise AssertionError("invalid specimen accepted")
    return {
        "schema": "paintcraft.levels-numerical-oracle/v1", "status": "foundation-only-not-Descriptor",
        "parameter_order": PARAMETER_ORDER,
        "record_encoding": "u8 ib,u8 iw,u16le gamma,u8 ob,u8 ow,u8 input,u8 expected;8bytes",
        "streams": {"every-gamma": stream(every_gamma_records()),
                    "all-byte-cases": stream(all_byte_records()),
                    "all-input-interval-edges": stream(input_edge_records())},
        "full_byte_cases": {"intervals": 6, "gammas": 11, "output_pairs": 4, "inputs_per_case": 256},
        "luts": [{"name": name, "parameters": parameters,
                  "expected": [channel(v, parameters) for v in range(256)]}
                 for name, parameters in literal_parameters],
        "half_ties": [
            {"parameters": p, "input": 1, "expected": expected}
            for p, expected in [
                ([0, 2, 100, 0, 255], 128), ([0, 2, 100, 255, 0], 128),
                ([0, 4, 200, 0, 255], 128), ([0, 4, 200, 255, 0], 128),
                ([0, 2, 50, 0, 2], 1), ([0, 2, 50, 2, 0], 2),
            ]
        ],
        "rgba_mask_cases": rgba_cases,
        "all_alpha_mask_stream": {"records": 256 * 3 * 2, "bytes": 256 * 3 * 2 * 4,
                                  "sha256": alpha_stream.hexdigest()},
        "invalid_parameters": invalid,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()
    output = Path(__file__).with_name("oracle.json")
    encoded = (json.dumps(build(), indent=2) + "\n").encode()
    if args.write:
        output.write_bytes(encoded)
    elif not output.exists() or output.read_bytes() != encoded:
        raise SystemExit("oracle.json differs; review contract before --write")
    print(f"{len(encoded)} bytes sha256={hashlib.sha256(encoded).hexdigest()}")


if __name__ == "__main__":
    main()
