"""Independent raw PCND2 opaque source for actual MSP privacy UI qualification."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / 'scripts'))
from pcnd_reference import canonical

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('output', type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
parent = ROOT / 'crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures/rgb-profile.pcnd'
raw = parent.read_bytes()
at = raw.index(b'PCSMETA1')
size = 192 + struct.unpack_from('>Q', raw, at + 112)[0]
attachment = raw[at:at + size]
pixels = {(x, y): ((255, 255, 255, 255) if (x + y) % 2 else (0, 0, 0, 255))
          for y in range(4) for x in range(4)}
doc = dict(document_id='73333333-3333-4333-8333-333333333333',
           state_id='71111111-1111-4111-8111-111111111111', canvas=(4, 4),
           title='Opaque source metadata copy', source_metadata=attachment,
           layers=[dict(id='72222222-2222-4222-8222-222222222222', name='Bilevel',
                        visible=True, opacity=255, origin=(0, 0), width=4, height=4,
                        default=(0, 0, 0, 255), pixels=pixels)])
data = canonical(doc)
rgba = bytes(v for y in range(4) for x in range(4) for v in pixels[x, y])
for name, value in [('bilevel.pcnd', data), ('bilevel.rgba', rgba)]:
    with (args.output / name).open('xb') as stream:
        stream.write(value)
receipt = dict(schema='paintcraft.advanced-metadata-browser-input/v1',
               producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               reference_writer_sha256=hashlib.sha256((ROOT / 'scripts/pcnd_reference.py').read_bytes()).hexdigest(),
               attachment_parent_sha256=hashlib.sha256(raw).hexdigest(),
               pcnd_sha256=hashlib.sha256(data).hexdigest(), rgba_sha256=hashlib.sha256(rgba).hexdigest(),
               recipe='Independent Python PCND2 serialization of 4x4 opaque checkerboard; exact original attachment copied from independently qualified PCND2. Current canvas is edited, original source dimensions remain provenance; no foreign source file or color transform claim.')
(args.output / 'input.json').write_text(json.dumps(receipt, indent=4) + '\n')
