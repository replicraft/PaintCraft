#!/usr/bin/env python3
"""Independent proposed point profiles; stdlib only, never imports product code."""
import argparse
from fractions import Fraction
import hashlib
import itertools
import json
from pathlib import Path

ALPHAS = (0, 1, 128, 255)
ANCHORS = (0, 1, 63, 64, 127, 128, 192, 254, 255)
KINDS = ("black-white-rgb-v1", "threshold-luma-v1", "posterize-rgb-v1", "solarize-rgb-v1")
HERE = Path(__file__).resolve().parent


def integer(value, low, high):
    if type(value) is not int or not low <= value <= high:
        raise ValueError("out-of-domain integer")
    return value


def rounded(numerator, denominator):
    """Nearest nonnegative rational, exact half rounds upward."""
    if numerator < 0 or denominator <= 0:
        raise ValueError("invalid rounding input")
    return (2 * numerator + denominator) // (2 * denominator)


def validate(kind, parameter):
    if kind == KINDS[0]:
        integer(parameter, 0, 100)
    elif kind == KINDS[1]:
        integer(parameter, 0, 255)
    elif kind == KINDS[2]:
        integer(parameter, 2, 32)
    elif kind == KINDS[3]:
        if parameter is not None:
            raise ValueError("Solarize has no parameter")
    else:
        raise ValueError("unknown proposed kind")


def point(kind, parameter, rgba):
    validate(kind, parameter)
    if len(rgba) != 4:
        raise ValueError("RGBA arity")
    for channel in rgba:
        integer(channel, 0, 255)
    r, g, b, alpha = rgba
    if kind == KINDS[0]:
        luma = 2126 * r + 7152 * g + 722 * b
        rgb = tuple(rounded((100 - parameter) * 10000 * c + parameter * luma, 1000000) for c in (r, g, b))
    elif kind == KINDS[1]:
        luma = 2126 * r + 7152 * g + 722 * b
        rgb = (255 if luma >= 10000 * parameter else 0,) * 3
    elif kind == KINDS[2]:
        steps = parameter - 1
        rgb = tuple(rounded(255 * rounded(c * steps, 255), steps) for c in (r, g, b))
    else:
        rgb = tuple(255 - c if c >= 128 else c for c in (r, g, b))
    return (*rgb, alpha)


def mix(source, candidate, coverage):
    integer(coverage, 0, 255)
    assert source[3] == candidate[3]
    return (*(rounded((255 - coverage) * a + coverage * b, 255) for a, b in zip(source[:3], candidate[:3])), source[3])


def compose(source, stages):
    pixel = source
    for kind, parameter, coverage in stages:
        pixel = mix(pixel, point(kind, parameter, pixel), coverage)
    return pixel


def fraction_reference(kind, parameter, rgba):
    """Separate rational expression checks, including both posterization rounds."""
    def nearest(value):
        value += Fraction(1, 2)
        return value.numerator // value.denominator
    r, g, b, alpha = rgba
    if kind == KINDS[0]:
        y = Fraction(2126 * r + 7152 * g + 722 * b, 10000)
        rgb = tuple(nearest(Fraction(100 - parameter, 100) * c + Fraction(parameter, 100) * y) for c in (r, g, b))
    elif kind == KINDS[1]:
        y = Fraction(2126 * r + 7152 * g + 722 * b, 10000)
        rgb = (255 if y >= parameter else 0,) * 3
    elif kind == KINDS[2]:
        k = parameter - 1
        rgb = tuple(nearest(Fraction(255 * nearest(Fraction(c * k, 255)), k)) for c in (r, g, b))
    else:
        rgb = tuple(c if c < 128 else 255 - c for c in (r, g, b))
    return (*rgb, alpha)


