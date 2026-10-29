"""Authored JPEGs and raw libjpeg-turbo reference pixels, no runtime dependency."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess

p = argparse.ArgumentParser()
p.add_argument('--reference', type=Path, required=True)
p.add_argument('--library', type=Path, required=True)
a = p.parse_args()
root = Path(__file__).resolve().parent
sha = lambda value: hashlib.sha256(value).hexdigest()


def exif(value, little):
    order = '<' if little else '>'
    tiff = (b'II' if little else b'MM') + struct.pack(order + 'HIH', 42, 8, 1)
    tiff += struct.pack(order + 'HHIHHI', 0x112, 3, 1, value, 0, 0)
    data = b'Exif\0\0' + tiff
    return b'\xff\xe1' + struct.pack('>H', len(data) + 2) + data


def oriented(raw, width, height, orientation):
    # Independent CIPA row/column placement, expressed as destination rows.
    pixels = [raw[i:i + 4] for i in range(0, len(raw), 4)]
    rows = [pixels[y * width:(y + 1) * width] for y in range(height)]
    if orientation == 2:
        rows = [row[::-1] for row in rows]
    elif orientation == 3:
        rows = [row[::-1] for row in rows[::-1]]
    elif orientation == 4:
        rows = rows[::-1]
    elif orientation == 5:
        rows = [list(row) for row in zip(*rows)]
    elif orientation == 6:
        rows = [list(row) for row in zip(*rows[::-1])]
    elif orientation == 7:
        rows = [list(row) for row in zip(*rows[::-1])][::-1]
    elif orientation == 8:
        rows = [list(row) for row in zip(*rows)][::-1]
    return b''.join(pixel for row in rows for pixel in row), len(rows[0]), len(rows)


# Authored non-square label rows pin all eight CIPA transforms independently.
labels = b''.join(bytes([value, 0, 0, 255]) for value in b'ABCDEF')
for orientation, expected in enumerate(('ABCDEF', 'CBAFED', 'FEDCBA', 'DEFABC',
                                         'ADBECF', 'DAEBFC', 'FCEBDA', 'CFBEAD'), 1):
    transformed, _, _ = oriented(labels, 3, 2, orientation)
    assert bytes(transformed[at] for at in range(0, len(transformed), 4)).decode() == expected


cases = []
for kind in ('gray', 'rgb', 'progressive', 'ycbcr', 'adobe-cmyk', 'unmarked-cmyk', 'adobe-ycck', 'rgb-numbered', 'adobe-ycck-color'):
    file = root / (kind + '.jpg')
    subprocess.run([a.reference, 'write', file, kind], check=True)
    info = json.loads(subprocess.check_output([a.reference, 'read', file]))
    components = bytes(info.pop('raw_samples'))
    (root / (kind + '.raw')).write_bytes(components)
    rgba = bytearray()
    for at in range(0, len(components), info['channels']):
        source = components[at:at + info['channels']]
        if info['channels'] == 4:
            c, m, y, k = source
            if not info['adobe']:
                c, m, y, k = 255 - c, 255 - m, 255 - y, 255 - k
            rgb = bytes((sample * k + 127) // 255 for sample in (c, m, y))
        elif info['channels'] == 1:
            rgb = source * 3
        else:
            rgb = source
        rgba.extend(rgb + b'\xff')
    (root / (kind + '.rgba')).write_bytes(rgba)
    base = dict(info, file=file.name, sha256=sha(file.read_bytes()),
                raw_file=kind + '.raw', raw_sha256=sha(components),
                rgba_file=kind + '.rgba', rgba_sha256=sha(rgba),
                orientation=1, expected_width=info['width'], expected_height=info['height'])
    cases.append(base)
    variants = [(value, endian) for value in range(1, 9) for endian in ('II', 'MM')] if kind in ('gray', 'rgb') else []
    if kind == 'progressive':
        variants = [(value, 'MM') for value in range(1, 9)]
    if kind in ('adobe-cmyk', 'unmarked-cmyk', 'adobe-ycck', 'rgb-numbered', 'adobe-ycck-color'):
        variants = [(6, 'II')]
    for value, endian in variants:
        name = f'{kind}-orientation-{value}-{endian}'
        encoded = file.read_bytes()
        result = encoded[:2] + exif(value, endian == 'II') + encoded[2:]
        (root / (name + '.jpg')).write_bytes(result)
        expected, width, height = oriented(rgba, info['width'], info['height'], value)
        (root / (name + '.rgba')).write_bytes(expected)
        cases.append(dict(base, file=name + '.jpg', sha256=sha(result), orientation=value,
                          rgba_file=name + '.rgba', rgba_sha256=sha(expected),
                          expected_width=width, expected_height=height))
manifest = {'provenance': {'reference_source_sha256': sha((root / 'reference.c').read_bytes()),
    'oracle_builder_source_sha256': sha(Path(__file__).read_bytes()),
    'reference_binary_sha256': sha(a.reference.read_bytes()),
    'libjpeg_turbo_version': '3.2.0', 'libjpeg_library_sha256': sha(a.library.read_bytes()),
    'ownership': 'Authored arrays and arithmetic blocks; no external artwork',
    'raw_api': 'jpeg_read_scanlines with explicit grayscale/RGB/CMYK output; no premultiplied surface',
    'orientation_reference': 'CIPA DC-X010-2017 row/column semantics, Python destination-row oracle',
    'color_policy': 'Explicit device-CMYK approximation, round integer channel product/255; invert conventional unmarked samples first'},
    'cases': cases}
(root / 'oracle.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(len(cases), 'independent JPEG cases')
