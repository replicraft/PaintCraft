"""Finite independent exact HSL reference. No product kernel or bound checker."""
import argparse
from fractions import Fraction as F
import hashlib
import json
from pathlib import Path
import runpy
import struct
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
REFERENCE = ROOT/'crates/services/paintcraft-effects/tests/vibrance-fixtures/generate.py'
REFERENCE_SHA = 'c77dd70bbea25ea7eacad1db4bceaedb07478ac9272af0993c29c6046c09f30d'
assert hashlib.sha256(REFERENCE.read_bytes()).hexdigest() == REFERENCE_SHA
helpers = runpy.run_path(str(REFERENCE), run_name='independent_fraction_hsl_reference')
to_hsl, from_hsl = helpers['to_hsl'], helpers['from_hsl']


def valid_parameters(values):
    if type(values) not in (tuple, list) or len(values) != 3:
        raise ValueError('three exact integer H,S,L values required')
    if any(type(v) is not int for v in values):
        raise ValueError('H,S,L must be strict integers')
    h, s, l = values
    if not (-180 <= h <= 180 and -100 <= s <= 100 and -100 <= l <= 100):
        raise ValueError('H/S/L domain')
    return h, s, l


def valid_rgb(rgb):
    if len(rgb) != 3 or any(type(v) is not int or not 0 <= v <= 255 for v in rgb):
        raise ValueError('three exact RGB8 integers required')


def clamp(value):
    return max(F(0), min(F(1), value))


def nearest(value):
    assert value >= 0
    return (2*value.numerator+value.denominator)//(2*value.denominator)


def exact(rgb, parameters):
    valid_rgb(rgb)
    dh, ds, dl = valid_parameters(parameters)
    h, s, l = to_hsl(rgb)
    # Explicit normalized color-space operations; no producer's closed form.
    h = (h + F(dh, 360)) % 1
    s = clamp(s * (1 + F(ds, 100)))
    l = clamp(l + F(dl, 100))
    return [255*clamp(channel) for channel in from_hsl(h, s, l)]


def candidate(rgb, parameters):
    return [nearest(value) for value in exact(rgb, parameters)]


def masked(rgba, transformed, coverage):
    assert type(coverage) is int and 0 <= coverage <= 255
    # Existing point policy: round candidate once, then one encoded RGB mix.
    return [nearest(F(v) + F(coverage, 255)*(new-v))
            for v, new in zip(rgba[:3], transformed)] + [rgba[3]]


NAMED = [
    ('black', [0,0,0]), ('white', [255,255,255]),
    ('red', [255,0,0]), ('green', [0,255,0]), ('blue', [0,0,255]),
    ('yellow-tied-max', [255,255,0]), ('cyan-tied-max', [0,255,255]),
    ('magenta-tied-max', [255,0,255]),
    ('tied-min-red', [200,30,30]), ('tied-min-green', [30,200,30]),
    ('tied-min-blue', [30,30,200]),
    ('near-red-wrap', [255,0,1]), ('near-magenta-wrap', [255,1,0]),
    ('near-gray', [127,128,129]), ('near-gray-permute', [129,127,128]),
    ('gray1', [1,1,1]), ('gray127', [127,127,127]),
    ('gray128', [128,128,128]), ('gray254', [254,254,254]),
    ('half-gray-green', [0,1,0]), ('half-gray-red', [1,0,0]),
    ('half-gray-blue', [0,0,1]), ('half-gray-high', [254,255,254]),
    ('purple', [64,16,128]), ('muted-red', [200,100,100]),
    ('irregular', [20,115,200]), ('almost-black', [0,1,2]),
    ('almost-white', [253,254,255]),
]
PARAMETERS = [(h,0,0) for h in [-180,-60,-2,-1,0,1,2,60,180]] + [
    (0,-100,0), (0,100,0), (0,0,-100), (0,0,100),
    (0,0,-10), (0,0,10), (0,0,-1), (0,0,1),
    (-180,-100,-100), (180,100,100), (-60,100,-25),
    (60,-50,25), (-1,1,-1), (1,-1,1), (137,73,-19),
]
ALPHAS = [0,1,127,128,254,255]
COVERAGES = [0,1,127,128,254,255]


def seeded():
    state = 0x48534C31
    def next_value():
        nonlocal state
        state = (1664525*state+1013904223) & 0xffffffff
        return state
    colors = [[(next_value() >> 16) & 255 for _ in range(3)] for _ in range(40)]
    params = [(next_value()%361-180, next_value()%201-100, next_value()%201-100)
              for _ in range(16)]
    return colors, params


