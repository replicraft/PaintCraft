#!/usr/bin/env python3
"""Independent exact Exposure corpus; standard library, no product imports/floats."""
import sys
sys.dont_write_bytecode = True

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
EV_MIN, EV_MAX = -800, 800
ENCODED_KNEE = Fraction(4045, 100000)
LINEAR_KNEE = Fraction(31308, 10000000)
SLOPE = Fraction(1292, 100)
OFFSET = Fraction(55, 1000)
SCALE = Fraction(1055, 1000)
SAMPLES = (0, 1, 9, 10, 11, 16, 64, 128, 254, 255)


def valid_stops(value):
    if type(value) is not int or not EV_MIN <= value <= EV_MAX:
        raise ValueError('stops_hundredths must be a strict integer -800..800')
    return value


def valid_byte(value):
    if type(value) is not int or not 0 <= value <= 255:
        raise ValueError('channel must be a strict integer 0..255')
    return value


def decode_power_100(encoded):
    """sRGB decode(x)^100: exponent 12/5 becomes integer 240."""
    assert isinstance(encoded, Fraction) and 0 <= encoded <= 1
    if encoded <= ENCODED_KNEE:
        return (encoded / SLOPE) ** 100
    return ((encoded + OFFSET) / SCALE) ** 240


def compare(pair, stops):
    """Compare exact positive quantities, including equality (ties round up)."""
    numerator, denominator = pair
    if stops >= 0:
        return (numerator << stops) >= denominator
    return numerator >= (denominator << -stops)


def quantize(pairs, stops):
    """Independent binary threshold search; never a monotone parameter sweep."""
    valid_stops(stops)
    lower, upper = 0, 255
    while lower < upper:
        middle = (lower + upper) // 2
        if compare(pairs[middle], stops):
            lower = middle + 1
        else:
            upper = middle
    return lower


def threshold_pairs(channel, thresholds):
    value = decode_power_100(Fraction(valid_byte(channel), 255))
    return [(value.numerator * threshold.denominator,
             threshold.numerator * value.denominator) for threshold in thresholds]


def prove_knee_separation():
    lower, upper = Fraction(19, 510), Fraction(21, 510)
    assert lower < ENCODED_KNEE < upper
    # All output half-byte thresholds lie outside the inverse transfer's tiny
    # discontinuity. The nearest two bracket the linear-space encode knee.
    assert lower / SLOPE < LINEAR_KNEE
    assert ((upper + OFFSET) / SCALE) ** 12 > LINEAR_KNEE ** 5
    return {
        'encoded_knee': str(ENCODED_KNEE),
        'linear_encode_knee': str(LINEAR_KNEE),
        'nearest_lower_half_byte': str(lower),
        'nearest_upper_half_byte': str(upper),
        'proof': 'lower/12.92 < encode knee; nonlinear upper decode > encode knee, proved by integer powers',
    }


def mix(incoming, candidate, coverage):
    valid_byte(coverage)
    return [(incoming[c] * (255 - coverage) + candidate[c] * coverage + 127) // 255
            for c in range(3)] + [incoming[3]]


def literal_controls(tables):
    # Low-linear exact half ties. Expected bytes are declared literals, not
    # computed by the classifier under test.
    ties = [(-100, 1, 1), (-100, 3, 2), (-100, 5, 3), (-100, 7, 4),
            (-100, 9, 5), (-200, 2, 1), (-200, 6, 2), (-200, 10, 3),
            (-300, 4, 1), (-400, 8, 1)]
    tie_records = []
    for stops, channel, expected in ties:
        assert tables[stops - EV_MIN][channel] == expected
        tie_records.append({'stops_hundredths': stops, 'input': channel, 'output': expected})
    # Explicit literal controls include linear-light behavior, clipping and knee.
    literals = [(-800, 0, 0), (800, 0, 0), (800, 255, 255),
                (0, 10, 10), (0, 11, 11), (100, 1, 2)]
    for stops, channel, expected in literals:
        assert tables[stops - EV_MIN][channel] == expected
    return tie_records, [{'stops_hundredths': s, 'input': v, 'output': o} for s, v, o in literals]


def rgba_controls(tables):
    records = []
    colors = [(0, 10, 11), (1, 64, 255), (128, 16, 200)]
    for stops in (-800, -100, -1, 0, 1, 100, 800):
        table = tables[stops - EV_MIN]
        for rgb in colors:
            for alpha in (0, 1, 127, 255):
                incoming = list(rgb) + [alpha]
                candidate = [table[v] for v in rgb] + [alpha]
                for coverage in (0, 1, 128, 254, 255):
                    output = mix(incoming, candidate, coverage)
                    assert output[3] == alpha
                    if coverage == 0 or stops == 0:
                        assert output == incoming
                    records.append({'stops_hundredths': stops, 'input_rgba': incoming,
                                    'coverage': coverage, 'output_rgba': output})
    return records



