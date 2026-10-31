"""Independent finite Add Noise mathematical reference; not software-libm authority."""
import argparse
from decimal import Decimal as D, localcontext, ROUND_FLOOR
from fractions import Fraction as F
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
MASK = (1 << 32) - 1
TAU_BITS = 0x401921FB54442D18
# Decode the named binary64 constant by integer exponent/significand arithmetic.
TAU_RATIONAL = F((1 << 52) + (TAU_BITS & ((1 << 52) - 1)), 1 << 50)
COORDS = [(0, 0), (1, 0), (0, 1), (63, 64), (64, 63), (64, 64),
          (65, 65), (100, 200), (MASK, 0), (0, MASK), (MASK, MASK), (MASK-1, 65)]
RGBS = [(0, 0, 0), (255, 255, 255), (128, 128, 128),
        (127, 128, 129), (64, 16, 128), (255, 1, 254)]
ALPHAS = [0, 1, 127, 128, 254, 255]
COVERAGES = [0, 1, 63, 127, 254, 255]


def integer(value, low, high):
    if type(value) is not int or not low <= value <= high:
        raise ValueError('strict integer outside domain')
    return value


def policy(amount, seed, mono, distribution):
    integer(amount, 0, 100)
    integer(seed, 0, 65535)
    if type(mono) is not bool or distribution not in ('uniform', 'gaussian'):
        raise ValueError('strict monochrome/distribution')
    return amount.to_bytes(2, 'big') + seed.to_bytes(2, 'big') + bytes([mono, distribution == 'gaussian'])


def mix(n):
    n ^= n >> 16
    n = (n * 0x7FEB352D) & MASK
    n ^= n >> 15
    n = (n * 0x846CA68B) & MASK
    return n ^ (n >> 16)


def draws(x, y, seed, channel):
    integer(x, 0, MASK)
    integer(y, 0, MASK)
    integer(seed, 0, 65535)
    integer(channel, 0, 2)
    state = (0xA511E9B3 ^ ((x*0x9E3779B9) & MASK) ^ ((y*0x85EBCA6B) & MASK)
             ^ ((seed*0xC2B2AE35) & MASK) ^ ((channel*0x27D4EB2D) & MASK))
    first = mix(state)
    return first, mix(first)  # SAME mutable state; never independent seeds.


def rounded_fraction(v):
    if v <= 0:
        return 0
    if v >= 255:
        return 255
    return (2*v.numerator + v.denominator) // (2*v.denominator)


def uniform(rgb, amount, words):
    if amount == 0:
        return list(rgb)
    return [rounded_fraction(F(c) + F(255*amount, 100) * F(2*w[0]+1-(1 << 32), 1 << 32))
            for c, w in zip(rgb, words)]


def decimal_fraction(v):
    return D(v.numerator) / D(v.denominator)


@lru_cache(None)
def pi(precision):
    # Machin identity, independently summed arctangent power series. Precision
    # agreement is NOT a certified enclosure or a global error bound.
    with localcontext() as ctx:
        ctx.prec = precision + 20
        def atan_reciprocal(n):
            x = D(1)/D(n)
            square, term, result, k = x*x, x, x, 1
            while True:
                term *= -square
                following = result + term/D(2*k+1)
                if following == result:
                    return result
                result, k = following, k+1
        return +(16*atan_reciprocal(5)-4*atan_reciprocal(239))


def cosine(angle, precision):
    p = pi(precision)
    angle %= 2*p
    if angle > p:
        angle -= 2*p
    sign = 1
    if abs(angle) > p/2:
        angle = p-abs(angle)
        sign = -1
    term = result = D(1)
    k = 1
    while True:
        term *= -(angle*angle)/D((2*k-1)*(2*k))
        following = result+term
        if following == result:
            return sign*result
        result, k = following, k+1


@lru_cache(None)
def gaussian_noise(first, second, precision, binary_tau):
    with localcontext() as ctx:
        ctx.prec = precision+20
        u1 = D(2*first+1)/D(1 << 33)
        u2 = D(2*second+1)/D(1 << 33)
        tau = decimal_fraction(TAU_RATIONAL) if binary_tau else 2*pi(precision)
        return +((-2*u1.ln()).sqrt()*cosine(tau*u2, precision))


def gaussian(rgb, amount, words, precision, binary_tau):
    if amount == 0:
        return list(rgb), None
    out, margins = [], []
    with localcontext() as ctx:
        ctx.prec = precision+20
        for channel, pair in zip(rgb, words):
            # This mathematical reference has NO intermediate binary64 rounding.
            v = D(channel) + D(255)*D(amount)/D(100)*gaussian_noise(*pair, precision, binary_tau)
            if v <= 0:
                out.append(0)
            elif v >= 255:
                out.append(255)
            else:
                floor = v.to_integral_value(rounding=ROUND_FLOOR)
                fraction = v-floor
                out.append(int(floor) + int(fraction >= D('0.5')))
                margins.append(abs(fraction-D('0.5')))
    return out, str(min(margins)) if margins else None


