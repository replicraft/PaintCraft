"""Independent PCND5 spatial descriptor preservation; Python stdlib byte writer.

No PaintCraft codecs or evaluator imports. Existing PCND1/2/3/4 fixtures are read
only. This set qualifies stored source/wire facts, not spatial rendering.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import uuid

HERE = Path(__file__).resolve().parent
DOCUMENT = '33333333-3333-4333-8333-333333333333'
STATE = '11111111-1111-4111-8111-111111111111'
LAYER = '22222222-2222-4222-8222-222222222222'
RAW = [255, 0, 0, 255, 7, 211, 93, 0]
KINDS = {1: 'invert-rgb-v1', 2: 'brightness-contrast-v1', 3: 'black-white-rgb-v1', 4: 'threshold-luma-v1', 5: 'posterize-rgb-v1', 6: 'solarize-rgb-v1', 7: 'box-blur-v1'}


def u(value):
    assert value >= 0
    output = bytearray()
    while value >= 128:
        output.append((value & 127) | 128)
        value >>= 7
    return bytes(output + bytes([value]))


def signed(value):
    return u(2 * value if value >= 0 else -2 * value - 1)


def ident(value):
    return uuid.UUID(value).bytes


def seal(body):
    return body + hashlib.sha256(body).digest()


def original_attachment():
    parent = HERE.parent / 'pcnd-metadata-fixtures/rgb-profile.pcnd'
    data = parent.read_bytes()
    assert data[4] == 2 and hashlib.sha256(data[:-32]).digest() == data[-32:]
    pos = 40

    def number():
        nonlocal pos
        result = shift = 0
        while True:
            byte = data[pos]
            pos += 1
            result |= (byte & 127) << shift
            if byte < 128:
                return result
            shift += 7

    number()
    number()
    flag = data[pos]
    pos += 1
    if flag:
        length = number()
        pos += length
    assert data[pos:pos + 8] == b'PCSMETA1'
    length = int.from_bytes(data[pos + 112:pos + 120], 'big')
    return data[pos:pos + 192 + length], hashlib.sha256(data).hexdigest()


ATTACHMENT, ATTACHMENT_PARENT_SHA = original_attachment()


def mask(coverage=128, enabled=True, codec=1, zero=False):
    return {'enabled': bool(enabled), 'codec': codec, 'domain': [-2, -1, 4, 2], 'bands': [] if zero else [{'y0': 0, 'y1': 1, 'runs': [{'x0': 0, 'x1': 1, 'coverage': coverage}]}]}


def mask_wire(value):
    if value is None:
        return b'\0'
    left, top, right, bottom = value['domain']
    output = bytes([1, value['enabled'], value['codec']]) + b''.join(map(signed, [left, top, right, bottom])) + u(len(value['bands']))
    for band in value['bands']:
        output += u(band['y0'] - top) + u(band['y1'] - band['y0']) + u(len(band['runs']))
        for run in band['runs']:
            output += u(run['x0'] - left) + u(run['x1'] - run['x0']) + bytes([run['coverage']])
    return output


def instance(tag=7, parameter=2, enabled=1, coverage=None):
    return {'tag': tag, 'parameter': parameter, 'enabled': enabled, 'mask': coverage}


def descriptor(operation):
    tag, parameter = operation['tag'], operation['parameter']
    out = {'kind': KINDS[tag]}
    if tag == 2:
        out.update(brightness=parameter[0], contrast=parameter[1])
    elif tag in [3, 4, 5, 7]:
        out[{3: 'strength', 4: 'level', 5: 'levels', 7: 'radius'}[tag]] = parameter
    return out


def record(index, operation, version):
    output = (index + 1).to_bytes(16, 'big') + bytes([operation['enabled'], operation['tag']])
    if operation['tag'] == 2:
        output += b''.join(map(signed, operation['parameter']))
    elif operation['tag'] in [3, 4, 5, 7]:
        output += bytes([operation['parameter']])
    if version >= 4:
        output += mask_wire(operation['mask'])
    return output


def file(operations, version=5, source=False, raw=None, origin=(0, 0)):
    body = b'PCND' + bytes([version]) + b'\r\n\x1a' + ident(DOCUMENT) + ident(STATE)
    body += u(2) + u(1) + b'\0' + bytes([source])
    if source:
        body += ATTACHMENT
    name = b'pcnd5'
    body += b'\x01' + ident(LAYER) + u(len(name)) + name + b'\x01\xff'
    body += b''.join(map(signed, origin)) + b'\x02\x01' + bytes(4)
    body += u(len(operations)) + b''.join(record(i, operation, version) for i, operation in enumerate(operations))
    pixels = bytes(RAW if raw is None else raw)
    body += b'\x01\0\0' + u(len(pixels)) + pixels + b'\0'
    return seal(body)


def mask_identity(value):
    if value is None:
        return None
    field = b'paintcraft.a8-mask/v1\0' + struct.pack('>qqqqQ', *value['domain'], len(value['bands']))
    for band in value['bands']:
        field += struct.pack('>qqQ', band['y0'], band['y1'], len(band['runs']))
        for run in band['runs']:
            field += struct.pack('>qqB', run['x0'], run['x1'], run['coverage'])
    return hashlib.sha256(field).hexdigest()


def stack_identity(operations):
    masked = any(operation['mask'] is not None for operation in operations)
    body = b'paintcraft.filter-stack/v' + (b'2' if masked else b'1') + b'\0' + struct.pack('>H', len(operations))
    for index, operation in enumerate(operations):
        kind = KINDS[operation['tag']].encode()
        body += (index + 1).to_bytes(16, 'big') + bytes([operation['enabled']]) + struct.pack('>H', len(kind)) + kind
        if operation['tag'] == 2:
            body += struct.pack('>hh', *operation['parameter'])
        elif operation['tag'] in [3, 4, 5, 7]:
            body += bytes([operation['parameter']])
        if masked:
            value = operation['mask']
            body += b'\0' if value is None else bytes([1, value['enabled']]) + bytes.fromhex(mask_identity(value))
    return hashlib.sha256(body).hexdigest()


def specimens():
    rows, files = [], {}

    def add(name, operations, accepted=True, code=None, **options):
        data = file(operations, **options)
        entry = {'file': name, 'accepted': accepted, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
        if accepted:
            entry.update(version=options.get('version', 5), raw_rgba=options.get('raw', RAW), origin=list(options.get('origin', (0, 0))), attachment=options.get('source', False), stack_sha256=stack_identity(operations), filters=[{'id': str(uuid.UUID(int=index + 1)), 'enabled': bool(operation['enabled']), 'descriptor': descriptor(operation), 'mask': operation['mask'], 'mask_sha256': mask_identity(operation['mask'])} for index, operation in enumerate(operations)])
        else:
            entry['code'] = code
        rows.append(entry)
        files[name] = data

    for radius in [0, 2, 32]:
        add(f'box-r{radius}.pcnd', [instance(parameter=radius)])
    for radius in [0, 2]:
        add(f'box-disabled-r{radius}.pcnd', [instance(parameter=radius, enabled=0)])
    add('box-half-mask.pcnd', [instance(coverage=mask())])
    add('box-zero-mask.pcnd', [instance(coverage=mask(zero=True))])
    add('box-disabled-mask.pcnd', [instance(coverage=mask(enabled=False))])
    add('point-box-point.pcnd', [instance(1, None), instance(), instance(6, None)])
    add('point-disabled-box-r0.pcnd', [instance(2, (-19, 23)), instance(3, 50), instance(parameter=0, enabled=0)])
    add('source-box.pcnd', [instance()], source=True)
    add('source-box-mask.pcnd', [instance(coverage=mask())], source=True)
    add('hidden-source-box.pcnd', [instance(parameter=32)], raw=[255, 0, 0, 0, 7, 211, 93, 0])
    add('two-box-shared-wire-masks.pcnd', [instance(coverage=mask()), instance(parameter=32, enabled=0, coverage=mask())])
    add('point-mask-disabled-box.pcnd', [instance(1, None, coverage=mask()), instance(parameter=0, enabled=0)])
    add('box-negative-origin.pcnd', [instance()], origin=(-5, 2))
    for radius in [33, 255]:
        add(f'bad-radius{radius}.pcnd', [instance(parameter=radius)], False, 'codec.failed')
        add(f'bad-disabled-radius{radius}.pcnd', [instance(parameter=radius, enabled=0)], False, 'codec.failed')
    for enabled in [0, 1]:
        add(f'unknown8-enabled{enabled}.pcnd', [instance(8, None, enabled)], False, 'capability.unsupported')
        for version in [3, 4]:
            add(f'old-v{version}-box-enabled{enabled}.pcnd', [instance(enabled=enabled, coverage=mask() if version == 4 else None)], False, 'capability.unsupported', version=version)
    add('v5-no-box.pcnd', [instance(1, None)], False, 'codec.failed')
    add('v5-mask-no-box.pcnd', [instance(1, None, coverage=mask())], False, 'codec.failed')
    add('v5-empty.pcnd', [], False, 'codec.failed')
    add('future-v6.pcnd', [instance()], False, 'capability.unsupported', version=6)
    add('disabled-unknown-mask-codec.pcnd', [instance(enabled=0, coverage=mask(enabled=False, codec=9))], False, 'capability.unsupported')
    add('bad-enabled2.pcnd', [instance(enabled=2)], False, 'codec.failed')
    # Seal a valid-body trailing byte; malformed integrity is a distinct control.
    data = file([instance()])
    for name, mutated in [('trailing.pcnd', seal(data[:-32] + b'\0')), ('bad-sha.pcnd', data[:-1] + bytes([data[-1] ^ 1]))]:
        rows.append({'file': name, 'accepted': False, 'code': 'codec.failed', 'bytes': len(mutated), 'sha256': hashlib.sha256(mutated).hexdigest()})
        files[name] = mutated
    value = {'schema': 'paintcraft.independent-pcnd5/v1', 'generator': __doc__, 'document': DOCUMENT, 'state': STATE, 'layer': LAYER, 'width': 2, 'height': 1, 'attachment_parent_sha256': ATTACHMENT_PARENT_SHA, 'attachment_digest': ATTACHMENT[152:184].hex(), 'profile_sha256': hashlib.sha256(ATTACHMENT[192:]).hexdigest(), 'source_header_hex': ATTACHMENT[:192].hex(), 'fixtures': rows}
    files['oracle.json'] = (json.dumps(value, indent=2) + '\n').encode()
    return files, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    files, rows = specimens()
    for name, content in files.items():
        if args.write:
            (HERE / name).write_bytes(content)
        else:
            assert (HERE / name).read_bytes() == content, f'{name} differs'
    print(json.dumps({'status': 'written' if args.write else 'verified', 'specimens': len(rows), 'valid': sum(row['accepted'] for row in rows), 'invalid': sum(not row['accepted'] for row in rows), 'manifest_sha256': hashlib.sha256(files['oracle.json']).hexdigest()}, sort_keys=True))


if __name__ == '__main__':
    main()
