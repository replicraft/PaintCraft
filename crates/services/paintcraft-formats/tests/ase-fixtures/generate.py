"""Owned ASE 1.0 specimens, independent of every Rust codec."""
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).parent

def name(text):
    encoded = (text + '\0').encode('utf-16be')
    return struct.pack('>H', len(encoded) // 2) + encoded

def block(kind, payload=b''):
    return struct.pack('>HI', kind, len(payload)) + payload

def color(label, model, values, kind):
    return block(1, name(label) + model.encode().ljust(4, b' ')
                 + struct.pack('>' + 'f' * len(values), *values) + struct.pack('>H', kind))

blocks = [
    color('Red', 'RGB', [1, 0, 0], 0),
    block(0xc001, name('印刷 / Print')),
    color('赤 🚀', 'RGB', [0.25, 0.5, 1], 2),
    color('Cyan', 'CMYK', [1, 0, 0, 0.125], 1),
    color('Lab spot', 'LAB', [0.6, -35, -5], 1),
    color('Gray', 'Gray', [0.75], 2),
    block(0xc002),
    block(0xc001, name('Empty')),
    block(0xc002),
    color('Red', 'Gray', [-0.0], 0),
]
files = {
    'mixed.ase': b'ASEF' + struct.pack('>HHI', 1, 0, len(blocks)) + b''.join(blocks),
    'empty.ase': b'ASEF' + struct.pack('>HHI', 1, 0, 0),
}
manifest = {'generator': 'generate.py, Python standard library', 'license': 'MIT, repository-authored fixtures', 'files': {}}
for filename, data in files.items():
    (ROOT / filename).write_bytes(data)
    manifest['files'][filename] = {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
if (ROOT / 'swatch-generated.ase').exists():
    data = (ROOT / 'swatch-generated.ase').read_bytes()
    manifest['files']['swatch-generated.ase'] = {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest(), 'generator': 'swatch 0.4.0 via oracle.py --write-fixture'}
(ROOT / 'sources.json').write_text(json.dumps(manifest, indent=4) + '\n')
