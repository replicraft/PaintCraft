"""Independent Normal PCND1-7 raw/RLE literals; no product imports or execution."""
from pathlib import Path
import hashlib
import json
HERE = Path(__file__).resolve().parent
# Fixed diagnostic location binds the actual repository root.
ROOT = HERE.parents[4]
DOC, STATE, LAYER = [bytes([n]) * 16 for n in [16, 17, 18]]
META = ROOT / 'crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures'

def u(n):
    assert n >= 0
    data = bytearray()
    while n >= 128:
        data.append((n & 127) | 128)
        n >>= 7
    return bytes(data) + bytes([n])

def signed(n): return u(2*n if n >= 0 else -2*n-1)
def text(s):
    data = s.encode('utf8')
    return u(len(data)) + data

def metadata(name):
    data = (META / name).read_bytes()
    start = data.index(b'PCSMETA1')
    header = data[start:start+192]
    length = int.from_bytes(header[112:120], 'big')
    return data[start:start+192+length]

def mask(value, enabled=True):
    body = bytes([1, int(enabled), 1]) + b''.join(signed(n) for n in [0, 0, 2, 1])
    if value == 0: return body + u(0)
    return body + u(1) + u(0) + u(1) + u(1) + u(0) + u(2) + bytes([value])

def filter_(kind, enabled=True, covered=None):
    return dict(kind=kind, enabled=enabled, mask=covered)

PARAMS = {'invert-rgb-v1': b'', 'box-blur-v1': b'\x01',
          'levels-rgb-v1': bytes([0,255,0,100,0,255])}

def wire(version, filters=(), own=None, attachment=None, title=None, name='Layer',
         visible=True, opacity=255, origin=(0,0), pixels=None, encoding=1):
    body = b'PCND' + bytes([version]) + b'\r\n\x1a'
    profiles = sorted({f['kind'] for f in filters})
    if version >= 6:
        prelude = u(len(profiles)) + b''.join(text(k)+text('params-v1') for k in profiles)
        body += u(len(prelude)) + prelude
    body += DOC + STATE + u(2) + u(1)
    body += b'\x00' if title is None else b'\x01' + text(title)
    if version >= 3: body += bytes([int(attachment is not None)])
    if attachment is not None: body += attachment
    body += u(1) + LAYER + text(name) + bytes([int(visible), opacity])
    body += signed(origin[0])+signed(origin[1])+u(2)+u(1)+bytes([255]*4)
    if version >= 3:
        body += u(len(filters))
        for index, f in enumerate(filters):
            body += bytes([32+index])*16 + bytes([int(f['enabled'])])
            if version >= 6:
                params = PARAMS[f['kind']]
                body += u(profiles.index(f['kind']))+u(len(params))+params
            else:
                body += b'\x01' if f['kind']=='invert-rgb-v1' else b'\x07\x01'
            if version >= 4:
                body += b'\x00' if f['mask'] is None else mask(*f['mask'])
    if version >= 7: body += b'\x00' if own is None else mask(*own)
    if pixels is not None:
        body += bytes([encoding])+u(0)+u(0)+u(len(pixels))+pixels
    body += b'\x00'
    return body + hashlib.sha256(body).digest()

resolution = metadata('resolution-only.pcnd')
icc = metadata('rgb-profile.pcnd')
cases = [
 ('v1-default',1,{}),
 ('v1-raw-hidden-rgb',1,dict(pixels=bytes([1,2,3,0,4,5,6,255]))),
 ('v1-rle-two-equal',1,dict(pixels=bytes([1,0,19,43,71,97]),encoding=2)),
 ('v1-explicit-default-elides',1,{}),
 ('v2-resolution-only',2,dict(attachment=resolution)),
 ('v2-icc-original',2,dict(attachment=icc)),
 ('v3-invert-enabled',3,dict(filters=[filter_('invert-rgb-v1')])),
 ('v3-invert-disabled',3,dict(filters=[filter_('invert-rgb-v1',False)])),
 ('v4-filter-mask128',4,dict(filters=[filter_('invert-rgb-v1',covered=(128,True))])),
 ('v4-disabled-zero-filter-mask',4,dict(filters=[filter_('invert-rgb-v1',False,(0,False))])),
 ('v5-boxblur',5,dict(filters=[filter_('box-blur-v1')])),
 ('v5-hidden-disabled-boxblur',5,dict(filters=[filter_('box-blur-v1',False)],visible=False,opacity=0)),
 ('v6-levels',6,dict(filters=[filter_('levels-rgb-v1')])),
 ('v6-disabled-levels-mask128',6,dict(filters=[filter_('levels-rgb-v1',False,(128,True))])),
 ('v6-catalog-sort-order',6,dict(filters=[filter_('levels-rgb-v1'),filter_('invert-rgb-v1')])),
 ('v7-own128-empty-catalog',7,dict(own=(128,True))),
 ('v7-disabled-own-zero',7,dict(own=(0,False))),
 ('v7-filter-and-own-stored',7,dict(filters=[filter_('invert-rgb-v1',False,(128,False))],own=(255,True))),
 ('v7-repeated-mask-occurrence',7,dict(filters=[filter_('invert-rgb-v1',covered=(128,True))],own=(128,True))),
 ('v7-metadata-unicode-origin',7,dict(own=(128,True),attachment=resolution,title='絵',name='レイヤー',origin=(-3,5))),
]
rows=[]
for name,version,settings in cases:
    expected=wire(version,**settings)
    source=wire(1,pixels=bytes([255]*8)) if name=='v1-explicit-default-elides' else expected
    rows.append(dict(name=name,version=version,input_hex=source.hex(),expected_hex=expected.hex(),
                     input_sha256=hashlib.sha256(source).hexdigest(),expected_sha256=hashlib.sha256(expected).hexdigest(),
                     expected_bytes=len(expected),independent=True))
result=dict(status='independent-stdlib-literals-before-new-Rust',cases=rows,
            metadata_origins=[dict(path=str((META/n).relative_to(ROOT)),sha256=hashlib.sha256((META/n).read_bytes()).hexdigest()) for n in ['resolution-only.pcnd','rgb-profile.pcnd']])
(HERE/'oracle.json').write_text(json.dumps(result,indent=4,ensure_ascii=False)+'\n')
