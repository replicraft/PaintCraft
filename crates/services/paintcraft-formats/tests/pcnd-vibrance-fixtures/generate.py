#!/usr/bin/env python3
"""Independent proposed PCND6 Vibrance codec bytes, not a rendering oracle. Stdlib only."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import uuid

HERE = Path(__file__).resolve().parent
PROFILE = 'vibrance-rgb-v1'
RAW = [10, 11, 128, 0, 20, 115, 200, 128]
DOC = '33333333-3333-4333-8333-333333333333'
STATE = '11111111-1111-4111-8111-111111111111'
LAYER = '22222222-2222-4222-8222-222222222222'


def u(value):
    out = bytearray()
    while value >= 128:
        out.append((value & 127) | 128)
        value >>= 7
    out.append(value)
    return bytes(out)


def text(value):
    raw = value.encode('ascii')
    return u(len(raw)) + raw


def fixture(amount, enabled, params=None, codec='params-v1'):
    params = struct.pack('>h', amount) if params is None else params
    prelude = u(1) + text(PROFILE) + text(codec)
    body = b'PCND\x06\r\n\x1a' + u(len(prelude)) + prelude
    body += uuid.UUID(DOC).bytes + uuid.UUID(STATE).bytes
    body += u(2) + u(1) + b'\0\0'  # canvas, selection absent, metadata absent
    body += u(1) + uuid.UUID(LAYER).bytes + text('x') + b'\x01\xff'
    body += b'\0\0' + u(2) + u(1) + bytes(4) + u(1)
    body += (1).to_bytes(16, 'big') + bytes([enabled]) + u(0) + u(len(params)) + params + b'\0'
    body += b'\x01\0\0' + u(len(RAW)) + bytes(RAW) + b'\0'
    return body + hashlib.sha256(body).digest()


def semantic(amount, enabled):
    name = PROFILE.encode('ascii')
    body = b'paintcraft.filter-stack/v3\0' + struct.pack('>H', 1)
    body += (1).to_bytes(16, 'big') + bytes([enabled])
    body += struct.pack('>H', len(name)) + name + struct.pack('>Ih', 2, amount) + b'\0'
    return hashlib.sha256(body).hexdigest()


def generate():
    rows = []
    def add(name, amount, enabled=1, accepted=True, params=None, codec='params-v1'):
        data = fixture(amount, enabled, params, codec)
        row = dict(name=name, accepted=accepted, amount_hundredths=amount, enabled=bool(enabled),
                   hex=data.hex(), sha256=hashlib.sha256(data).hexdigest())
        if accepted:
            row['stack_digest_v3'] = semantic(amount, enabled)
        else:
            row['error'] = 'CapabilityUnsupported' if codec != 'params-v1' else 'CodecFailed'
        rows.append(row)
    for amount in [-100, -75, -1, 0, 1, 50, 100]:
        add(f'enabled-{amount}', amount)
    for amount in [-100, 0, 100]:
        add(f'disabled-{amount}', amount, 0)
    for amount in [-32768, -101, 101, 32767]:
        add(f'range-{amount}', amount, accepted=False)
    for params in [b'', b'\0', b'\0\0\0', b'\0\0\0\0']:
        add(f'length-{len(params)}', 0, accepted=False, params=params)
    add('unknown-codec-disabled', 0, 0, False, codec='params-v2')
    return dict(recipe='Independent literal PCND6 framing; signed i16BE Vibrance params; SHA256 footer and semantic hash computed with hashlib. No PaintCraft serializer or renderer.',
                document_id=DOC, state_id=STATE, layer_id=LAYER, raw_layer_rgba=RAW,
                fixtures=rows)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    output = json.dumps(generate(), indent=2) + '\n'
    path = HERE / 'oracle.json'
    if args.check:
        assert path.read_text() == output, 'fixture drift'
        print('19 independent proposed Vibrance fixtures verified (10 positive, 9 negative)')
    else:
        path.write_text(output)
