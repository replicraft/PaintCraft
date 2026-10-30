"""Independent UI-only PCND3 red/green specimen and exact Fraction planes.

Python stdlib only. No PaintCraft codec/evaluator imports. Run --write to author,
then run without arguments to verify every specimen and manifest byte.
"""
import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import uuid

HERE = Path(__file__).resolve().parent
DOCUMENT = '33333333-3333-4333-8333-333333333333'
STATE = '11111111-1111-4111-8111-111111111111'
LAYER = '22222222-2222-4222-8222-222222222222'
FILTER = '00000000-0000-0000-0000-000000000001'
RGBA = [255, 0, 0, 255, 0, 200, 0, 255]


def u(number):
    assert number >= 0
    out = bytearray()
    while number >= 128:
        out.append((number & 127) | 128)
        number >>= 7
    return bytes(out + bytes([number]))


def identifier(value):
    return uuid.UUID(value).bytes


def specimen():
    # Header, title-none, source-attachment-none and one ordinary root layer.
    body = b'PCND\x03\r\n\x1a' + identifier(DOCUMENT) + identifier(STATE)
    body += u(2) + u(1) + b'\0\0' + u(1) + identifier(LAYER)
    name = b'point-ui'
    body += u(len(name)) + name + b'\x01\xff'
    body += b'\0\0' + u(2) + u(1) + bytes(4)
    # One enabled, unmasked identity Black & White strength-zero prefix.
    body += u(1) + identifier(FILTER) + b'\x01\x03\0'
    # Raw 2x1 tile and end-of-layer marker. File hash covers the entire body.
    body += b'\x01\0\0' + u(len(RGBA)) + bytes(RGBA) + b'\0'
    return body + hashlib.sha256(body).digest()


def rounded(value):
    assert 0 <= value <= 255
    return (value.numerator * 2 + value.denominator) // (value.denominator * 2)


def evaluate(kind, parameter):
    output = []
    for offset in (0, 4):
        rgb = RGBA[offset:offset + 3]
        luma = sum(Fraction(channel) * coefficient for channel, coefficient in zip(rgb, (Fraction(2126, 10000), Fraction(7152, 10000), Fraction(722, 10000))))
        if kind == 'bw':
            result = [rounded(Fraction(100 - parameter, 100) * channel + Fraction(parameter, 100) * luma) for channel in rgb]
        elif kind == 'threshold':
            result = [255 if luma >= parameter else 0] * 3
        elif kind == 'posterize':
            k = parameter - 1
            result = [rounded(Fraction(255 * rounded(Fraction(channel * k, 255)), k)) for channel in rgb]
        elif kind == 'solarize':
            result = [255 - channel if channel >= 128 else channel for channel in rgb]
        else:
            raise ValueError('Unknown independent algorithm')
        output += result + [255]
    return output


def manifest(data):
    settings = [('bw', 100, 50), ('threshold', 128, 200), ('posterize', 6, 2), ('solarize', None, None)]
    cases = [{'kind': kind, 'default_parameter': default, 'changed_parameter': changed, 'default_rgba': evaluate(kind, default), 'changed_rgba': evaluate(kind, changed)} for kind, default, changed in settings]
    assert [case['default_rgba'] for case in cases] == [[54, 54, 54, 255, 143, 143, 143, 255], [0, 0, 0, 255, 255, 255, 255, 255], [255, 0, 0, 255, 0, 204, 0, 255], [0, 0, 0, 255, 0, 55, 0, 255]]
    for case in cases:
        assert case['default_rgba'] != RGBA
        assert case['default_rgba'][:3] != case['default_rgba'][4:7]
    return {'schema': 'paintcraft.browser-point-ui-fixture/v1', 'generator': __doc__, 'document': DOCUMENT, 'state': STATE, 'layer': LAYER, 'width': 2, 'height': 1, 'cases': cases, 'fixtures': [{'file': 'red-green.pcnd', 'accepted': True, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'version': 3, 'attachment': False, 'raw_rgba': RGBA, 'evaluated_rgba': RGBA, 'rendered_rgba': RGBA, 'filters': [{'id': FILTER, 'enabled': True, 'descriptor': {'kind': 'black-white-rgb-v1', 'strength': 0}, 'coverage': None}]}]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    data = specimen()
    files = {'red-green.pcnd': data, 'oracle.json': (json.dumps(manifest(data), indent=2) + '\n').encode()}
    for name, content in files.items():
        if args.write:
            (HERE / name).write_bytes(content)
        else:
            assert (HERE / name).read_bytes() == content, f'{name} differs'
    print(json.dumps({'status': 'written' if args.write else 'verified', 'specimens': 1, 'default_cases': 4, 'sha256': hashlib.sha256(data).hexdigest()}, sort_keys=True))


if __name__ == '__main__':
    main()
