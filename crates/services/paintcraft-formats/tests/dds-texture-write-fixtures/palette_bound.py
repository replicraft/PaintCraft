"""Finite palette SSE lower bound subject to maximum RGB component error 96.

For each assignment of four target pixels, enumerate every RGB565 endpoint
pair independently per channel. Ignoring whole-word endpoint ordering relaxes
constraints, so the result is a LOWER bound, not an attainable block optimum.
Pillow verifies endpoint expansion and four-color integer interpolation first.
This is a developer mathematical diagnostic, not a product encoder/decoder.
"""
import argparse
import hashlib
import io
from itertools import product
import json
import math
from pathlib import Path
import struct
import time

from PIL import Image


def expand(value, bits):
    return (value << (8 - bits)) | (value >> (2 * bits - 8))


def palette_controls(raw):
    header = bytearray(raw[:148])
    for offset, value in [(12, 1), (16, 1), (20, 8), (28, 1), (108, 0x1000)]:
        struct.pack_into("<I", header, offset, value)
    flags = struct.unpack_from("<I", header, 8)[0] & ~0x20000
    struct.pack_into("<I", header, 8, flags)
    controls = []
    for bits, channel, shift in [(5, 0, 11), (6, 1, 5), (5, 2, 0)]:
        for endpoint in range(1 << bits):
            block = struct.pack("<HHI", endpoint << shift, 0, 0)
            actual = Image.open(io.BytesIO(header + block)).convert("RGBA").tobytes()[channel]
            assert actual == expand(endpoint, bits), (bits, endpoint, actual)
        for first, second in [(31, 0), (29, 3), (17, 11), (63 if bits == 6 else 31, 5)]:
            for index in [2, 3]:
                c0, c1 = expand(first, bits), expand(second, bits)
                expected = (2 * c0 + c1) // 3 if index == 2 else (c0 + 2 * c1) // 3
                block = struct.pack("<HHI", first << shift, second << shift, index)
                actual = Image.open(io.BytesIO(header + block)).convert("RGBA").tobytes()[channel]
                assert actual == expected, (bits, first, second, index, actual, expected)
                controls.append([bits, first, second, index, actual])
    return controls


def block_bound(pixels, color_count):
    assignments = list(product(range(color_count), repeat=4))
    channel_minima = []
    for channel, bits in [(0, 5), (1, 6), (2, 5)]:
        values = [pixel[channel] for pixel in pixels]
        best = [(10**10, None) for _ in assignments]
        for first, second in product(range(1 << bits), repeat=2):
            c0, c1 = expand(first, bits), expand(second, bits)
            palette = ([c0, c1, (c0 + c1) // 2] if color_count == 3 else
                       [c0, c1, (2 * c0 + c1) // 3, (c0 + 2 * c1) // 3])
            errors = [[(value - color)**2 for color in palette] for value in values]
            e0, e1, e2, e3 = errors
            for index, (i, j, k, m) in enumerate(assignments):
                selected = (e0[i], e1[j], e2[k], e3[m])
                score = sum(selected)
                if max(selected) <= 96**2 and score < best[index][0]:
                    best[index] = (score, [first, second])
        channel_minima.append(best)
    winner = min(range(len(assignments)),
                 key=lambda index: sum(channel[index][0] for channel in channel_minima))
    return {
        "minimum_sse_lower_bound": sum(channel[winner][0] for channel in channel_minima),
        "indices": assignments[winner],
        "channel_endpoint_candidates": [channel[winner][1] for channel in channel_minima],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="Frozen Normal specimen directory")
    args = parser.parse_args()
    started = time.perf_counter()
    source = args.directory / "17x1-Bc1-Linear-Nearest.rgba"
    if not source.exists():
        source = args.directory.parent / "sources/17x1-opaque.rgba"
    base = source.read_bytes()
    points = [list(base[x * 4:x * 4 + 3]) for x in range(1, 17, 2)]
    raw = (args.directory / "17x1-Bc1-Linear-Nearest.dds").read_bytes()
    controls = palette_controls(raw)
    results = {}
    for count in [3, 4]:
        blocks = [block_bound(points[:4], count), block_bound(points[4:], count)]
        sse = sum(block["minimum_sse_lower_bound"] for block in blocks)
        results[str(count) + "colors"] = {
            "blocks": blocks,
            "sse_lower_bound": sse,
            "rms_lower_bound": math.sqrt(sse / 24),
        }
    # Even a perfect first block cannot make the joint target possible: every
    # second-block pixel also excludes default black under component error 96.
    assert all(max(pixel) > 96 for pixel in points[4:])
    assert min(profile["blocks"][1]["minimum_sse_lower_bound"]
               for profile in results.values()) > 24576
    print(json.dumps({
        "scope": __doc__,
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "source_rgba_sha256": hashlib.sha256(base).hexdigest(),
        "pillow_palette_controls": controls,
        "predeclared_sse_budget": 24576,
        "maximum_component_error_constraint": 96,
        "results": results,
        "elapsed_seconds": time.perf_counter() - started,
    }, indent=4))


if __name__ == "__main__":
    main()
