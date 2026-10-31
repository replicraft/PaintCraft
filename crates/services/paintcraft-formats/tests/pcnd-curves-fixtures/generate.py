#!/usr/bin/env python3
"""Independent proposed Curves PCND6 specimens; no product imports or rendering."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import uuid

HERE = Path(__file__).resolve().parent
PROFILE = 'curves-rgb-v1'
RAW = bytes([10, 11, 128, 0, 20, 115, 200, 128])
IDENTITY = [0, 64, 128, 192, 255]
DOC = '33333333-3333-4333-8333-333333333333'
STATE = '11111111-1111-4111-8111-111111111111'
LAYER = '22222222-2222-4222-8222-222222222222'


def u(n):
    assert type(n) is int and n >= 0
    out = bytearray()
    while n >= 128:
        out.append((n & 127) | 128)
        n >>= 7
    return bytes(out + bytes([n]))


def signed(n):
    return u(2 * n if n >= 0 else -2 * n - 1)


def text(s):
    b = s.encode('ascii')
    return u(len(b)) + b


def mask():
    return dict(enabled=True, domain=[-1, 0, 2, 1],
                bands=[dict(y0=0, y1=1, runs=[dict(x0=-1, x1=1, coverage=128),
                                                dict(x0=1, x1=2, coverage=255)])])


def mask_bytes(m):
    if m is None:
        return b'\0'
    x, y, _, _ = m['domain']
    out = bytes([1, m['enabled'], 1]) + b''.join(signed(v) for v in m['domain'])
    out += u(len(m['bands']))
    for b in m['bands']:
        out += u(b['y0'] - y) + u(b['y1'] - b['y0']) + u(len(b['runs']))
        for r in b['runs']:
            out += u(r['x0'] - x) + u(r['x1'] - r['x0']) + bytes([r['coverage']])
    return out


def mask_digest(m):
    out = b'paintcraft.a8-mask/v1\0' + struct.pack('>qqqqQ', *m['domain'], len(m['bands']))
    for b in m['bands']:
        out += struct.pack('>qqQ', b['y0'], b['y1'], len(b['runs']))
        for r in b['runs']:
            out += struct.pack('>qqB', r['x0'], r['x1'], r['coverage'])
    return hashlib.sha256(out).digest()


def op(values=IDENTITY, enabled=True, profile=PROFILE, codec='params-v1', coverage=None):
    return dict(profile=profile, codec=codec, parameters=list(values), enabled=enabled, mask=coverage)


def fixture(ops):
    pairs = sorted({(o['profile'], o['codec']) for o in ops})
    prelude = u(len(pairs)) + b''.join(text(p) + text(c) for p, c in pairs)
    out = b'PCND\x06\r\n\x1a' + u(len(prelude)) + prelude
    out += uuid.UUID(DOC).bytes + uuid.UUID(STATE).bytes + u(2) + u(1) + b'\0\0'
    out += u(1) + uuid.UUID(LAYER).bytes + text('x') + b'\x01\xff'
    out += signed(0) + signed(0) + u(2) + u(1) + bytes(4) + u(len(ops))
    semantic = b'paintcraft.filter-stack/v3\0' + struct.pack('>H', len(ops))
    for i, o in enumerate(ops):
        params = bytes(o['parameters'])
        ident = (i + 1).to_bytes(16, 'big')
        out += ident + bytes([o['enabled']]) + u(pairs.index((o['profile'], o['codec'])))
        out += u(len(params)) + params + mask_bytes(o['mask'])
        name = o['profile'].encode('ascii')
        semantic += ident + bytes([o['enabled']]) + struct.pack('>H', len(name)) + name
        semantic += struct.pack('>I', len(params)) + params + bytes([o['mask'] is not None])
        if o['mask'] is not None:
            semantic += bytes([o['mask']['enabled']]) + mask_digest(o['mask'])
    out += b'\x01\0\0' + u(len(RAW)) + RAW + b'\0'
    return out + hashlib.sha256(out).digest(), hashlib.sha256(semantic).hexdigest()


def generate():
    rows = []
    def add(name, operations, error=None, corrupt=False):
        data, digest = fixture(operations)
        if corrupt:
            data = data[:-1] + bytes([data[-1] ^ 1])
        row = dict(name=name, expected='accept-after-codec-integration' if error is None else error,
                   operations=operations, hex=data.hex(), bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
        if error is None:
            row['stack_digest_v3'] = digest
        rows.append(row)
    add('identity', [op()])
    add('free-endpoints', [op([23, 9, 240, 73, 201])])
    add('nonmonotone', [op([0, 255, 0, 255, 0])])
    add('constant-zero', [op([0] * 5)])
    add('constant-full', [op([255] * 5)])
    add('disabled-identity', [op(enabled=False)])
    add('disabled-nonmonotone', [op([255, 0, 255, 0, 255], enabled=False)])
    add('fractional-mask', [op([0, 8, 251, 9, 243], coverage=mask())])
    add('mixed-old-profiles', [op([], profile='invert-rgb-v1'), op(),
                              op([0, 255, 0, 100, 0, 255], profile='levels-rgb-v1'),
                              op([0, 0], profile='exposure-srgb-v1', enabled=False)])
    for n in [4, 6]:
        add(f'length-{n}', [op([0] * n)], 'CodecFailed')
    add('unknown-codec-disabled', [op(enabled=False, codec='params-v2')], 'CapabilityUnsupported')
    add('unknown-profile-disabled', [op(enabled=False, profile='curves-rgb-v2')], 'CapabilityUnsupported')
    add('corrupt-inner-sha256', [op()], 'CodecFailed', corrupt=True)
    return dict(status='PLANNED_SPECIMENS_NOT_READER_QUALIFICATION',
                recipe='Standalone stdlib byte writer from PCND6 contract; no product serializer, evaluator or prepared LUT.',
                document_id=DOC, state_id=STATE, layer_id=LAYER, raw_layer_rgba=list(RAW), fixtures=rows)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    value = json.dumps(generate(), indent=2) + '\n'
    target = HERE / 'oracle.json'
    if args.check:
        assert target.read_text() == value, 'fixture drift'
        print('14 independent specimens verified: 9 planned positive, 5 negative; no reader execution')
    else:
        target.write_text(value)
