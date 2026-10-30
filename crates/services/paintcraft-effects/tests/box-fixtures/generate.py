"""Independent proposed PaintCraft BoxBlurV1 numeric oracle; Python stdlib only.

This is the proposed fixed-square rational contract, not a port of Palette's
separable u16 implementation. No PaintCraft kernel, imaging package or FFI is used.
"""
import argparse
from collections import Counter
from fractions import Fraction
from functools import lru_cache
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
RADII = [0, 1, 2, 32]
MASK_AMOUNTS = [0, 1, 64, 85, 127, 128, 170, 254, 255]


def nearest(value):
    assert isinstance(value, Fraction) and value >= 0
    return (2 * value.numerator + value.denominator) // (2 * value.denominator)


@lru_cache(maxsize=4096)
def clamped_multiplicities(width, height, radius, x, y):
    """Direct enumeration of the full square, with repeated edge samples counted."""
    locations = Counter()
    for dy in range(-radius, radius + 1):
        for dx in range(-radius, radius + 1):
            locations[(max(0, min(width - 1, x + dx)), max(0, min(height - 1, y + dy)))] += 1
    assert sum(locations.values()) == (2 * radius + 1) ** 2
    return tuple(locations.items())


def box_pixel(rgba, width, height, radius, x, y):
    assert 0 <= radius <= 32 and len(rgba) == width * height * 4
    if radius == 0:
        return rgba[(y * width + x) * 4:(y * width + x) * 4 + 4]
    alpha = 0
    colors = [0, 0, 0]
    for (sx, sy), copies in clamped_multiplicities(width, height, radius, x, y):
        p = rgba[(sy * width + sx) * 4:(sy * width + sx) * 4 + 4]
        alpha += copies * p[3]
        for c in range(3):
            colors[c] += copies * p[c] * p[3]
    channels = [nearest(Fraction(total, alpha)) for total in colors] if alpha else [0, 0, 0]
    return channels + [nearest(Fraction(alpha, (2 * radius + 1) ** 2))]


def region(rgba, size, origin, radius, rect=None):
    width, height = size
    ox, oy = origin
    left, top, rw, rh = rect or [ox, oy, width, height]
    assert ox <= left < left + rw <= ox + width
    assert oy <= top < top + rh <= oy + height
    return [channel for y in range(top - oy, top - oy + rh) for x in range(left - ox, left - ox + rw) for channel in box_pixel(rgba, width, height, radius, x, y)]


def masked(incoming, candidate, coverage):
    assert 0 <= coverage <= 255
    if coverage == 0:
        return list(incoming)
    if coverage == 255:
        return list(candidate)
    weights = [(255 - coverage) * incoming[3], coverage * candidate[3]]
    total = sum(weights)
    if total:
        colors = [nearest(Fraction(weights[0] * incoming[c] + weights[1] * candidate[c], total)) for c in range(3)]
    else:
        colors = [nearest(Fraction((255 - coverage) * incoming[c] + coverage * candidate[c], 255)) for c in range(3)]
    return colors + [nearest(Fraction(total, 255))]


def plane(name, rgba, size, origin, radius, partitions=None, roi=None):
    output = region(rgba, size, origin, radius, roi)
    return {'name': name, 'size': size, 'origin': origin, 'radius': radius, 'rgba': rgba, 'region': roi or [*origin, *size], 'expected': output, 'partitions': [{'region': rect, 'expected': region(rgba, size, origin, radius, rect)} for rect in partitions or []]}


