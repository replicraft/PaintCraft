"""Independent Fraction CPU scene/LOD literals; no product modules."""
from fractions import Fraction as F
from pathlib import Path
import json,sys,hashlib

def up(q):return (2*q.numerator+q.denominator)//(2*q.denominator)
def mix(back,front,mode,mask=255,opacity=255):
 a=up(F(front[3]*mask,255));a=up(F(a*opacity,255))
 if not a:return back[:]
 af,ab=F(a,255),F(back[3],255);alpha=af+ab*(1-af);out=[]
 for s,d in zip(front[:3],back[:3]):
  s,d=F(s,255),F(d,255);b=d*s if mode=='multiply' else d+s-d*s
  out.append(up(255*((1-af)*ab*d+(1-ab)*af*s+af*ab*b)/alpha))
 return out+[up(255*alpha)]
back=[31,127,241,128]
# Independent explicit clamped3-tap averages of red/blue source, BEFORE own mask.
blur=[[170,0,85,255],[85,0,170,255]]
rows=[]
for mode in ['multiply','screen']:
 native=[mix(back,p,mode,128 if i==0 else 0,128) for i,p in enumerate(blur)]
 A=sum(p[3] for p in native)
 lod=[up(F(sum(p[c]*p[3] for p in native),A)) for c in range(3)]+[up(F(A,2))]
 rows.append({'mode':mode,'backdrop':back,'raw':[255,0,0,255,0,0,255,255],'box1':blur,'mask_domain':[0,0,1,1],'mask':128,'opacity':128,'native':native,'lod1':lod})
text=json.dumps({'status':'independent expected literals frozen before renderer tests','spatial':rows},indent=2)+'\n'
p=Path(__file__).with_name('oracle.json')
if '--check' in sys.argv:assert p.read_text()==text
else:p.write_text(text)
print(hashlib.sha256(text.encode()).hexdigest())
