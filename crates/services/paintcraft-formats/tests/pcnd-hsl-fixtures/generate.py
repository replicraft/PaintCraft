"""Independent PCND6 HSL signed-six-byte framing; no PaintCraft imports."""
import argparse,hashlib,json,struct,uuid
from pathlib import Path
HERE=Path(__file__).resolve().parent
PROFILE='hue-saturation-rgb-v1';RAW=[10,11,128,0,20,115,200,128]
DOC='33333333-3333-4333-8333-333333333333';STATE='11111111-1111-4111-8111-111111111111';LAYER='22222222-2222-4222-8222-222222222222'
def sha(b):return hashlib.sha256(b).hexdigest()
def u(n):
 assert type(n)is int and n>=0
 out=bytearray()
 while n>=128:out.append((n&127)|128);n>>=7
 return bytes(out+bytes([n]))
def signed(n):return u(n*2 if n>=0 else -n*2-1)
def text(s):b=s.encode('ascii');return u(len(b))+b
def mask(enabled):
 if enabled is None:return b'\0'
 return bytes([1,enabled,1])+b''.join(signed(n)for n in [-1,-2,3,2])+u(0)
def mask_digest():return bytes.fromhex(sha(b'paintcraft.a8-mask/v1\0'+b''.join(struct.pack('>q',n)for n in [-1,-2,3,2])+struct.pack('>Q',0)))
def fixture(values,enabled,params=None,codec='params-v1',mask_enabled=None,profile=PROFILE):
 params=struct.pack('>hhh',*values)if params is None else params
 pre=u(1)+text(profile)+text(codec);body=b'PCND\x06\r\n\x1a'+u(len(pre))+pre
 body+=uuid.UUID(DOC).bytes+uuid.UUID(STATE).bytes+u(2)+u(1)+b'\0\0'+u(1)
 body+=uuid.UUID(LAYER).bytes+text('x')+b'\x01\xff\0\0'+u(2)+u(1)+bytes(4)+u(1)
 body+=(1).to_bytes(16,'big')+bytes([enabled])+u(0)+u(len(params))+params+mask(mask_enabled)
 body+=b'\x01\0\0'+u(len(RAW))+bytes(RAW)+b'\0'
 return body+hashlib.sha256(body).digest()
def semantic(values,enabled,mask_enabled):
 name=PROFILE.encode('ascii');body=b'paintcraft.filter-stack/v3\0'+struct.pack('>H',1)+(1).to_bytes(16,'big')+bytes([enabled])+struct.pack('>H',len(name))+name+struct.pack('>Ihhh',6,*values)
 body+=b'\0'if mask_enabled is None else bytes([1,mask_enabled])+mask_digest()
 return sha(body)
def generate():
 rows=[]
 def add(name,values,enabled=1,accepted=True,params=None,codec='params-v1',mask_enabled=None,profile=PROFILE):
  data=fixture(values,enabled,params,codec,mask_enabled,profile)
  row=dict(name=name,accepted=accepted,parameters=list(values),enabled=bool(enabled),mask_enabled=None if mask_enabled is None else bool(mask_enabled),hex=data.hex(),sha256=sha(data))
  if accepted:row['stack_digest_v3']=semantic(values,enabled,mask_enabled)
  else:row['error']='CapabilityUnsupported'if codec!='params-v1'or profile!=PROFILE else'CodecFailed'
  rows.append(row)
 for i,v in enumerate([(0,0,0),(-180,-100,100),(180,100,-100),(-180,0,0),(180,0,0),(137,73,-19),(-1,1,-1),(-1,-2,-3)]):add('enabled-'+str(i),v)
 for i,v in enumerate([(0,0,0),(-180,-100,100),(180,100,-100)]):add('disabled-'+str(i),v,0)
 add('zero-mask-enabled',(0,0,0),mask_enabled=1);add('zero-mask-disabled',(137,73,-19),0,mask_enabled=0)
 for i,v in enumerate([(-181,0,0),(181,0,0),(0,-101,0),(0,101,0),(0,0,-101),(0,0,101),(-32768,0,0),(32767,0,0)]):add('range-'+str(i),v,accepted=False)
 for n in [0,1,2,4,5,7,8]:add('length-'+str(n),(0,0,0),accepted=False,params=bytes(n))
 add('unknown-codec-disabled',(0,0,0),0,False,codec='params-v2');add('unknown-profile-disabled',(0,0,0),0,False,profile='future-hsl-v1')
 return dict(recipe='Independent stdlib PCND6 framing, H/S/L signed i16BE and optional canonical allzero A8 mask. No product codec/renderer.',raw_rgba=RAW,fixtures=rows)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');args=p.parse_args();value=generate();text_value=json.dumps(value,indent=2)+'\n';path=HERE/'oracle.json'
 if args.check:assert path.read_text()==text_value,'fixture drift'
 else:path.write_text(text_value)
 print(json.dumps(dict(records=len(value['fixtures']),positive=sum(r['accepted']for r in value['fixtures']),negative=sum(not r['accepted']for r in value['fixtures']))))