def generate():
    colors, random_params = seeded()
    colors = [rgb for _, rgb in NAMED] + colors
    params = list(dict.fromkeys(PARAMETERS+random_params))
    scalar, ties, digest = [], [], hashlib.sha256()
    for ci, rgb in enumerate(colors):
        assert candidate(rgb, (0,0,0)) == rgb
        assert candidate(rgb, (-180,0,0)) == candidate(rgb, (180,0,0))
        for pi, adjustment in enumerate(params):
            values = exact(rgb, adjustment)
            output = [nearest(v) for v in values]
            digest.update(bytes(output))
            scalar.append(dict(rgb=rgb, parameters=list(adjustment), expected=output))
            for channel, value in enumerate(values):
                if value.denominator == 2 and output[channel] != rgb[channel]:
                    ties.append(dict(color_index=ci, parameter_index=pi, channel=channel,
                                     exact=[value.numerator,value.denominator], expected=output[channel]))
            if adjustment[2] == -100: assert output == [0]*3
            if adjustment[2] == 100: assert output == [255]*3
            if adjustment == (0,-100,0):
                assert output == [nearest(F(max(rgb)+min(rgb),2))]*3
    literals = [
        dict(rgb=[255,0,0], parameters=[2,0,0], expected=[255,9,0]),
        dict(rgb=[255,0,0], parameters=[-2,0,0], expected=[255,0,9]),
        dict(rgb=[0,1,0], parameters=[0,-100,0], expected=[1,1,1]),
        dict(rgb=[0,0,0], parameters=[0,0,10], expected=[26,26,26]),
        dict(rgb=[255,255,255], parameters=[0,0,-10], expected=[230,230,230]),
        dict(rgb=[255,0,0], parameters=[-180,0,0], expected=[0,255,255]),
        dict(rgb=[255,0,0], parameters=[180,0,0], expected=[0,255,255]),
    ]
    for row in literals: assert candidate(row['rgb'], row['parameters']) == row['expected']
    rgba_rows, rgba_hash = [], hashlib.sha256()
    rgba_colors = [rgb for _, rgb in NAMED[2:8]] + [NAMED[i][1] for i in [13,19,23,24]]
    rgba_params = [(0,0,0),(2,0,0),(-180,0,0),(180,0,0),
                   (0,-100,0),(0,100,0),(0,0,-100),(0,0,100),(137,73,-19)]
    for rgb in rgba_colors:
        for adjustment in rgba_params:
            output = candidate(rgb, adjustment)
            for alpha in ALPHAS:
                for coverage in COVERAGES:
                    incoming = rgb+[alpha]
                    result = masked(incoming, output, coverage)
                    assert result[3] == alpha
                    if coverage == 0: assert result == incoming
                    if coverage == 255: assert result == output+[alpha]
                    rgba_hash.update(bytes(result))
                    rgba_rows.append(dict(input_rgba=incoming, parameters=list(adjustment),
                                          coverage=coverage, expected=result))
    invalid = [None, True, [], [0,0], [0,0,0,0], [True,0,0], [0,False,0],
               [0,0,1.0], ['0',0,0], [-181,0,0], [181,0,0],
               [0,-101,0], [0,101,0], [0,0,-101], [0,0,101]]
    for value in invalid:
        try: valid_parameters(value)
        except ValueError: pass
        else: raise AssertionError(('accepted malformed parameters',value))
    assert ties and all(type(v) is int for rgb in colors for v in rgb)
    return dict(schema='paintcraft.proposed-hsl-independent/v1',
                status='SOURCE_ONLY_NO_IMPLEMENTATION_OR_PARITY_CLAIM',
                reference=dict(path=str(REFERENCE.relative_to(ROOT)),sha256=REFERENCE_SHA),
                units=['hue_degrees','saturation_hundredths','lightness_hundredths'],
                named_colors=[dict(name=name,rgb=rgb) for name,rgb in NAMED],
                parameter_order=[list(p) for p in params],
                seed=dict(algorithm='LCG32', initial=0x48534C31, multiplier=1664525,
                          increment=1013904223, random_colors=40, random_parameters=16),
                scalar_record_order='color outer (named then seeded), parameter inner; RGB bytes',
                scalar_rgb_sha256=digest.hexdigest(), scalar_cases=scalar,
                causal_half_ties=ties, literals=literals,
                equal_effect_distinct_intent=dict(parameters=[[-180,0,0],[180,0,0]],
                    semantic_hex=[struct.pack('>hhh',h,0,0).hex() for h in [-180,180]]),
                rgba_record_order='10 named colors,9 parameters,alpha[0,1,127,128,254,255],coverage[0,1,127,128,254,255];RGBA bytes',
                rgba_sha256=rgba_hash.hexdigest(), rgba_controls=rgba_rows,
                invalid_parameters=invalid)


if __name__ == '__main__':
    parser=argparse.ArgumentParser()
    parser.add_argument('--check',action='store_true')
    args=parser.parse_args()
    value=generate()
    text=json.dumps(value,indent=2)+'\n'
    path=HERE/'oracle.json'
    if args.check:
        assert path.read_text()==text,'oracle drift'
    else:
        path.write_text(text)
    print(json.dumps(dict(status='verified'if args.check else'generated',
                          scalars=len(value['scalar_cases']),ties=len(value['causal_half_ties']),
                          rgba=len(value['rgba_controls']),runtime=False)))
