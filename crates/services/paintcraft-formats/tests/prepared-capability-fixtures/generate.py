#!/usr/bin/env python3
"""Independent exact capability hash framing from the reviewed binary contract."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parent
REGISTRY = {
    'invert-rgb-v1': 'direct-rgba8-point-v1',
    'brightness-contrast-v1': 'direct-rgba8-point-v1',
    'black-white-rgb-v1': 'direct-rgba8-point-v1',
    'threshold-luma-v1': 'direct-rgba8-point-v1',
    'posterize-rgb-v1': 'direct-rgba8-point-v1',
    'solarize-rgb-v1': 'direct-rgba8-point-v1',
    'box-blur-v1': 'premultiplied-region-rgba8-v1',
    'levels-rgb-v1': 'prepared-levels-rgba8-v1',
    'exposure-srgb-v1': 'prepared-exposure-rgba8-v1',
}


def text(s):
    raw = s.encode('ascii')
    return struct.pack('>H', len(raw)) + raw


def row(names):
    names = sorted(set(names))
    required = b'paintcraft.worker-required/v1\0' + struct.pack('>H', len(names))
    resolution = b'paintcraft.worker-resolution/v1\0' + struct.pack('>HH', 1, len(names))
    for name in names:
        pair = text(name) + text('params-v1')
        required += pair
        resolution += pair + text(REGISTRY[name]) + struct.pack('>I', 1)
    return dict(profiles=names, required_sha256=hashlib.sha256(required).hexdigest(),
                resolution_sha256=hashlib.sha256(resolution).hexdigest())


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    data = dict(recipe='Independent stdlib binary hash framing. Exact reviewed domains and big-endian lengths; not Rust serialization.',
                rows=[row(x) for x in [[], ['invert-rgb-v1'], ['exposure-srgb-v1'],
                                      ['invert-rgb-v1', 'exposure-srgb-v1'], list(REGISTRY)]])
    encoded = json.dumps(data, indent=2) + '\n'
    path = ROOT / 'oracle.json'
    if args.check:
        assert path.read_text() == encoded
        print('5 independent capability hash subsets verified')
    else:
        path.write_text(encoded)