def all_alpha_controls(tables):
    """Complete alpha coverage without expanding thousands of JSON samples."""
    records, aggregate = [], hashlib.sha256()
    for stops in (-800, -100, -1, 0, 1, 100, 800):
        table = tables[stops - EV_MIN]
        for rgb in ((0, 10, 11), (1, 64, 255), (128, 16, 200)):
            for coverage in (0, 1, 128, 254, 255):
                stream = bytearray()
                for alpha in range(256):
                    incoming = list(rgb) + [alpha]
                    candidate = [table[v] for v in rgb] + [alpha]
                    output = mix(incoming, candidate, coverage)
                    assert output[3] == alpha
                    if coverage == 0 or stops == 0:
                        assert output == incoming
                    stream.extend(output)
                assert len(stream) == 256 * 4
                aggregate.update(stream)
                records.append({'stops_hundredths': stops, 'input_rgb': list(rgb),
                                'coverage': coverage, 'rgba_sha256': hashlib.sha256(stream).hexdigest()})
    return {
        'record_order': 'EV [-800,-100,-1,0,1,100,800], RGB [[0,10,11],[1,64,255],[128,16,200]], coverage [0,1,128,254,255]; each output stream is RGBA for input alpha 0..255 ascending',
        'stream_count': len(records), 'rgba_pixels': len(records) * 256,
        'bytes': len(records) * 256 * 4,
        'aggregate_rgba_sha256': aggregate.hexdigest(), 'streams': records,
    }


def generate():
    knee = prove_knee_separation()
    thresholds = [decode_power_100(Fraction(2 * j + 1, 510)) for j in range(255)]
    assert all(a < b for a, b in zip(thresholds, thresholds[1:]))
    tables = [bytearray(256) for _ in range(EV_MAX - EV_MIN + 1)]
    for channel in range(256):
        pairs = threshold_pairs(channel, thresholds)
        for stops in range(EV_MIN, EV_MAX + 1):
            tables[stops - EV_MIN][channel] = quantize(pairs, stops)
    assert tables[-EV_MIN] == bytes(range(256))  # Identity independently derived.
    assert all(table[0] == 0 for table in tables)
    assert all(all(a <= b for a, b in zip(table, table[1:])) for table in tables)
    assert all(all(a <= b for a, b in zip(column, column[1:]))
               for column in zip(*tables))
    ties, literals = literal_controls(tables)
    rejected = []
    for invalid in (-801, 801, True, False, 1.0, '100', None):
        try:
            valid_stops(invalid)
        except ValueError:
            rejected.append({'input': invalid, 'error': 'input.schema'})
        else:
            raise AssertionError(('invalid parameter accepted', invalid))
    aggregate = hashlib.sha256()
    rows = []
    for stops, table in zip(range(EV_MIN, EV_MAX + 1), tables):
        aggregate.update(table)
        rows.append({'stops_hundredths': stops, 'lut_sha256': hashlib.sha256(table).hexdigest(),
                     'literals': [{'input': v, 'output': table[v]} for v in SAMPLES]})
    return {
        'schema': 'paintcraft.exposure-independent-oracle/v1',
        'status': 'proposed numerical foundation; no Descriptor, persistence or UI implementation claim',
        'profile_id': 'exposure-srgb-v1',
        'generator_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'arithmetic': 'exact rational sRGB, exact 2^(stops/100), clamped linear light, encoded nearest ties-up; no float oracle',
        'record_order': 'stops_hundredths ascending -800..800, then input byte ascending 0..255',
        'lut_count': len(rows), 'channels': len(rows) * 256,
        'aggregate_lut_sha256': aggregate.hexdigest(),
        'knee_proof': knee, 'luts': rows,
        'exact_half_ties': ties, 'literal_controls': literals,
        'rgba_mask_controls': rgba_controls(tables),
        'all_alpha_mask_controls': all_alpha_controls(tables),
        'invalid_parameters': rejected,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true', help='write only the new oracle file')
    args = parser.parse_args()
    result = generate()
    encoded = (json.dumps(result, indent=2, sort_keys=True) + '\n').encode()
    target = HERE / 'oracle.json'
    if args.write:
        target.write_bytes(encoded)
    elif not target.exists() or target.read_bytes() != encoded:
        raise SystemExit('oracle.json is missing or differs; existing historical fixtures are never modified')
    print(json.dumps({'luts': result['lut_count'], 'channels': result['channels'],
                      'rgba_mask_controls': len(result['rgba_mask_controls']),
                      'all_alpha_pixels': result['all_alpha_mask_controls']['rgba_pixels'],
                      'oracle_sha256': hashlib.sha256(encoded).hexdigest(),
                      'aggregate_lut_sha256': result['aggregate_lut_sha256']}))


if __name__ == '__main__':
    main()