def literal_controls():
    # Independently written expectations, not copied from a product renderer.
    controls = [
        (KINDS[0], 100, (255, 0, 0, 0), (54, 54, 54, 0)),
        (KINDS[0], 100, (0, 255, 0, 1), (182, 182, 182, 1)),
        (KINDS[0], 100, (0, 0, 255, 128), (18, 18, 18, 128)),
        (KINDS[0], 50, (255, 0, 0, 255), (155, 27, 27, 255)),
        (KINDS[0], 100, (20, 115, 0, 255), (87, 87, 87, 255)),
        (KINDS[0], 0, (13, 178, 247, 0), (13, 178, 247, 0)),
        (KINDS[1], 128, (0, 178, 4, 0), (0, 0, 0, 0)),
        (KINDS[1], 128, (128, 128, 128, 1), (255, 255, 255, 1)),
        (KINDS[1], 0, (0, 0, 0, 128), (255, 255, 255, 128)),
        (KINDS[1], 255, (255, 255, 254, 255), (0, 0, 0, 255)),
        (KINDS[1], 255, (255, 255, 255, 0), (255, 255, 255, 0)),
        (KINDS[2], 2, (127, 128, 255, 0), (0, 255, 255, 0)),
        (KINDS[2], 3, (64, 127, 191, 1), (128, 128, 128, 1)),
        (KINDS[3], None, (127, 128, 255, 128), (127, 127, 0, 128)),
        (KINDS[3], None, (0, 129, 254, 0), (0, 126, 1, 0)),
    ]
    result = []
    for kind, parameter, source, expected in controls:
        assert point(kind, parameter, source) == expected
        assert fraction_reference(kind, parameter, source) == expected
        result.append({"kind": kind, "parameter": parameter, "rgba": source, "expected": expected})
    return result


class Stream:
    def __init__(self, order, record):
        self.hash = hashlib.sha256()
        self.records = 0
        self.bytes = 0
        self.order = order
        self.record = record
    def add(self, values):
        encoded = bytes(values)
        self.hash.update(encoded)
        self.records += 1
        self.bytes += len(encoded)
    def finish(self):
        return {"records": self.records, "bytes": self.bytes, "sha256": self.hash.hexdigest(), "order": self.order, "record": self.record}


