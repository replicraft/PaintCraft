#!/usr/bin/env python3
"""Independent proposed Curves oracle. No product imports or kernel calls.

Solve each segment's four endpoint/value/derivative constraints, then evaluate
its polynomial by Horner. Integer basis below is ONLY an intermediate-bound
experiment; it does not supply expected LUT bytes.
"""
import argparse
from fractions import Fraction as F
import hashlib
import itertools
import json
from pathlib import Path
import random
import sys
import time

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
KNOTS = (0, 64, 128, 192, 255)
COVERAGES = (0, 1, 127, 128, 254, 255)
COLORS = ((0, 128, 255), (17, 203, 61), (255, 0, 1))


def solve(matrix):
    """Small exact Gaussian elimination, independent of Hermite basis code."""
    a = [[F(v) for v in row] for row in matrix]
    for column in range(4):
        pivot = next(r for r in range(column, 4) if a[r][column])
        a[column], a[pivot] = a[pivot], a[column]
        divisor = a[column][column]
        a[column] = [v / divisor for v in a[column]]
        for row in range(4):
            if row != column:
                factor = a[row][column]
                a[row] = [v - factor * p for v, p in zip(a[row], a[column])]
    return tuple(row[4] for row in a)


def polynomials(points):
    slopes = []
    for j in range(5):
        left, right = max(0, j - 1), min(4, j + 1)
        slopes.append(F(points[right] - points[left], KNOTS[right] - KNOTS[left]))
    result = []
    for i in range(4):
        d = KNOTS[i + 1] - KNOTS[i]
        # Coefficients a,b,c,e of a+b*n+c*n^2+e*n^3.
        result.append(solve([
            [1, 0, 0, 0, points[i]],
            [0, 1, 0, 0, slopes[i]],
            [1, d, d*d, d*d*d, points[i + 1]],
            [0, 1, 2*d, 3*d*d, slopes[i + 1]],
        ]))
    return result


def round_byte(value):
    value = min(F(255), max(F(0), value))
    return (2 * value.numerator + value.denominator) // (2 * value.denominator)


