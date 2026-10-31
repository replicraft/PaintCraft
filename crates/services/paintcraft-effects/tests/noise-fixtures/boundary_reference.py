"""Additional independent exact draw-boundary fixtures; retain the full base corpus."""
from decimal import Decimal
import hashlib
import json
from pathlib import Path
import runpy
import sys
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
reference = runpy.run_path(str(HERE/'reference.py'))
MASK = (1 << 32)-1


def undo_xor_right(n, shift):
    result = n
    for _ in range((32+shift-1)//shift):
        result = n ^ (result >> shift)
    return result


def unmix(n):
    n = undo_xor_right(n, 16)
    n = (n*pow(0x846CA68B, -1, 1 << 32)) & MASK
    n = undo_xor_right(n, 15)
    n = (n*pow(0x7FEB352D, -1, 1 << 32)) & MASK
    return undo_xor_right(n, 16)


def source_x(first):
    # y=seed=channel=0, solve the odd source-x multiplication modulo2^32.
    return ((unmix(first) ^ 0xA511E9B3)*pow(0x9E3779B9, -1, 1 << 32)) & MASK


def build():
    rows = []
    # Last target forces second draw exactly quarter-circle: first=mix^-1(2^30).
    for wanted_first in [0, MASK, unmix(1 << 30)]:
        x = source_x(wanted_first)
        assert reference['draws'](x, 0, 0, 0)[0] == wanted_first
        for amount in [0, 1, 10, 50, 100]:
            for mono in [True, False]:
                for distribution in ['uniform', 'gaussian']:
                    rgb = [64, 16, 128]
                    words = [reference['draws'](x, 0, 0, 0 if mono else c) for c in range(3)]
                    row = {'source_local': [x, 0], 'seed': 0, 'amount_hundredths': amount,
                           'monochrome': mono, 'distribution': distribution, 'rgba': [*rgb, 0],
                           'successive_words': [list(pair) for pair in words]}
                    if distribution == 'uniform':
                        expected = reference['uniform'](rgb, amount, words)
                        row['exact_fraction_rgb'] = expected
                    else:
                        expected, margin = reference['gaussian'](rgb, amount, words, 160, True)
                        lower, _ = reference['gaussian'](rgb, amount, words, 96, True)
                        ideal, _ = reference['gaussian'](rgb, amount, words, 160, False)
                        row.update(exact_binary_tau_math_rgb=expected, ideal_real_tau_rgb=ideal,
                                   precision_agrees=(expected == lower), half_margin=margin)
                    row['reference_rgba'] = [*expected, 0]
                    row['six_mask_rgba'] = [[*reference['coverage'](rgb, expected, a), 0]
                                            for a in reference['COVERAGES']]
                    rows.append(row)
    assert len(rows) == 60
    packed = bytes(v for row in rows for v in row['reference_rgba'])
    masks = bytes(v for row in rows for rgba in row['six_mask_rgba'] for v in rgba)
    return {'status': 'MATHEMATICAL_DRAW_BOUNDARY_REFERENCE_NOT_SOFTWARE_AUTHORITY',
            'rows': rows, 'rgba_sha256': hashlib.sha256(packed).hexdigest(),
            'mask_rgba_sha256': hashlib.sha256(masks).hexdigest(),
            'precision_ambiguities': [i for i,r in enumerate(rows) if not r.get('precision_agrees', True)]}


if __name__ == '__main__':
    encoded = (json.dumps(build(), indent=2)+'\n').encode()
    target = HERE/'boundary-controls.json'
    if '--check' in sys.argv:
        assert target.read_bytes() == encoded
    else:
        assert not target.exists()
        target.write_bytes(encoded)
    print('60 exact draw-boundary rows', hashlib.sha256(encoded).hexdigest())
