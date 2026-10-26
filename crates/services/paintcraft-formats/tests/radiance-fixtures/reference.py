"""Owned RGBE packet fixtures and independent scalar sample reconstruction.

Packet syntax follows the original Radiance color.c reader; lower-edge and
Radiance midpoint representatives are emitted separately, not conflated.
"""
import hashlib
import json
import math
from pathlib import Path
import struct

root = Path(__file__).parent
manifest = []


def reference(width, height, body):
    offset = 0
    pixels = []

    def take(count):
        nonlocal offset
        out = body[offset:offset + count]
        assert len(out) == count
        offset += count
        return out

    for _ in range(height):
        first = take(4)
        if 8 <= width <= 32767 and first[:2] == bytes([2, 2]) and first[2] < 128:
            assert int.from_bytes(first[2:], 'big') == width
            planes = []
            for _ in range(4):
                plane = []
                while len(plane) < width:
                    count = take(1)[0]
                    assert count != 0
                    plane.extend([take(1)[0]] * (count - 128) if count > 128 else take(count))
                assert len(plane) == width
                planes.append(plane)
            pixels.extend(zip(*planes))
        else:
            assert first[:3] != bytes([1, 1, 1])
            row = [first]
            shift = 0
            while len(row) < width:
                pixel = take(4)
                if pixel[:3] == bytes([1, 1, 1]):
                    row.extend([row[-1]] * (pixel[3] << shift))
                    shift += 8
                else:
                    row.append(pixel)
                    shift = 0
            assert len(row) == width
            pixels.extend(row)
    assert offset == len(body)
    lower, midpoint = [], []
    for r, g, b, exponent in pixels:
        lower.extend([math.ldexp(v, exponent - 136) if exponent else 0.0 for v in [r, g, b]])
        midpoint.extend([math.ldexp(v + .5, exponent - 136) if exponent else 0.0 for v in [r, g, b]])
    return lower, midpoint


def write(name, width, height, body, metadata='', error=None, resolution=None):
    resolution = resolution or f'-Y {height} +X {width}'
    header = '#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n' + metadata + '\n' + resolution + '\n'
    data = header.encode() + body
    (root / (name + '.hdr')).write_bytes(data)
    record = {'file': name + '.hdr', 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'width': width, 'height': height, 'expected_error': error}
    if error is None:
        lower, midpoint = reference(width, height, body)
        for suffix, values in [('lower-edge.f32le', lower), ('midpoint.f32le', midpoint)]:
            (root / (name + '.' + suffix)).write_bytes(b''.join(struct.pack('<f', v) for v in values))
    manifest.append(record)


literal = bytes([128, 64, 32, 129, 0, 0, 0, 0, 255, 1, 20, 140, 2, 3, 200, 96])
write('exponents-10x1', 10, 1, b''.join(bytes([128, 64, 32, e]) for e in [0, 1, 96, 97, 128, 129, 159, 160, 200, 255]))
write('literal-2x2', 2, 2, literal)
write('calibrated-2x2', 2, 2, literal, 'EXPOSURE=2\nEXPOSURE=3\nCOLORCORR=2 1 .5\nCOLORCORR=.5 2 4\n# already adjusted stored values\n')
write('explicit-rec709', 2, 2, literal, 'PRIMARIES=.64 .33 .30 .60 .15 .06 .3127 .3290\n')
write('near-rec709-unresolved', 2, 2, literal, 'PRIMARIES=.64004 .33 .30 .60 .15 .06 .3127 .3290\n')
write('rounded-ee-unresolved', 2, 2, literal, 'PRIMARIES=.64 .33 .29 .60 .15 .06 .3333 .3333\n')
write('legacy-repeat-8', 8, 1, bytes([128, 64, 32, 129, 1, 1, 1, 7]))
write('legacy-zero-low-digit-257', 257, 1, bytes([128, 64, 32, 129, 1, 1, 1, 0, 1, 1, 1, 1]))
first = bytes([2, 2, 0, 8, 136, 128, 136, 64, 136, 32, 136, 129])
second = bytes([2, 2, 0, 8]) + bytes([8]) + bytes([128] * 8) + bytes([8]) + bytes([64] * 8) + bytes([8]) + bytes([32] * 8) + bytes([8, 0, 1, 96, 97, 128, 160, 200, 255])
write('modern-rle-8x2', 8, 2, first + second)
write('wrong-modern-width', 8, 1, bytes([2, 2, 0, 16]) + first[4:], error='CodecFailed')
write('zero-modern-run', 8, 1, first[:4] + bytes([0]) + first[4:], error='CodecFailed')
write('oversized-modern-run', 8, 1, first[:4] + bytes([137]) + first[5:], error='CodecFailed')
write('truncated-modern-literal', 8, 1, first[:4] + bytes([8, 1, 2, 3, 4, 5, 6, 7]), error='CodecFailed')
write('overflow-legacy-zero-chain', 2, 1, bytes([128, 64, 32, 129]) + bytes([1, 1, 1, 0]) * 10, error='CapabilityUnsupported')
write('legacy-marker-collision', 1, 1, bytes([2, 2, 0, 129]), error='CapabilityUnsupported')
write('nonstandard-orientation', 1, 1, bytes([128, 64, 32, 129]), resolution='+Y 1 +X 1', error='CapabilityUnsupported')
(root / 'manifest.json').write_text(json.dumps(manifest, indent=4) + '\n')
print(f'Wrote {len(manifest)} owned Radiance packet/header fixtures')