def literals():
    rows = []
    edge = [255, 0, 0, 255, 0, 255, 0, 0]
    for radius, expected in [(0, edge), (1, [255, 0, 0, 170, 255, 0, 0, 85]), (2, [255, 0, 0, 153, 255, 0, 0, 102]), (32, [255, 0, 0, 129, 255, 0, 0, 126])]:
        row = plane(f'red-transparent-edge-r{radius}', edge, [2, 1], [-7, -3], radius)
        assert row['expected'] == expected
        rows.append(row)
    tie = plane('rgb-half-ties-up', [0, 0, 0, 1, 1, 3, 5, 2], [2, 1], [0, 0], 1)
    assert tie['expected'] == [1, 2, 3, 1, 1, 2, 4, 2]
    rows.append(tie)
    low_alpha = [0] * 36
    low_alpha[16:20] = [255, 23, 101, 1]
    row = plane('positive-weight-rounds-alpha-to-zero-retains-color', low_alpha, [3, 3], [-1, -1], 1)
    assert row['expected'] == [255, 23, 101, 0] * 9
    rows.append(row)
    zero = plane('all-hidden-colors-contribute-zero-at-positive-radius', [255, 22, 17, 0, 7, 233, 44, 0], [2, 1], [4, -8], 32)
    assert zero['expected'] == [0] * 8
    rows.append(zero)
    for radius in RADII:
        raw = [channel for i in range(15) for channel in [(i * 41 + 13) % 256, (i * 17 + 5) % 256, (i * 73 + 201) % 256, [0, 1, 2, 127, 128, 254, 255][i % 7]]]
        rows.append(plane(f'nonsquare-negative-origin-r{radius}', raw, [5, 3], [-9, -4], radius, [[-9, -4, 2, 3], [-7, -4, 3, 1], [-7, -3, 3, 2]]))
        raw = [channel for i in range(35) for channel in [(i * 29) % 256, (i * 53 + 9) % 256, (i * 11 + 7) % 256, [255, 1, 0, 128, 254][i % 5]]]
        row = plane(f'interior-roi-true-source-clamp-r{radius}', raw, [7, 5], [-13, -7], radius, [[-11, -6, 1, 3], [-10, -6, 2, 1], [-10, -5, 2, 2]], [-11, -6, 3, 3])
        if radius:
            cropped = region(raw, [7, 5], [-13, -7], 0, [-11, -6, 3, 3])
            assert row['expected'] != region(cropped, [3, 3], [-11, -6], radius), 'Control must distinguish source CLAMP from wrong ROI CLAMP'
        rows.append(row)
    return rows


def mask_literals():
    pairs = [
        ([0, 0, 0, 1], [1, 3, 5, 2]),
        ([200, 10, 30, 0], [10, 50, 250, 0]),
        ([99, 88, 77, 0], [255, 1, 2, 1]),
        ([255, 0, 0, 255], [0, 255, 255, 0]),
        ([0, 255, 0, 127], [255, 0, 255, 128]),
    ]
    assert masked(*pairs[0], 85) == [1, 2, 3, 1]
    assert masked(*pairs[2], 1) == [255, 1, 2, 0]
    assert masked(*pairs[3], 128) == [255, 0, 0, 127]
    return [{'incoming': p, 'candidate': q, 'coverage': m, 'expected': masked(p, q, m)} for p, q in pairs for m in MASK_AMOUNTS]



def masked_planes():
    raw = [255, 0, 0, 255, 0, 255, 0, 0]
    rows = []
    for radius in RADII:
        candidate = region(raw, [2, 1], [-7, -3], radius)
        for amount in [0, 128, 255]:
            output = [c for p in [0, 4] for c in masked(raw[p:p + 4], candidate[p:p + 4], amount)]
            if radius == 1 and amount == 128:
                assert output == [255, 0, 0, 212, 255, 0, 0, 43]
            rows.append({'radius': radius, 'size': [2, 1], 'origin': [-7, -3], 'rgba': raw, 'rounded_candidate': candidate, 'coverage': amount, 'expected': output})
    return rows


