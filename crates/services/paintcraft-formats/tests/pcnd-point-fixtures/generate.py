"""Independent PCND3/4 algorithm-registry specimens, Python stdlib only."""
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import struct
import uuid

ROOT = Path(__file__).resolve().parent
BASE = (ROOT.parent / 'pcnd-filter-fixtures/legacy-v1.pcnd').read_bytes()
PARENT = (ROOT.parent / 'pcnd-filter-fixtures/source-stack.pcnd').read_bytes()
assert BASE[76:80] == bytes([1, 0, 0, 8])
assert PARENT[44:52] == b'PCSMETA1'
PROFILE_LENGTH = int.from_bytes(PARENT[156:164], 'big')
ATTACHMENT = PARENT[44:44 + 192 + PROFILE_LENGTH]
assert PARENT[44 + 192 + PROFILE_LENGTH] == 1
RGBA = [20, 115, 0, 255, 0, 178, 4, 255]
IDS = {1: 'invert-rgb-v1', 2: 'brightness-contrast-v1',
       3: 'black-white-rgb-v1', 4: 'threshold-luma-v1',
       5: 'posterize-rgb-v1', 6: 'solarize-rgb-v1'}


def identities(operations):
    masked = any(c is not None for _, _, _, c in operations)
    stack = b'paintcraft.filter-stack/v' + (b'2' if masked else b'1') + b'\0'
    stack += struct.pack('>H', len(operations))
    masks = []
    for index, (tag, parameter, enabled, coverage) in enumerate(operations):
        kind = IDS[tag].encode()
        stack += (index + 1).to_bytes(16, 'big') + bytes([enabled]) + struct.pack('>H', len(kind)) + kind
        if tag == 2:
            stack += struct.pack('>hh', *parameter)
        elif tag in [3, 4, 5]:
            stack += bytes([parameter])
        if coverage is None:
            masks.append(None)
            if masked:
                stack += b'\0'
        else:
            field = b'paintcraft.a8-mask/v1\0' + struct.pack('>qqqqQ', 0, 0, 2, 1, 1)
            field += struct.pack('>qqQqqB', 0, 1, 1, 0, 1, coverage)
            digest = hashlib.sha256(field).digest()
            masks.append(digest.hex())
            stack += b'\x01\x01' + digest
    digest = hashlib.sha256(stack).digest()
    inventory = b'paintcraft.filter-bake-witness/v1\0' + BASE[8:40]
    inventory += struct.pack('>IIQ', 2, 1, 1) + BASE[44:60] + digest
    inventory += b'\x01\xff' + struct.pack('>qqII', 0, 0, 2, 1) + bytes(4)
    return digest.hex(), hashlib.sha256(inventory).hexdigest(), masks


def u(value):
    out = bytearray()
    while value >= 128:
        out.append((value & 127) | 128)
        value >>= 7
    return bytes(out + bytes([value]))


def signed(value):
    return u(value * 2 if value >= 0 else -value * 2 - 1)


def nearest(value):
    value = max(Fraction(0), min(Fraction(255), value))
    return (2 * value.numerator + value.denominator) // (2 * value.denominator)


def candidate(pixel, tag, parameter):
    rgb = pixel[:3]
    luma = sum(Fraction(v) * w for v, w in zip(rgb, [Fraction(2126, 10000), Fraction(7152, 10000), Fraction(722, 10000)]))
    if tag == 1:
        result = [255 - c for c in rgb]
    elif tag == 2:
        b, c = parameter
        result = [nearest((Fraction(v) - Fraction(255, 2)) * Fraction(100 + c, 100 - c) + Fraction(255, 2) + Fraction(255 * b, 100)) for v in rgb]
    elif tag == 3:
        result = [nearest(Fraction(100 - parameter, 100) * v + Fraction(parameter, 100) * luma) for v in rgb]
    elif tag == 4:
        result = [255 if luma >= parameter else 0] * 3
    elif tag == 5:
        result = [nearest(Fraction(255 * nearest(Fraction(v * (parameter - 1), 255)), parameter - 1)) for v in rgb]
    elif tag == 6:
        result = [255 - v if v >= 128 else v for v in rgb]
    else:
        raise ValueError('Unknown reference algorithm')
    return result + pixel[3:]


def mask_record(coverage):
    # Full 2x1 declared domain, one run over the left source pixel only.
    return bytes([1, 1, 1]) + b''.join(map(signed, [0, 0, 2, 1])) + bytes([1, 0, 1, 1, 0, 1, coverage])


def instance(index, tag, parameter, enabled, coverage, masked):
    data = (index + 1).to_bytes(16, 'big') + bytes([enabled, tag])
    if tag == 2:
        data += b''.join(map(signed, parameter))
    elif tag in [3, 4, 5]:
        data += bytes([parameter])
    if masked:
        data += b'\0' if coverage is None else mask_record(coverage)
    return data


