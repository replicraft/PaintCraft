"""Independent PCND7 persistence wire specimens; stdlib only, no product codec."""
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
# Existing independent PCND1 fixture supplies IDs, title, layer and literal RGBA.
PARENT = HERE.parent / 'pcnd-filter-fixtures/legacy-v1.pcnd'
# Source-only candidate verification can select the unchanged tracked parent.
if not PARENT.exists():
    PARENT = Path.cwd() / 'crates/services/paintcraft-formats/tests/pcnd-filter-fixtures/legacy-v1.pcnd'
BASE = PARENT.read_bytes()

def u(n):
    out = bytearray()
    while n >= 128:
        out.append((n & 127) | 128)
        n >>= 7
    return bytes(out) + bytes([n])

def s(n):
    return u(2*n if n >= 0 else -2*n-1)

def mask(bands=(), enabled=1, codec=1):
    out = bytes([1, enabled, codec]) + b''.join(s(n) for n in [-2,-1,4,2]) + u(len(bands))
    for y,height,runs in bands:
        out += u(y)+u(height)+u(len(runs))
        for x,width,value in runs:
            out += u(x)+u(width)+bytes([value])
    return out

BANDS = [(0,1,[(0,2,128),(3,3,255)]),(2,1,[(2,2,64)])]
# PCND1 fixed prefix: IDs[32], canvas2x1, no title, one layer.
assert BASE[43] == 1 and BASE[76:80] == bytes([1,0,0,8])
# v7 adds catalog prelude immediately after magic, source flag before layer count,
# filter stack before own mask, and preserves all existing raw tile bytes.
def wire(own, mixed=False, version=7, empty_manifest=None):
    profile = b'invert-rgb-v1'; codec = b'params-v1'
    entries = u(1)+u(len(profile))+profile+u(len(codec))+codec if mixed else u(0)
    if empty_manifest is not None:
        entries = empty_manifest
    manifest = u(len(entries))+entries
    filters = u(1)+(0).to_bytes(16,'big')+bytes([1,0,0])+mask(BANDS) if mixed else u(0)
    body = BASE[:4]+bytes([version])+BASE[5:8]+manifest+BASE[8:43]+b'\0'+BASE[43:76]+filters+own+BASE[76:-32]
    return body+hashlib.sha256(body).digest()

rows=[]
def add(name, own, accepted=True, mixed=False, **extra):
    data=wire(own,mixed,**extra)
    rows.append(dict(name=name,accepted=accepted,hex=data.hex(),sha256=hashlib.sha256(data).hexdigest()))
add('bands',mask(BANDS))
add('disabled',mask(BANDS,enabled=0))
add('zero',mask())
add('mixed',mask(BANDS),mixed=True)
# Every v7 layer has its own presence byte; the second layer has no mask.
first = wire(mask(BANDS))[:-32]
# Prefix includes magic8+empty prelude2+fixed document35+sourceflag1.
assert first[46] == 1
second = bytes([99])*16 + BASE[60:76] + b'\0\0' + BASE[76:-32]
body = first[:46] + b'\2' + first[47:] + second
data = body + hashlib.sha256(body).digest()
rows.append(dict(name='two-layers',accepted=True,hex=data.hex(),sha256=hashlib.sha256(data).hexdigest()))
add('absent-v7',b'\0',False)
add('bad-presence',b'\2',False)
add('bad-enabled',mask(enabled=2),False)
add('unknown-disabled-codec',mask(enabled=0,codec=9),False)
add('zero-coverage',mask([(0,1,[(0,1,0)])]),False)
add('overlap',mask([(0,1,[(0,3,1),(2,1,2)])]),False)
add('adjacent-equal',mask([(0,1,[(0,2,128),(2,1,128)])]),False)
add('out-domain',mask([(0,1,[(5,2,255)])]),False)
add('truncated',mask(BANDS)[:-1],False)
add('empty-catalog-v6',mask(),False,version=6)
value=dict(scope='Independent finite wire cases; not appearance or worker capability qualification',parent_sha256=hashlib.sha256(BASE).hexdigest(),domain=[-2,-1,4,2],fixtures=rows)
text=json.dumps(value,indent='\t')+'\n'
path=HERE/'oracle.json'
if '--check' in sys.argv:
    assert path.read_text()==text
else:
    path.write_text(text)