def streams():
    result = {}

    def collect(name, records, framing):
        digest = hashlib.sha256()
        count = length = 0
        for record in records:
            payload = bytes(record)
            digest.update(payload)
            count += 1
            length += len(payload)
        result[name] = {'records': count, 'bytes': length, 'framing': framing, 'sha256': digest.hexdigest()}

    collect('one_by_one_all_alphas_all_radii', ([r, alpha, *box_pixel([17, 93, 201, alpha], 1, 1, r, 0, 0)] for r in range(33) for alpha in range(256)), 'radius:u8,alpha:u8,expectedRGBA:4u8; radius outer0..32,alpha inner0..255')
    pairs = [[255, 0, 0, 255, 0, 255, 0, 0], [0, 0, 0, 1, 1, 3, 5, 2], [13, 97, 251, 0, 201, 55, 7, 255]]
    collect('two_pixel_edges_all_radii', ([r, index, x, *box_pixel(raw, 2, 1, r, x, 0)] for r in range(33) for index, raw in enumerate(pairs) for x in range(2)), 'radius:u8,pairIndex:u8,x:u8,expectedRGBA:4u8; radius0..32,three declared pairs in source order,x0..1')
    rgb_pairs = [([0, 0, 0], [1, 3, 5]), ([255, 0, 0], [0, 255, 0]), ([17, 93, 201], [251, 8, 127]), ([255, 255, 255], [0, 0, 0])]
    collect('masked_all_input_alphas_selected_candidate_alphas', ([index, ap, aq, m, *masked(p + [ap], q + [aq], m)] for index, (p, q) in enumerate(rgb_pairs) for ap in range(256) for aq in [0, 1, 127, 128, 255] for m in MASK_AMOUNTS), 'rgbPairIndex:u8,inputAlpha:u8,candidateAlpha:u8,coverage:u8,expectedRGBA:4u8; four RGB pairs in source order, inputAlpha0..255,candidateAlpha[0,1,127,128,255],coverage[0,1,64,85,127,128,170,254,255]')
    return result


def specimen():
    return {'schema': 'paintcraft.proposed-box-blur-oracle/v1', 'status': 'Independent proposed contract; product adoption/runtime/parity not qualified by these fixtures', 'algorithm': 'PaintCraftBoxBlurV1', 'radius': {'min': 0, 'max': 32, 'default': 2}, 'policy': {'input': 'straight encoded RGB8 and alpha; no linear-light/CMS conversion', 'boundary': 'true CLAMP to finite source extent, independent of ROI/partitions/negative origin', 'candidate': 'r0 exact RGBA copy; r>0 one exact square sum: round(Sa/N) alpha and round(Sc/Sa) color, zero color only if Sa=0', 'mask': 'one alpha-aware blend after rounded candidate; exact endpoints; W=0 uses straight hidden-RGB interpolation', 'round': 'nearest rational; nonnegative half ties upward'}, 'literals': literals(), 'mask_literals': mask_literals(), 'masked_planes': masked_planes(), 'streams': streams(), 'source_recipe': {'edge_pairs': [[255, 0, 0, 255, 0, 255, 0, 0], [0, 0, 0, 1, 1, 3, 5, 2], [13, 97, 251, 0, 201, 55, 7, 255]], 'mask_rgb_pairs': [[[0, 0, 0], [1, 3, 5]], [[255, 0, 0], [0, 255, 0]], [[17, 93, 201], [251, 8, 127]], [[255, 255, 255], [0, 0, 0]]]}, 'distinction': 'Not Palette Gaussian/Box parity: no separable intermediate rounding; no upstream kernel imports.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    value = specimen()
    data = (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()
    target = HERE / 'oracle.json'
    if args.write:
        target.write_bytes(data)
    else:
        assert target.read_bytes() == data, 'oracle.json changed; inspect contract before --write'
    print(json.dumps({'status': 'written' if args.write else 'verified', 'streams': len(value['streams']), 'records': sum(stream['records'] for stream in value['streams'].values()), 'plane_literals': len(value['literals']), 'mask_literals': len(value['mask_literals']), 'sha256': hashlib.sha256(data).hexdigest()}, sort_keys=True))


if __name__ == '__main__':
    main()