def file(operations, *, attachment=False, raw=None):
    rgba = RGBA if raw is None else raw
    masked = any(coverage is not None for _, _, _, coverage in operations)
    version = 4 if masked else 3
    body = BASE[:4] + bytes([version]) + BASE[5:43]
    body += bytes([attachment]) + (ATTACHMENT if attachment else b'') + BASE[43:76]
    body += u(len(operations)) + b''.join(instance(i, *op, masked) for i, op in enumerate(operations))
    body += bytes([1, 0, 0, 8]) + bytes(rgba) + b'\0'
    return body + hashlib.sha256(body).digest()


rows = []


def add(name, operations, *, attachment=False, raw=None):
    rgba = list(RGBA if raw is None else raw)
    data = file(operations, attachment=attachment, raw=rgba)
    result = list(rgba)
    descriptors = []
    for index, (tag, parameter, enabled, coverage) in enumerate(operations):
        descriptor = {'kind': IDS[tag]}
        if tag == 2:
            descriptor.update(brightness=parameter[0], contrast=parameter[1])
        elif tag in [3, 4, 5]:
            descriptor[{3: 'strength', 4: 'level', 5: 'levels'}[tag]] = parameter
        descriptors.append({'id': str(uuid.UUID(int=index + 1)), 'enabled': bool(enabled), 'descriptor': descriptor, 'coverage': coverage})
        if not enabled:
            continue
        for offset in [0, 4]:
            incoming = result[offset:offset + 4]
            output = candidate(incoming, tag, parameter)
            amount = 255 if coverage is None else (coverage if offset == 0 else 0)
            result[offset:offset + 3] = [nearest(Fraction(255 - amount, 255) * a + Fraction(amount, 255) * b) for a, b in zip(incoming, output)][:3]
    rendered = list(result)
    for offset in [0, 4]:
        if rendered[offset + 3] == 0:
            rendered[offset:offset + 3] = [0, 0, 0]
    (ROOT / name).write_bytes(data)
    stack, inventory, masks = identities(operations)
    count = sum(c is not None for _, _, _, c in operations)
    rows.append({'file': name, 'accepted': True, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'version': data[4], 'attachment': attachment, 'raw_rgba': rgba, 'evaluated_rgba': result, 'rendered_rgba': rendered, 'filters': descriptors, 'stack_sha256': stack, 'filter_inventory_sha256': inventory, 'mask_sha256': masks, 'mask_count': count, 'bands': count, 'runs': count})


for tag, values in [(3, [0, 50, 100]), (4, [0, 128, 255]), (5, [2, 6, 32]), (6, [None])]:
    for value in values:
        add(f'tag{tag}-{value}.pcnd', [(tag, value, 1, None)])
    default = {3: 100, 4: 128, 5: 6, 6: None}[tag]
    add(f'tag{tag}-disabled.pcnd', [(tag, default, 0, None)])
    add(f'tag{tag}-masked.pcnd', [(tag, default, 1, 128)])
add('bw-then-threshold.pcnd', [(3, 100, 1, None), (4, 128, 1, None)])
add('threshold-then-bw.pcnd', [(4, 128, 1, None), (3, 100, 1, None)])
six = [(1, None, 1, None), (2, (10, -7), 1, None), (3, 50, 1, None), (4, 128, 1, None), (5, 6, 1, None), (6, None, 1, None)]
add('all-six.pcnd', six)
add('source-all-six.pcnd', six, attachment=True)
add('hidden-bw.pcnd', [(3, 100, 1, None)], raw=RGBA[:3] + [0] + RGBA[4:])


def invalid(name, tag, parameter, enabled=1, code='codec.failed'):
    data = file([(tag, parameter, enabled, None)])
    (ROOT / name).write_bytes(data)
    rows.append({'file': name, 'accepted': False, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'code': code})


invalid('bad-strength101.pcnd', 3, 101)
invalid('bad-strength255.pcnd', 3, 255)
invalid('bad-levels1.pcnd', 5, 1)
invalid('bad-levels33.pcnd', 5, 33)
invalid('unknown-enabled.pcnd', 7, None, code='capability.unsupported')
invalid('unknown-disabled.pcnd', 7, None, enabled=0, code='capability.unsupported')
manifest = {'generator': __doc__, 'parent_sha256': hashlib.sha256(BASE).hexdigest(), 'attachment_parent_sha256': hashlib.sha256(PARENT).hexdigest(), 'arithmetic': 'Independent exact Fraction expressions; candidate rounded before per-filter coverage; hidden RGB evaluated, alpha unchanged.', 'document': str(uuid.UUID(bytes=BASE[8:24])), 'state': str(uuid.UUID(bytes=BASE[24:40])), 'layer': str(uuid.UUID(bytes=BASE[44:60])), 'width': 2, 'height': 1, 'profile_bytes': PROFILE_LENGTH, 'metadata_wire_bytes': len(ATTACHMENT), 'fixtures': rows}
(ROOT / 'oracle.json').write_text(json.dumps(manifest, indent=2) + '\n')
assert len(rows) == 29 and sum(r['accepted'] for r in rows) == 23
