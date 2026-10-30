"""Independent PCND4 A8/export identity and literal appearance planes (stdlib)."""
import hashlib
import json
from pathlib import Path
import struct
import uuid
ROOT=Path(__file__).resolve().parent
DOC=uuid.UUID('33333333-3333-4333-8333-333333333333').bytes
STATE=uuid.UUID('11111111-1111-4111-8111-111111111111').bytes
LAYER=uuid.UUID('22222222-2222-4222-8222-222222222222').bytes
RAW=bytes([0,0,0,255,255,255,255,255])
parent=(ROOT.parent/'pcnd-metadata-fixtures/rgb-profile.pcnd').read_bytes()
assert parent[73:81]==b'PCSMETA1'
profile_size=int.from_bytes(parent[185:193],'big')
attachment=parent[73:73+192+profile_size]
assert len(attachment)==192+profile_size

def sha(data):return hashlib.sha256(data).digest()
def mask(coverage,enabled):
    # Domain [0,0,2,1], a single uniform band/run or canonical storedzero.
    body=bytes([1,enabled,1,0,0,4,2,int(coverage is not None)])
    digest=b'paintcraft.a8-mask/v1\0'+struct.pack('>qqqqQ',0,0,2,1,int(coverage is not None))
    if coverage is not None:
        body+=bytes([0,1,1,0,2,coverage])
        digest+=struct.pack('>qqQqqB',0,1,1,0,2,coverage)
    return body,sha(digest)

def make(coverage,mask_enabled,filter_enabled,metadata):
    payload,digest=mask(coverage,mask_enabled)
    body=b'PCND\x04\r\n\x1a'+DOC+STATE+b'\x02\x01\0'+bytes([metadata])+(attachment if metadata else b'')
    body+=b'\x01'+LAYER+b'\x05Image\x01\xff\0\0\x02\x01'+bytes(4)
    body+=b'\x01'+bytes(16)+bytes([filter_enabled,1])+payload
    body+=b'\x01\0\0\x08'+RAW+b'\0'
    stack=sha(b'paintcraft.filter-stack/v2\0'+b'\0\x01'+bytes(16)+bytes([filter_enabled])+b'\0\x0d'+b'invert-rgb-v1'+bytes([1,mask_enabled])+digest)
    inventory=sha(b'paintcraft.filter-bake-witness/v1\0'+DOC+STATE+struct.pack('>IIQ',2,1,1)+LAYER+stack+b'\x01\xff'+bytes(16)+struct.pack('>II',2,1)+bytes(4))
    return body+sha(body),inventory,digest

rows=[]
for name,coverage,mask_enabled,filter_enabled,expected in [
 ('constant',255,True,True,[255,255,255,255,0,0,0,255]),
 ('fractional',128,True,True,[128,128,128,255,127,127,127,255]),
 ('zero',None,True,True,list(RAW)),
 ('mask-disabled',128,False,True,[255,255,255,255,0,0,0,255]),
 ('filter-disabled',128,True,False,list(RAW)),
]:
 for metadata in [False,True]:
    filename=name+('-metadata' if metadata else '')+'.pcnd'
    data,inventory,digest=make(coverage,mask_enabled,filter_enabled,metadata)
    (ROOT/filename).write_bytes(data)
    rows.append(dict(file=filename,sha256=sha(data).hex(),bytes=len(data),filter_inventory_sha256=inventory.hex(),mask_sha256=digest.hex(),mask_count=1,bands=int(coverage is not None),runs=int(coverage is not None),metadata=metadata,expected_rgba=expected,msp_supported=name!='fractional'))
(ROOT/'oracle.json').write_text(json.dumps(dict(generator=__doc__,source_parent_sha256=sha(parent).hex(),attachment_sha256=attachment[152:184].hex(),profile_bytes=profile_size,metadata_wire_bytes=len(attachment),document=str(uuid.UUID(bytes=DOC)),state=str(uuid.UUID(bytes=STATE)),layer=str(uuid.UUID(bytes=LAYER)),width=2,height=1,raw_rgba=list(RAW),fixtures=rows),indent=4)+'\n')
