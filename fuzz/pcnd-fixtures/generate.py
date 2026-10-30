"""Independent, separately inventoried PCND reader seeds; no Rust writer calls."""
from pathlib import Path
import hashlib
import json
import struct
import subprocess
import sys
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from pcnd_reference import canonical
HERE=Path(__file__).resolve().parent
PARENT=ROOT/'crates/services/paintcraft-formats/tests/pcnd-metadata-fixtures'
def sha(data):return hashlib.sha256(data).hexdigest()
def framed(data):return bytes(data[:-32])+hashlib.sha256(data[:-32]).digest()
def number(data,at):
    value=0;shift=0
    while True:
        byte=data[at];at+=1;value|=(byte&127)<<shift
        if byte<128:return value,at
        shift+=7

def integer(value):
    out=bytearray()
    while value>=128:out.append((value&127)|128);value>>=7
    out.append(value);return bytes(out)
def block(source):
    at=40
    for _ in range(2):_,at=number(source,at)
    flag=source[at];at+=1
    if flag:n,at=number(source,at);at+=n
    n,at=number(source,at);assert n==1
    at+=16;n,at=number(source,at);at+=n+2
    for _ in range(4):_,at=number(source,at)
    at+=4;kind=at;assert source[at]==1;at+=1
    for _ in range(2):_,at=number(source,at)
    length_at=at;n,at=number(source,at)
    return kind,length_at,at,source[at:at+n],source[at+n:-32]
def literal(data):
    if len(data)<15:return bytes([len(data)<<4])+data
    out=bytearray([240]);n=len(data)-15
    while n>=255:out.append(255);n-=255
    out.append(n);return bytes(out)+data

def main():
    seeds=[];provenance=[]
    def existing(name):
        p=PARENT/name;b=p.read_bytes();seeds.append(dict(target='pcnd',name=name,source=p.relative_to(ROOT).as_posix(),bytes=len(b),sha256=sha(b),accepted=True))
    for name in ['legacy-v1.pcnd','rgb-profile.pcnd','resolution-only.pcnd','orientation-only.pcnd']:existing(name)
    parent=(PARENT/'rgb-profile.pcnd').read_bytes();header=parent.index(b'PCSMETA1')
    def derived(name,data,base,recipe,accepted=True):
        p=HERE/name;p.write_bytes(data);seeds.append(dict(target='pcnd',name=name,source=p.relative_to(ROOT).as_posix(),bytes=len(data),sha256=sha(data),accepted=accepted));provenance.append(dict(file=name,parent_sha256=sha(base),recipe=recipe,sha256=sha(data)))
    unknown=bytearray(parent);unknown[4]=4;derived('unknown-version.pcnd',framed(unknown),parent,'Set container version byte4 to4; recompute trailing outerSHA256.',False)
    overflow=bytearray(parent);struct.pack_into('>Q',overflow,header+112,(1<<64)-1);derived('icc-extent-overflow.pcnd',framed(overflow),parent,'Set PCSMETA1 profile extent at header+112 to u64MAX; preserve old inner digest to verify extent admission precedes copy; recompute outerSHA256.',False)
    inner=bytearray(parent);inner[header+152]^=1;derived('inner-digest-mismatch.pcnd',framed(inner),parent,'Flip first attachment digest byte at header+152 only; recompute outerSHA256.',False)
    legacy=(PARENT/'legacy-v1.pcnd').read_bytes();kind,length_at,data_at,raw,suffix=block(legacy)
    runs=b''.join(b'\0\0'+raw[at:at+4] for at in range(0,len(raw),4))
    compressed=subprocess.check_output(['zstd','-q','--stdout','--stream-size='+str(len(raw))],input=raw)
    for name,encoding,payload,recipe in [('rgba-runs.pcnd',2,runs,'u16LE zero count-minus-one followed by each originalRGBA sample'),('zstd.pcnd',3,compressed,'Zstandard1.5.7 CLI -q --stdout --stream-size=16 of exact original raw tile'),('lz4.pcnd',4,literal(raw),'Independent LZ4 literal-only block of exact original raw tile'),('runs-lz4.pcnd',5,struct.pack('<I',len(runs))+literal(runs),'u32LE run extent plus independent LZ4 literal-only block of exact RGBA runs')]:
        body=legacy[:kind]+bytes([encoding])+legacy[kind+1:length_at]+integer(len(payload))+payload+suffix
        derived(name,body+hashlib.sha256(body).digest(),legacy,recipe+'; replace only tile encoding/length/payload and recompute outerSHA256.')
    def sparse(width,height,points,name):return canonical(dict(document_id='33333333-3333-4333-8333-333333333333',state_id='11111111-1111-4111-8111-111111111111',canvas=(2,2),title=name,layers=[dict(id='22222222-2222-4222-8222-222222222222',name=name,visible=True,opacity=255,origin=(-513,257),width=width,height=height,default=(13,71,201,0),pixels=points)]))
    derived('huge-default-only.pcnd',sparse(1048576,1048576,{},'Huge default-only layer'),legacy,'Independent canonical writer:2x2canvas, one1048576x1048576default-only sparse layer, hiddenRGBdefault, no resident tiles.')
    points={(0,0):(1,2,3,255),(256,0):(4,5,6,0)}
    derived('two-tiles.pcnd',sparse(768,1,points,'Two resident tiles'),legacy,'Independent canonical writer:768x1layer, samples at x0 and256; exactly2nominal256x256 resident tiles.')
    derived('three-tiles.pcnd',sparse(768,1,{**points,(512,0):(7,8,9,255)},'Three resident tiles'),legacy,'Independent canonical writer:768x1layer, samples at x0/256/512;3nominaltiles exceed131072staged-pixel target allowance.',False)
    assert len(seeds)==14
    (HERE/'provenance.json').write_text(json.dumps(dict(schema='paintcraft.pcnd-fuzz-provenance/v1',generator_sha256=sha(Path(__file__).read_bytes()),reference_writer_sha256=sha((ROOT/'scripts/pcnd_reference.py').read_bytes()),zstd=subprocess.check_output(['zstd','--version'],text=True).strip(),derived=provenance),indent=2)+'\n')
    (ROOT/'fuzz/pcnd-seeds.json').write_text(json.dumps(dict(schema='paintcraft.pcnd-fuzz-seeds/v1',seeds=seeds),indent=2)+'\n')
    print('14 separatePCNDseeds; existing historical24/stock29sources unchanged')
if __name__=='__main__':main()
