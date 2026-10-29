#!/usr/bin/env python3
"""Independent PCND2 source attachment serializer; no Rust codec calls."""
import hashlib
import json
from pathlib import Path
import struct

from pcnd_reference import ROOT, canonical, specimen


def attachment(profile=None, *, resolution=True, orientation=6):
    header = bytearray(192)
    header[:8] = b'PCSMETA1'
    struct.pack_into('>H', header, 8, 1)
    header[10] = 5  # TIFF source, RGB samples; provenance never activates ICC.
    header[11] = 1
    source = b'Independent PCND2 original-source placeholder'
    struct.pack_into('>Q', header, 12, len(source))
    header[20:52] = hashlib.sha256(source).digest()
    struct.pack_into('>IIIII', header, 52, 7, 3, 2, 2 if orientation >= 5 else 3, 3 if orientation >= 5 else 2)
    header[72:74] = bytes([orientation, 1])
    if resolution:
        header[74:76] = bytes([1, 1])  # Inch, exact rationals and raw TIFF unit2.
        struct.pack_into('>qQqQ', header, 76, 30000, 100, -3780, 1)
        header[108] = 1
        struct.pack_into('>I', header, 184, 2)
    header[110] = 2
    if profile is not None:
        header[109] = 1
        struct.pack_into('>Q', header, 112, len(profile))
        header[120:152] = hashlib.sha256(profile).digest()
    payload = profile or b''
    header[152:184] = hashlib.sha256(header + payload).digest()
    return bytes(header) + payload


def fixtures():
    profile = (ROOT / 'assets/profiles/sRGB2014.icc').read_bytes()
    document = dict(document_id='33333333-3333-4333-8333-333333333333',
                    state_id='11111111-1111-4111-8111-111111111111',
                    canvas=(2, 3), title='Independent source attachment', layers=[])
    result = {'rgb-profile.pcnd': canonical(dict(document, source_metadata=attachment(profile))),
              'resolution-only.pcnd': canonical(dict(document, source_metadata=attachment())),
              'orientation-only.pcnd': canonical(dict(document, source_metadata=attachment(resolution=False))),
              'legacy-v1.pcnd': specimen()}
    return result


def write():
    destination = ROOT / 'crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures'
    destination.mkdir(parents=True, exist_ok=True)
    records = []
    for name, data in fixtures().items():
        (destination / name).write_bytes(data)
        records.append(dict(file=name, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))
    (destination / 'oracle.json').write_text(json.dumps(dict(
        schema='paintcraft.pcnd2-reference/v1',
        author='Independent Python serialization, not Rust codec output',
        profile_sha256=hashlib.sha256((ROOT / 'assets/profiles/sRGB2014.icc').read_bytes()).hexdigest(),
        fixtures=records), indent=2) + '\n')


if __name__ == '__main__':
    write()
