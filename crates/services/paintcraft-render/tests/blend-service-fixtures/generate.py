#!/usr/bin/env python3
"""Independent exact mixed-service literals; Python stdlib, no product imports."""
from fractions import Fraction as F
import argparse
import json
from pathlib import Path


def decode100(encoded):
    if encoded <= F(4045, 100000):
        return (encoded / F(1292, 100)) ** 100
    return ((encoded + F(55, 1000)) / F(1055, 1000)) ** 240


def exposure100(value):
    incoming = decode100(F(value, 255)) * (2 ** 100)
    return sum(incoming >= decode100(F(2 * i + 1, 510)) for i in range(255))


def half_up(value):
    return (2 * value.numerator + value.denominator) // (2 * value.denominator)


def build():
    records = []
    for filter_mask in [255, 128]:
        levels = [(x * (255 - filter_mask) + 128 * filter_mask + 127) // 255
                  for x in [204, 77, 13]]
        exposed = [exposure100(x) for x in levels]
        # Knots [255,191,127,63,0] and their centered/secant derivatives are
        # exactly 255-x on every interval, so the independent cubic is linear.
        curved = [255 - x for x in exposed]
        for own_mask in [255, 128]:
            alpha = (128 * own_mask + 127) // 255
            alpha = (alpha * 64 + 127) // 255
            for mode in ['multiply', 'screen']:
                pixels = []
                for source, backdrop in zip(curved, [31, 127, 241]):
                    blend = F(source * backdrop, 255) if mode == 'multiply' else F(255 * (source + backdrop) - source * backdrop, 255)
                    value = F(backdrop * (255 - alpha), 255) + blend * F(alpha, 255)
                    pixels.append(half_up(value))
                records.append({'filter_mask': filter_mask, 'own_mask': own_mask,
                                'mode': mode, 'after_levels': levels,
                                'after_exposure': exposed, 'after_curves': curved,
                                'composed': pixels + [255]})
    assert exposure100(128) == 176
    return {'status': 'independent-rational-expectations-before-Rust-tests',
            'profiles': {'levels': [0, 255, 100, 128, 128], 'exposure': 100,
                         'curves': [255, 191, 127, 63, 0]},
            'backdrop': [31, 127, 241, 255], 'source': [204, 77, 13, 128],
            'opacity': 64, 'records': records}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    path = Path(__file__).with_name('oracle.json')
    encoded = json.dumps(build(), indent=2) + '\n'
    if args.check:
        assert path.read_text() == encoded
    else:
        path.write_text(encoded)


if __name__ == '__main__':
    main()
