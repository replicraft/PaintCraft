"""Independent PCND8 bytes, stdlib only; no product codec/kernel imports."""
import hashlib
import json
from pathlib import Path
import sys
import uuid

HERE = Path(__file__).resolve().parent
PARENT = HERE.parent / 'pcnd-filter-fixtures/legacy-v1.pcnd'
if not PARENT.exists():
    PARENT = Path.cwd() / 'crates/services/paintcraft-formats/tests/pcnd-filter-fixtures/legacy-v1.pcnd'
METADATA = PARENT.parent.parent / 'pcnd-metadata-fixtures/rgb-profile.pcnd'
LEGACY = PARENT.read_bytes()
META = METADATA.read_bytes()
assert META[73:81] == b'PCSMETA1'
attachment = META[73:-33]
assert len(attachment) == 192 + int.from_bytes(attachment[112:120], 'big')
DOC = uuid.UUID('33333333-3333-4333-8333-333333333333').bytes
STATE = uuid.UUID('11111111-1111-4111-8111-111111111111').bytes
LAYER = uuid.UUID('22222222-2222-4222-8222-222222222222').bytes
RAW = bytes([17,32,255,0,0,127,200,255])


def u(n):
    out = bytearray()
    while n >= 128:
        out.append((n & 127) | 128)
        n >>= 7
    return bytes(out) + bytes([n])


def signed(n):
    return u(2*n if n >= 0 else -2*n-1)


def own():
    # Some enabled canonical A8 uniform128 over finite [0,0,2,1].
    return bytes([1,1,1])+b''.join(signed(n) for n in [0,0,2,1])+u(1)+u(0)+u(1)+u(1)+u(0)+u(2)+bytes([128])


def manifest(profile=None, codec='params-v1'):
    entries = u(0) if profile is None else u(1)+u(len(profile))+profile.encode()+u(len(codec))+codec.encode()
    return u(len(entries))+entries


def layer(mode, ordinal=0, visible=1, opacity=255, filtered=False, masked=False, explicit_default=False):
    identity = LAYER if ordinal == 0 else bytes([ordinal])*16
    prefix = identity+u(5)+b'Image'+bytes([visible,opacity,mode])
    prefix += signed(0)+signed(0)+u(2)+u(1)
    default = bytes([1,2,3,0]) if explicit_default else bytes(4)
    pixels = default*2 if explicit_default else RAW
    filters = u(1)+bytes(16)+bytes([1])+u(0)+u(0)+bytes([0]) if filtered else u(0)
    tile = bytes([1])+u(0)+u(0)+u(len(pixels))+pixels+bytes([0])
    return prefix+default+filters+(own() if masked else bytes([0]))+tile


def wire(modes, source=False, filtered=False, codec='params-v1', root_count=None, **settings):
    body = b'PCND\x08\r\n\x1a'+manifest('invert-rgb-v1' if filtered else None,codec)+DOC+STATE
    body += u(2)+u(1)+bytes([0, int(source)])+(attachment if source else b'')
    body += u(len(modes)) if root_count is None else root_count
    for i, mode in enumerate(modes):
        body += layer(mode,i,filtered=filtered,**settings)
    return finish(body)


def finish(body):
    return body+hashlib.sha256(body).digest()


rows = []
def add(name, modes, accepted=True, **settings):
    data=wire(modes,**settings)
    rows.append(dict(name=name,accepted=accepted,modes=modes,hex=data.hex(),sha256=hashlib.sha256(data).hexdigest(),source=settings.get('source',False),filtered=settings.get('filtered',False),masked=settings.get('masked',False),canonical_writer_equal=not settings.get('explicit_default',False)))

add('multiply',[1])
add('screen',[2])
add('mixed',[0,1,2])
add('hidden-multiply',[1],visible=0)
add('zero-opacity-screen',[2],opacity=0)
add('source-multiply',[1],source=True)
add('filter-ownmask-screen',[2],filtered=True,masked=True)
add('explicit-default-record',[1],explicit_default=True)
add('all-normal-v8',[0],False)
add('unknown-mode3',[3],False)
add('unknown-invisible255',[255],False,visible=0)
add('unknown-codec',[1],False,filtered=True,codec='params-v9')
add('noncanonical-layer-count',[1],False,root_count=b'\x81\x00')
base=wire([1])
# Exact prefix stops after visibility/opacity, before mandatory blend byte.
mode_offset=8+2+32+2+2+1+16+1+5+2
assert base[mode_offset] == 1
truncated=finish(base[:mode_offset])
rows.append(dict(name='truncated-mode',accepted=False,hex=truncated.hex(),sha256=hashlib.sha256(truncated).hexdigest()))
bad=base[:-1]+bytes([base[-1]^1])
rows.append(dict(name='bad-sha',accepted=False,hex=bad.hex(),sha256=hashlib.sha256(bad).hexdigest()))
# Existing independent PCND1 bytes must remain byte-exact for allNormal.
rows.append(dict(name='legacy-normal-v1',accepted=True,modes=[0],hex=LEGACY.hex(),sha256=hashlib.sha256(LEGACY).hexdigest(),canonical_writer_equal=True))
for row in rows:
    if not row['accepted']:
        row['error'] = 'CapabilityUnsupported' if row['name']=='unknown-codec' else 'CodecFailed'
value=dict(scope='Independent PCND8 metadata/persistence specimens only; visual execution remains closed',legacy_sha256=hashlib.sha256(LEGACY).hexdigest(),metadata_parent_sha256=hashlib.sha256(META).hexdigest(),rawRGBA=list(RAW),fixtures=rows)
text=json.dumps(value,indent=2)+'\n'
path=HERE/'oracle.json'
if '--check' in sys.argv:
    assert path.read_text()==text
else:
    path.write_text(text)
print(hashlib.sha256(text.encode()).hexdigest())