def table(points):
    assert len(points) == 5 and all(type(v) is int and 0 <= v <= 255 for v in points)
    segments = polynomials(points)
    exact, output = [], []
    for value in range(256):
        i = min(value // 64, 3)
        n = value - KNOTS[i]
        a, b, c, e = segments[i]
        y = ((e*n + c)*n + b)*n + a
        exact.append(y)
        output.append(round_byte(y))
    assert all(output[x] == p for x, p in zip(KNOTS, points))
    return output, exact


def profiles():
    result = [(f'vertex-{i:02d}', list(p)) for i, p in enumerate(itertools.product((0, 255), repeat=5))]
    result += [
        ('identity', [0, 64, 128, 192, 255]),
        ('inverse', [255, 191, 127, 63, 0]),
        ('free-endpoints', [23, 9, 240, 73, 201]),
        ('ordered-plateau', [0, 128, 128, 128, 255]),
        ('constant-37', [37]*5),
        ('last-span', [0, 0, 0, 0, 254]),
        ('centered-127', [0, 8, 251, 9, 243]),
        ('alternating-interior', [13, 230, 29, 177, 51]),
    ]
    rng = random.Random(0x435552564553)
    result += [(f'seed-{i:02d}', [rng.randrange(256) for _ in range(5)]) for i in range(24)]
    return result


def mix(table_bytes, pixel, coverage):
    # Incoming alpha is preserved, including hidden RGB when alpha is zero.
    return [((255-coverage)*v + coverage*table_bytes[v] + 127)//255 for v in pixel[:3]] + [pixel[3]]


def bounds():
    """Exact extrema of a specified checked integer basis evaluation order.

    Every recorded parameter-dependent intermediate is affine in five outputs.
    Therefore its extrema over the full real cube [0,255]^5 occur among the
    32 vertices. The n/d basis scalars depend only on the byte input. Clipping
    and rounding-input transforms are monotone and preserve endpoint bounds.
    This proves this order, not arbitrary reassociation/Horner implementations.
    """
    extrema = {}
    witness = {}
    def note(name, value, points, v):
        lo, hi = extrema.get(name, (value, value))
        extrema[name] = (min(lo, value), max(hi, value))
        if name not in witness or abs(value) > abs(witness[name]['value']):
            witness[name] = {'value': value, 'points': list(points), 'input': v}
    cases = 0
    for points in itertools.product((0, 255), repeat=5):
        expected_vertex, _ = table(points)
        for v in range(256):
            cases += 1
            i = min(v//64, 3)
            d = KNOTS[i+1] - KNOTS[i]
            n = v - KNOTS[i]
            left, right = max(0, i-1), min(4, i+1)
            dl = KNOTS[right] - KNOTS[left]
            delta_l = points[right] - points[left]
            left_r, right_r = i, min(4, i+2)
            dr = KNOTS[right_r] - KNOTS[left_r]
            delta_r = points[right_r] - points[left_r]
            scalars = {'n2':n*n, 'n3':n*n*n, 'd2':d*d, 'd3':d*d*d,
                'two_n3':2*n*n*n, 'three_n2_d':3*n*n*d,
                'two_n2_d':2*n*n*d, 'n_d2':n*d*d}
            h00 = 2*n**3-3*n*n*d+d**3
            h10 = n**3-2*n*n*d+n*d*d
            h01 = -2*n**3+3*n*n*d
            h11 = n**3-n*n*d
            scalars.update(h00=h00,h10=h10,h01=h01,h11=h11,
                h00_sub=2*n**3-3*n*n*d,h10_sub=n**3-2*n*n*d,
                negative_two_n3=-2*n**3,n2_d=n*n*d,
                delta_left=delta_l,delta_right=delta_r,width=d,
                offset=n,denominator_left=dl,denominator_right=dr,
                denominator_partial=d**3*dl)
            terms=[]
            for label, factors in [('value_left',(h00,points[i],dl,dr)),
                ('tangent_left',(h10,d,delta_l,dr)),
                ('value_right',(h01,points[i+1],dl,dr)),
                ('tangent_right',(h11,d,delta_r,dl))]:
                product=1
                for j, factor in enumerate(factors):
                    product *= factor
                    note(f'{label}_mul{j+1}',product,points,v)
                terms.append(product)
            partial=0
            for j, term in enumerate(terms):
                partial += term
                note(f'sum{j+1}',partial,points,v)
            denominator=d**3*dl*dr
            clipped=min(255*denominator,max(0,partial))
            scalars.update(denominator=denominator,clamp_upper=255*denominator,
                clipped=clipped,twice_clipped=2*clipped,round_numerator=2*clipped+denominator,
                round_denominator=2*denominator,rounded=(2*clipped+denominator)//(2*denominator))
            assert scalars['rounded'] == expected_vertex[v]
            for name, value in scalars.items():
                note(name,value,points,v)
    maximum=max(max(abs(a),abs(b)) for a,b in extrema.values())
    assert maximum < 2**63
    return {'method':'Affine intermediate extrema on all32 output vertices for every byte input; clipping monotone. No product code called.',
        'cases':cases,'evaluation_order':'h00*yL*dl*dr;h10*d*deltaL*dr;h01*yR*dl*dr;h11*d*deltaR*dl;left-to-right sum;clamp;2*N+D;2*D;integer division',
        'max_absolute':maximum,'signed_bits_required':maximum.bit_length()+1,
        'i64_sufficient_for_this_order':True,'vertex_bytes_crosschecked_against_fraction_solver':cases,
        'mask_intermediates_universal_bounds':{'single_product':65025,'sum_of_two_weighted_products':65025,'rounded_numerator':65152},
        'extrema':{k:{'min':v[0],'max':v[1],'absolute_witness':witness[k]} for k,v in sorted(extrema.items())}}


def make():
    rows, tables, ties = [], {}, []
    aggregate=hashlib.sha256()
    for name, points in profiles():
        lut, exact=table(points)
        tables[name]=lut
        raw=bytes(lut)
        aggregate.update(raw)
        rows.append({'name':name,'points':points,'lut_sha256':hashlib.sha256(raw).hexdigest(),
            'literals':[{'input':v,'expected':lut[v]} for v in (0,1,31,63,64,65,95,127,128,129,159,191,192,193,223,254,255)]})
        for v, y in enumerate(exact):
            if len(ties)<12 and 0 < y < 255 and y.denominator==2:
                ties.append({'name':name,'points':points,'input':v,'exact':[y.numerator,y.denominator],'expected':lut[v]})
    assert len(rows)==64 and len(ties)==12
    assert tables['identity']==list(range(256))
    rgba=[]; streams=[]; alpha_aggregate=hashlib.sha256()
    names=[name for name,_ in profiles()[32:40]]
    for name in names:
        lut=tables[name]
        for rgb in COLORS:
            for m in COVERAGES:
                for a in (0,128,255):
                    p=[*rgb,a]
                    rgba.append({'name':name,'input':p,'coverage':m,'expected':mix(lut,p,m)})
                raw=bytes(c for a in range(256) for c in mix(lut,[*rgb,a],m))
                alpha_aggregate.update(raw)
                streams.append({'name':name,'input_rgb':list(rgb),'coverage':m,'records':256,'rgba_sha256':hashlib.sha256(raw).hexdigest()})
    invalid=[[],[0]*4,[0]*6,[-1,0,0,0,0],[0,0,0,0,256],[True,0,0,0,0],[0.,64,128,192,255],None,'identity']
    return {'schema':'paintcraft.curves-proposed-independent-oracle/v1',
        'status':'Proposed numerical foundation only; no registered profile or product capability',
        'specification':'specification.md','knots':list(KNOTS),'parameter_encoding_proposed':'exact five u8 bytes in knot order',
        'generator_method':'Fraction Gaussian elimination of four endpoint/value/derivative constraints; polynomial Horner evaluation',
        'profiles':rows,'profile_count':len(rows),'full_lut_bytes':len(rows)*256,
        'aggregate_lut_sha256':aggregate.hexdigest(),'half_ties':ties,
        'rgba_mask_cases':rgba,'all_alpha_streams':streams,'all_alpha_records':len(streams)*256,
        'aggregate_all_alpha_sha256':alpha_aggregate.hexdigest(),
        'invalid_arrays':invalid,
        'invalid_lexical_json':['[0.0,64,128,192,255]','[0e0,64,128,192,255]','[true,64,128,192,255]','[0,64,128,192,255,]','[NaN,64,128,192,255]'],
        'lexical_policy_proposed':'Exact integer bytes; fraction/exponent/boolean/coercion/default rejected in proposed controls. Signed-zero lexical acceptance is undecided; final public JSON schema not chosen.',
        'bounds':bounds(),
        'coverage_note':'64 full-byte profiles and144 all-alpha streams; not exhaustive over2^40 parameter tuples. Future single-knot sweeps not run here.'}


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--write',action='store_true');args=parser.parse_args()
    started=time.monotonic()
    value=make()
    encoded=(json.dumps(value,indent=2,ensure_ascii=True,allow_nan=False)+'\n').encode()
    output=HERE/'oracle.json'
    if args.write: output.write_bytes(encoded)
    else: assert output.read_bytes()==encoded,'Frozen oracle differs; no overwrite during verification'
    print(json.dumps({'action':'write' if args.write else 'verify','elapsed_seconds':round(time.monotonic()-started,6),
        'sha256':hashlib.sha256(encoded).hexdigest(),'profiles':value['profile_count'],
        'alpha_records':value['all_alpha_records'],'bounds_max':value['bounds']['max_absolute'],
        'bounds_signed_bits':value['bounds']['signed_bits_required']}))

if __name__=='__main__':main()
