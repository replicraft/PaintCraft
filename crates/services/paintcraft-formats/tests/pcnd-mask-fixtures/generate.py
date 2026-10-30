"""Independent PCND4 A8-band wire specimens; Python stdlib, no product encoder."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BASE = (ROOT.parent / 'pcnd-filter-fixtures/legacy-v1.pcnd').read_bytes()
# Fixed independently authored IDs/title/layer fields from the PCND3 fixture.
PREFIX = BASE[:8] + BASE[8:43] + b'\0' + BASE[43:76]
assert BASE[43] == 1 and BASE[76:80] == b'\x01\0\0\x08'
RAW_TILES = BASE[76:-32]


def u(n):
    out = bytearray()
    while n >= 128:
        out.append((n & 127) | 128)
        n >>= 7
    out.append(n)
    return bytes(out)


def s(n):
    return u(2*n if n >= 0 else -2*n-1)


def mask(bands, domain=(-2,-1,4,2), enabled=1, codec=1):
    out = bytes([1, enabled, codec]) + b''.join(map(s, domain)) + u(len(bands))
    for y, height, runs in bands:
        out += u(y) + u(height) + u(len(runs))
        for x, width, value in runs:
            out += u(x) + u(width) + bytes([value])
    return out


def instance(identity, payload, enabled=1):
    return identity.to_bytes(16,'big') + bytes([enabled,1]) + payload


def file(instances, version=4):
    body = PREFIX[:4] + bytes([version]) + PREFIX[5:] + u(len(instances)) + b''.join(instances) + RAW_TILES
    return body + hashlib.sha256(body).digest()


VALID = [(0,1,[(0,2,128),(3,3,255)]),(2,1,[(2,2,64)])]
rows = {}
def add(name, payloads, accepted=False, **facts):
    rows[name] = (file(payloads), accepted, facts)

add('bands.pcnd',[instance(0,mask(VALID))],True,bands=2,runs=3)
parent=(ROOT.parent/'pcnd-metadata-fixtures/rgb-profile.pcnd').read_bytes()
assert parent[73:81]==b'PCSMETA1'
attachment=parent[73:-33]
source=rows['bands.pcnd'][0][:-32]
source=source[:43]+b'\x01'+attachment+source[44:]
rows['source-bands.pcnd']=(source+hashlib.sha256(source).digest(),True,dict(bands=2,runs=3,source_parent_sha256=hashlib.sha256(parent).hexdigest()))
# Independent visible-pixel plane: half-strength invert on red, green untouched.
plane=file([instance(0,mask([(0,1,[(0,1,128)])],domain=(0,0,2,1)))])
body=plane[:-32];assert body[-9:-1]==bytes([17,32,255,0,0,127,200,255])
body=body[:-9]+bytes([255,0,0,255,0,255,0,255])+body[-1:]
rows['opaque-half.pcnd']=(body+hashlib.sha256(body).digest(),True,dict(bands=1,runs=1,rgba=[255,0,0,255,0,255,0,255],rendered_rgba=[127,128,128,255,0,255,0,255]))
add('disabled.pcnd',[instance(0,mask(VALID,enabled=0),enabled=0)],True,bands=2,runs=3)
add('zero.pcnd',[instance(0,mask([]))],True,bands=0,runs=0)
add('mixed.pcnd',[instance(0,b'\0'),instance(1,mask(VALID))],True,bands=2,runs=3)
add('repeated.pcnd',[instance(i,mask(VALID)) for i in range(3)],True,bands=6,runs=9)
add('unknown-disabled.pcnd',[instance(0,mask([],enabled=0,codec=9),enabled=0)],code='capability.unsupported')
add('bad-presence.pcnd',[instance(0,b'\x02')])
add('bad-enabled.pcnd',[instance(0,mask([],enabled=2))])
add('empty-v4.pcnd',[instance(0,b'\0')])
add('bad-domain.pcnd',[instance(0,mask([],domain=(4,0,4,1)))])
add('out-domain.pcnd',[instance(0,mask([(0,1,[(5,2,1)])]))])
add('overlap-runs.pcnd',[instance(0,mask([(0,1,[(0,3,1),(2,1,2)])]))])
add('merge-runs.pcnd',[instance(0,mask([(0,1,[(0,2,128),(2,2,128)])]))])
add('zero-coverage.pcnd',[instance(0,mask([(0,1,[(0,2,0)])]))])
add('zero-width.pcnd',[instance(0,mask([(0,1,[(0,0,1)])]))])
add('zero-height.pcnd',[instance(0,mask([(0,0,[(0,1,1)])]))])
add('empty-band.pcnd',[instance(0,mask([(0,1,[])]))])
add('overlap-bands.pcnd',[instance(0,mask([(0,2,[(0,1,1)]),(1,1,[(0,1,2)])]))])
add('merge-bands.pcnd',[instance(0,mask([(0,1,[(0,1,1)]),(1,1,[(0,1,1)])]))])
add('noncanonical-count.pcnd',[instance(0,mask([])[:-1]+b'\x80\0')])
add('truncated.pcnd',[instance(0,mask(VALID)[:-1])])
add('future-v5.pcnd',[instance(0,mask(VALID))])
old, accepted, facts = rows['future-v5.pcnd']
body=old[:-32];body=body[:4]+b'\x05'+body[5:]
rows['future-v5.pcnd']=(body+hashlib.sha256(body).digest(),False,{'code':'capability.unsupported'})
manifest=[]
for name,(data,accepted,facts) in rows.items():
    (ROOT/name).write_bytes(data)
    manifest.append(dict(file=name,accepted=accepted,bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),**facts))
(ROOT/'oracle.json').write_text(json.dumps({'generator':__doc__,'parent_sha256':hashlib.sha256(BASE).hexdigest(),'domain':[-2,-1,4,2],'samples':[[ -2,-1,128],[0,-1,0],[1,-1,255],[0,1,64],[4,1,0]],'fixtures':manifest},indent=2)+'\n')
