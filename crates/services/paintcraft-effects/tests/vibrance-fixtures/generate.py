#!/usr/bin/env python3
"""Independent rational RGB→HSL→RGB reference; no PaintCraft imports."""
import argparse
from fractions import Fraction as F
import hashlib
import itertools
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
AMOUNTS = [-100, -99, -75, -50, -1, 0, 1, 25, 50, 99, 100]


def clamp(x, lo=F(0), hi=F(1)):
    return max(lo, min(hi, x))


def nearest(x):
    return (2 * x.numerator + x.denominator) // (2 * x.denominator)


def to_hsl(rgb):
    r, g, b = [F(v, 255) for v in rgb]
    high, low = max(r, g, b), min(r, g, b)
    light = (high + low) / 2
    if high == low:
        return F(0), F(0), light
    delta = high - low
    saturation = delta / (high + low if light <= F(1, 2) else 2 - high - low)
    if high == r:
        hue = ((g - b) / delta) % 6
    elif high == g:
        hue = (b - r) / delta + 2
    else:
        hue = (r - g) / delta + 4
    return hue / 6, saturation, light


def hue_channel(p, q, t):
    t %= 1
    if t < F(1, 6):
        return p + (q - p) * 6 * t
    if t < F(1, 2):
        return q
    if t < F(2, 3):
        return p + (q - p) * (F(2, 3) - t) * 6
    return p


def from_hsl(h, s, l):
    if s == 0:
        return [l] * 3
    q = l * (1 + s) if l < F(1, 2) else l + s - l * s
    p = 2 * l - q
    return [hue_channel(p, q, h + F(1, 3)), hue_channel(p, q, h), hue_channel(p, q, h - F(1, 3))]


def exact(rgb, amount):
    h, s, l = to_hsl(rgb)
    adjusted = clamp(s * (1 + F(amount, 100) * (1 - s)))
    return [255 * clamp(v) for v in from_hsl(h, adjusted, l)]


def candidate(rgb, amount):
    return [nearest(v) for v in exact(rgb, amount)]


def mix(rgb, transformed, coverage):
    # Existing stage boundary: round candidate once, THEN separate A8 interpolation.
    return [nearest(F(v) + F(coverage, 255) * (new - v)) for v, new in zip(rgb, transformed)]


def generate():
    named = {
        'black': [0, 0, 0], 'white': [255, 255, 255],
        'red': [255, 0, 0], 'green': [0, 255, 0], 'blue': [0, 0, 255],
        'cyan': [0, 255, 255], 'magenta': [255, 0, 255], 'yellow': [255, 255, 0],
        'gray127': [127] * 3, 'gray128': [128] * 3,
        'literal-color': [20, 115, 200], 'dark-color': [64, 16, 128],
        'near-gray': [127, 128, 129], 'near-black': [0, 1, 2], 'near-white': [253, 254, 255],
    }
    for index, color in enumerate(itertools.permutations([1, 127, 128])):
        named[f'permutation-{index}'] = list(color)
    for index, color in enumerate([[64,64,128],[64,128,64],[128,64,64],
                                   [128,128,64],[128,64,128],[64,128,128]]):
        named[f'max-min-tie-{index}'] = color
    colors = {tuple(c) for c in named.values()}
    # Deterministic local LCG, no random module/version dependency or dense RGB cube.
    state = 0x51A7BEEF
    for _ in range(512):
        rgb=[]
        for _ in range(3):
            state=(1664525 * state + 1013904223) & 0xffffffff
            rgb.append(state >> 24)
        colors.add(tuple(rgb))
    rows=[]; ties=[]
    for rgb in sorted(colors):
        for amount in AMOUNTS:
            values=exact(rgb,amount)
            output=[nearest(v) for v in values]
            rows.append(dict(rgb=list(rgb),amount_hundredths=amount,output=output))
            for channel,v in enumerate(values):
                if v.denominator == 2:
                    ties.append(dict(rgb=list(rgb),amount_hundredths=amount,channel=channel,
                                     numerator=v.numerator,denominator=2,output=output[channel]))
    grays=[]
    for gray in range(256):
        for amount in [-100,0,100]:
            out=candidate([gray]*3,amount);assert out==[gray]*3
            grays.append(dict(gray=gray,amount_hundredths=amount,output=out))
    rgba=[]
    for rgb in list(named.values()):
        for amount in [-100,-50,0,50,100]:
            transformed=candidate(rgb,amount)
            for alpha in [0,1,127,128,254,255]:
                for coverage in [0,1,127,128,254,255]:
                    rgba.append(dict(input=rgb+[alpha],amount_hundredths=amount,coverage=coverage,
                                     candidate=transformed+[alpha],output=mix(rgb,transformed,coverage)+[alpha]))
    assert all(r['output']==r['rgb'] for r in rows if r['amount_hundredths']==0)
    assert ties, 'finite deterministic sample must retain real half-rounding witnesses'
    return dict(status='INDEPENDENT_NUMERICAL_PLANNING_NOT_PRODUCT_SUPPORT',
                contract='RGB→HSL exact Fraction; s\u2032=clamp(s*(1+a*(1-s))), a=amount_hundredths/100; H/L unchanged; clamp RGB then nearest ties up once. Alpha unchanged incl0; A8 mixes rounded candidate as separate stage.',
                named_colors=named, samples=rows, grays=grays, half_ties=ties, rgba_mask=rgba)


if __name__ == '__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');args=parser.parse_args()
    data=generate();text=json.dumps(data,indent=2)+'\n';target=HERE/'oracle.json'
    if args.check:
        assert target.read_text()==text,'oracle drift'
    else:
        target.write_text(text)
    print(json.dumps({'samples':len(data['samples']),'grays':len(data['grays']),'half_ties':len(data['half_ties']),
                      'rgba_mask':len(data['rgba_mask']),'sha256':hashlib.sha256(text.encode()).hexdigest()}))
