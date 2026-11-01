#!/usr/bin/env python3
"""Independent Fraction blend intent oracle; no PaintCraft/Palette kernel imports."""
from fractions import Fraction as F
from pathlib import Path
import hashlib
import json
import sys


def nearest(x):
    return (2 * x.numerator + x.denominator) // (2 * x.denominator)


def reference(back, front, mode, opacity=255, mask=255):
    # Existing two independent layer-alpha byte boundaries are intentional.
    alpha = nearest(F(front[3] * mask, 255))
    alpha = nearest(F(alpha * opacity, 255))
    if alpha == 0:
        return list(back), []  # preserves baseline hidden destination RGB
    af, ab = F(alpha, 255), F(back[3], 255)
    out_a = af + ab * (1 - af)
    result, exact = [], []
    for dest, src in zip(back[:3], front[:3]):
        cb, cs = F(dest, 255), F(src, 255)
        if mode == 'multiply':
            mixed = cb * cs
        elif mode == 'screen':
            mixed = 1 - (1 - cb) * (1 - cs)
        elif mode == 'normal':
            mixed = cs
        else:
            raise ValueError(mode)
        straight = 255 * ((1 - af) * ab * cb + (1 - ab) * af * cs + af * ab * mixed) / out_a
        exact.append([straight.numerator, straight.denominator])
        result.append(nearest(straight))
    return result + [nearest(out_a * 255)], exact


def present(rgba):
    return [nearest(F(c * rgba[3], 255)) for c in rgba[:3]] + [rgba[3]]


def row(name, back, front, opacity=255, mask=255):
    cases = []
    for mode in ['normal', 'multiply', 'screen']:
        out, exact = reference(back, front, mode, opacity, mask)
        cases.append(dict(mode=mode, rgba=out, exact_rgb=exact, display_premultiplied=present(out)))
    return dict(name=name, back=back, front=front, opacity=opacity, own_mask=mask, cases=cases)


def oracle():
    data = [
        row('opaque-distinct', [31, 127, 241, 255], [204, 77, 13, 255]),
        row('black-source', [23, 99, 201, 255], [0, 0, 0, 255]),
        row('white-source', [23, 99, 201, 255], [255, 255, 255, 255]),
        row('black-backdrop', [0, 0, 0, 255], [23, 99, 201, 255]),
        row('white-backdrop', [255, 255, 255, 255], [23, 99, 201, 255]),
        row('both-partial', [10, 240, 80, 128], [200, 30, 180, 128]),
        row('source-opaque-backdrop-partial', [10, 240, 80, 128], [200, 30, 180, 255]),
        row('transparent-backdrop-hidden', [91, 7, 241, 0], [204, 77, 13, 128]),
        row('transparent-source-hidden', [91, 7, 241, 128], [204, 77, 13, 0]),
        row('both-transparent-hidden', [91, 7, 241, 0], [204, 77, 13, 0]),
        row('opacity-zero', [91, 7, 241, 128], [204, 77, 13, 255], 0),
        row('mask-zero', [91, 7, 241, 128], [204, 77, 13, 255], 255, 0),
        row('alpha128-mask128-opacity128', [91, 7, 241, 128], [204, 77, 13, 128], 128, 128),
        row('two-stage-alpha-counterexample', [0, 0, 0, 0], [200, 60, 10, 1], 128, 128),
        row('u32-maximum', [255, 255, 255, 255], [255, 255, 255, 255]),
        row('small-alpha', [254, 1, 128, 1], [1, 254, 127, 2]),
        row('opacity127', [31, 127, 241, 254], [204, 77, 13, 129], 127),
        row('primaries', [255, 0, 0, 255], [0, 255, 255, 255]),
    ]
    # Finite deterministic search only to pin two actual exact half ties.
    ties = []
    for mode in ['multiply', 'screen']:
        found = None
        for a in [2, 64, 127, 128, 254]:
            if found:
                break
            for b in [2, 64, 127, 128, 254]:
                if found:
                    break
                for d in [0, 1, 31, 63, 127, 128, 191, 254]:
                    if found:
                        break
                    for s in [1, 31, 63, 127, 128, 191, 254]:
                        back, front = [d, d, d, b], [s, s, s, a]
                        out, exact = reference(back, front, mode)
                        if exact[0][1] == 2:
                            found = dict(mode=mode, back=back, front=front, exact=exact[0], rgba=out)
                            break
        assert found
        ties.append(found)
    backdrop = [30, 80, 140, 255]
    layers = [dict(rgba=[210, 40, 80, 128], mode='multiply', opacity=129),
              dict(rgba=[40, 190, 230, 192], mode='screen', opacity=173)]
    ordered = []
    for indices in [[0, 1], [1, 0]]:
        out = backdrop
        for index in indices:
            layer = layers[index]
            out, _ = reference(out, layer['rgba'], layer['mode'], layer['opacity'])
        ordered.append(dict(order=indices, rgba=out, display_premultiplied=present(out)))
    assert ordered[0]['rgba'] != ordered[1]['rgba']
    return dict(status='PROPOSED_REFERENCE_NOT_PRODUCT_QUALIFICATION',
                policy='encoded-sRGB byte inputs; existing separate own-mask and opacity alpha rounding; W3C Fraction blend then one channel rounding per layer',
                vectors=data, exact_half_ties=ties,
                order=dict(backdrop=backdrop, layers=layers, outputs=ordered))


def main():
    path = Path(__file__).with_name('oracle.json')
    output = (json.dumps(oracle(), indent=2, sort_keys=True) + '\n').encode()
    if '--check' in sys.argv:
        assert path.read_bytes() == output
    else:
        path.write_bytes(output)
    print(hashlib.sha256(output).hexdigest())


if __name__ == '__main__':
    main()