def streams():
    output = {}
    unary = Stream("input c=0..255", "one output byte")
    for c in range(256):
        pixel = (c, c, c, 0)
        answer = point(KINDS[3], None, pixel)
        assert answer == fraction_reference(KINDS[3], None, pixel)
        unary.add(answer[:1])
    output["solarize_all_channels"] = unary.finish()
    poster = Stream("levels=2..32 outer; input c=0..255 inner", "one output byte")
    for levels, c in itertools.product(range(2, 33), range(256)):
        pixel = (c, c, c, 0)
        answer = point(KINDS[2], levels, pixel)
        assert answer == fraction_reference(KINDS[2], levels, pixel)
        poster.add(answer[:1])
    output["posterize_all_parameters_channels"] = poster.finish()
    identity = Stream("strength=0..100; gray=0..255; alpha=0,1,128,255", "four output RGBA bytes")
    for strength, gray, alpha in itertools.product(range(101), range(256), ALPHAS):
        source = (gray, gray, gray, alpha)
        answer = point(KINDS[0], strength, source)
        assert answer == source
        identity.add(answer)
    output["black_white_gray_identity"] = identity.finish()
    gray_threshold = Stream("level=0..255; gray=0..255; alpha=0,1,128,255", "four output RGBA bytes")
    for level, gray, alpha in itertools.product(range(256), range(256), ALPHAS):
        answer = point(KINDS[1], level, (gray, gray, gray, alpha))
        expected = (255 if gray >= level else 0, ) * 3 + (alpha, )
        assert answer == expected
        gray_threshold.add(answer)
    output["threshold_all_gray_boundaries"] = gray_threshold.finish()
    brackets = Stream("level=0..255; R then G in listed anchors; sorted distinct B={0,255,crossing-1,crossing,crossing+1} clipped0..255", "level,R,G,B,one output gray byte")
    for level, r, g in itertools.product(range(256), ANCHORS, ANCHORS):
        remainder = 10000 * level - 2126 * r - 7152 * g
        crossing = -(-remainder // 722)
        for b in sorted({0, 255, *(v for v in (crossing-1, crossing, crossing+1) if 0 <= v <= 255)}):
            expected = 255 if 2126*r + 7152*g + 722*b >= 10000*level else 0
            answer = point(KINDS[1], level, (r, g, b, 0))
            assert answer[:3] == (expected,)*3
            assert answer == fraction_reference(KINDS[1], level, (r, g, b, 0))
            brackets.add((level, r, g, b, expected))
    output["threshold_chromatic_brackets"] = brackets.finish()
    cross = Stream("strength=0..100; R then G then B in0,127,128,255; alpha=0,1,128,255", "four output RGBA bytes")
    for strength, r, g, b, alpha in itertools.product(range(101), (0,127,128,255), (0,127,128,255), (0,127,128,255), ALPHAS):
        source = (r,g,b,alpha)
        answer = point(KINDS[0], strength, source)
        if alpha == 0:
            assert answer == fraction_reference(KINDS[0], strength, source)
        if strength == 0:
            assert answer == source
        assert answer[3] == alpha
        cross.add(answer)
    output["black_white_cross_channels"] = cross.finish()
    return output


def contextual_controls():
    source = (0,178,4,0)
    bw_then_threshold = compose(source, [(KINDS[0],100,255),(KINDS[1],128,255)])
    direct_threshold = compose(source, [(KINDS[1],128,255)])
    assert bw_then_threshold == (255,255,255,0)
    assert direct_threshold == (0,0,0,0)
    samples = []
    for kind, parameter in [(KINDS[0],100),(KINDS[1],128),(KINDS[2],6),(KINDS[3],None)]:
        for alpha, coverage in itertools.product(ALPHAS,(0,1,127,128,254,255)):
            source = (17,129,241,alpha)
            candidate = point(kind,parameter,source)
            answer = mix(source,candidate,coverage)
            assert answer[3] == alpha
            if coverage == 0: assert answer == source
            if coverage == 255: assert answer == candidate
            samples.append({"kind":kind,"parameter":parameter,"rgba":source,"coverage":coverage,"expected":answer})
    tie_source = (20,115,0,0)
    tie_mask = mix(tie_source,point(KINDS[0],100,tie_source),128)
    assert tie_mask == (54,101,44,0)
    return {"order":{"rgba":(0,178,4,0),"black_white100_then_threshold128":bw_then_threshold,"direct_threshold128":direct_threshold},"rounded_candidate_before_mask":{"rgba":tie_source,"strength":100,"coverage":128,"expected":tie_mask},"mask_samples":samples}


def reject_controls():
    count = 0
    for kind, values in [
        (KINDS[0], (-1,101,1.0,True,None,float('nan'))),
        (KINDS[1], (-1,256,1.0,False,None)),
        (KINDS[2], (0,1,33,256,2.0,True,None)),
        (KINDS[3], (0,False,1.0)),
        ("unknown-v1", (None,)),
    ]:
        for parameter in values:
            try:
                point(kind,parameter,(0,0,0,0))
            except ValueError:
                count += 1
            else:
                raise AssertionError((kind,parameter))
    return count


def specimen():
    return {
        "schema":"paintcraft.proposed-point-oracle/v1",
        "status":"Proposed independent numerical fixtures; product implementation not started by this checkpoint",
        "arithmetic":"straight encoded RGB8; integer rational nearest ties upward; unchanged alpha and transformed hidden RGB",
        "source_reference":{"repository":"https://github.com/spiffyfitz/palette","commit":"aefc84fe9233e66344147078de866860326d8cfa","path":"crates/palette-core/src/processing/point.rs","relationship":"independent exact rational contract, not upstream float byte-parity qualification"},
        "chromatic_threshold_anchors":ANCHORS,
        "literal_controls":literal_controls(),
        "streams":streams(),
        "context":contextual_controls(),
        "rejected_domain_controls":reject_controls(),
    }


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write',action='store_true',help='replace proposed oracle.json intentionally')
    args=parser.parse_args()
    value=specimen()
    data=(json.dumps(value,indent=2,sort_keys=True,allow_nan=False)+'\n').encode()
    target=HERE/'oracle.json'
    if args.write:
        target.write_bytes(data)
    else:
        if not target.exists() or target.read_bytes()!=data:
            raise SystemExit('oracle.json differs; inspect contract changes before --write')
    print(json.dumps({'status':'generated' if args.write else 'verified','streams':len(value['streams']),'records':sum(v['records'] for v in value['streams'].values()),'rejected_domains':value['rejected_domain_controls'],'sha256':hashlib.sha256(data).hexdigest()},sort_keys=True))


if __name__=='__main__':
    main()