def coverage(source, candidate, amount):
    # Existing point coverage contract: mix ONCE after candidate RGB8 rounding.
    return [(a*(255-amount)+b*amount+127)//255 for a, b in zip(source, candidate)]


def build():
    rows, ambiguities, differences, mask_stream = [], [], [], bytearray()
    for x, y in COORDS:
        for rgb in RGBS:
            for seed in [0, 1, 65535]:
                for amount in [0, 1, 10, 50, 100]:
                    for mono in [True, False]:
                        for distribution in ['uniform', 'gaussian']:
                            index = len(rows)
                            alpha = ALPHAS[(index // 2) % 6]
                            rgba = [*rgb, alpha]
                            semantic = policy(amount, seed, mono, distribution)
                            words = [draws(x, y, seed, 0 if mono else c) for c in range(3)]
                            row = {'index': index, 'source_local': [x, y],
                                   'world_origin': [-100, -200], 'rgba': rgba,
                                   'amount_hundredths': amount, 'seed': seed, 'monochrome': mono,
                                   'distribution': distribution, 'semantic_hex': semantic.hex(),
                                   'successive_words': [list(pair) for pair in words]}
                            if distribution == 'uniform':
                                expected = uniform(rgb, amount, words)
                                row['exact_fraction_rgb'] = expected
                            else:
                                ideal96, _ = gaussian(rgb, amount, words, 96, False)
                                ideal160, ideal_margin = gaussian(rgb, amount, words, 160, False)
                                binary96, _ = gaussian(rgb, amount, words, 96, True)
                                binary160, binary_margin = gaussian(rgb, amount, words, 160, True)
                                row.update(ideal_real_tau_rgb=ideal160,
                                           exact_binary_tau_math_rgb=binary160,
                                           ideal_half_margin=ideal_margin,
                                           binary_tau_half_margin=binary_margin,
                                           precision_96_160_agree=(ideal96 == ideal160 and binary96 == binary160))
                                if not row['precision_96_160_agree']:
                                    ambiguities.append(index)
                                if ideal160 != binary160:
                                    differences.append(index)
                                expected = binary160
                            row['reference_rgba'] = [*expected, alpha]
                            row['mask_stream_offset'] = len(mask_stream)
                            for mask in COVERAGES:
                                mask_stream.extend([*coverage(rgb, expected, mask), alpha])
                            rows.append(row)
    packed = bytes(channel for row in rows for channel in row['reference_rgba'])
    assert len(rows) == 4320
    return {'status': 'MATHEMATICAL_REFERENCE_ONLY_NORMATIVE_SOFTWARE_AUTHORITY_PENDING',
            'record_order': 'coords,rgb,seed,amount,mono(True/False),distribution(uniform/gaussian)',
            'rounding': 'clamp then nearest ties upward; alpha exact; mask once after candidate',
            'precision': {'requested_digits': [96, 160], 'guard_digits': 20,
                          'not_certified_intervals': True, 'not_global_epsilon': True},
            'gaussian_software_graph': {'status': 'PENDING_FINAL_ROOT_GRAPH',
                'tau_bits': f'{TAU_BITS:016x}', 'tau_rational': [TAU_RATIONAL.numerator, TAU_RATIONAL.denominator],
                'reference_does_not_emulate_libm_or_IEEE_intermediate_steps': True},
            'source_local_coordinate_rule': 'world_position=world_origin+source_local; hash uses only source_local, never tile/partition/world position',
            'partition_widths_for_consumers': [1, 32, 63, 64, 65, 128],
            'rows': rows, 'precision_ambiguities': ambiguities,
            'ideal_vs_binary_tau_byte_differences': differences,
            'rgba_stream_sha256': hashlib.sha256(packed).hexdigest(),
            'six_coverages': COVERAGES, 'mask_rgba_stream_records': len(rows)*6,
            'mask_rgba_stream_sha256': hashlib.sha256(mask_stream).hexdigest()}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--write', action='store_true')
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    encoded = (json.dumps(build(), indent=2)+'\n').encode()
    target = HERE/'oracle.json'
    if args.write:
        assert not target.exists(), 'preserve first generated oracle; never overwrite'
        target.write_bytes(encoded)
    if args.check:
        assert target.read_bytes() == encoded
    print(json.dumps({'records': 4320, 'sha256': hashlib.sha256(encoded).hexdigest(),
                      'bytes': len(encoded), 'written': args.write, 'checked': args.check}))


if __name__ == '__main__':
    main()
