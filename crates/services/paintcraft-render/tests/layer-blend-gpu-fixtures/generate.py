"""Independent Fraction public GPU stage expectations; no product imports."""
from fractions import Fraction as F
from pathlib import Path
import json,sys,hashlib
HERE=Path(__file__).parent
PARENT=HERE.parents[3]/'foundation/paintcraft-core/tests/layer-blend-fixtures/oracle.json'
if not PARENT.exists():PARENT=Path.cwd()/'crates/foundation/paintcraft-core/tests/layer-blend-fixtures/oracle.json'
source=json.loads(PARENT.read_text())
def up(v):return (2*v.numerator+v.denominator)//(2*v.denominator)
def over(back,front,mode,mask=255,opacity=255):
 a=up(F(front[3]*mask,255));a=up(F(a*opacity,255))
 if a==0:return back[:]
 a,b=F(a,255),F(back[3],255);A=a+b*(1-a);out=[]
 for s,d in zip(front[:3],back[:3]):
  s,d=F(s,255),F(d,255)
  M=s if mode=='normal' else s*d if mode=='multiply' else 1-(1-s)*(1-d)
  out.append(up(255*((1-a)*b*d+(1-b)*a*s+a*b*M)/A))
 return out+[up(255*A)]
def present(v):return [up(F(c*v[3],255)) for c in v[:3]]+[v[3]]
rows=[]
for row in source['vectors']:
 backdrop=over([0,0,0,0],row['back'],'normal')
 for case in row['cases']:
  out=over(backdrop,row['front'],case['mode'],row['own_mask'],row['opacity'])
  rows.append({'name':row['name']+'-'+case['mode'],'back':row['back'],'front':row['front'],'mode':case['mode'],'mask':row['own_mask'],'opacity':row['opacity'],'straight':out,'display':present(out)})
for back in [[31,127,241,255],[31,127,241,128]]:
 for mode in ['multiply','screen']:
  out=over(over([0,0,0,0],back,'normal'),[20,115,200,255],mode,127,128)
  rows.append({'name':'mask127-'+str(back[3])+'-'+mode,'back':back,'front':[20,115,200,255],'mode':mode,'mask':127,'opacity':128,'straight':out,'display':present(out)})
order=[]
for indices in [[0,1],[1,0]]:
 layers=source['order']['layers'];out=source['order']['backdrop'][:]
 for i in indices:out=over(out,layers[i]['rgba'],layers[i]['mode'],255,layers[i]['opacity'])
 order.append({'order':indices,'straight':out,'display':present(out)})
spatial=[]
for mode in ['multiply','screen']:
 back=[31,127,241,128];native=[over(back,p,mode,127 if i==0 else 0,128) for i,p in enumerate([[170,0,85,255],[85,0,170,255]])]
 A=sum(p[3] for p in native);lod=[up(F(sum(p[c]*p[3] for p in native),A)) for c in range(3)]+[up(F(A,2))]
 spatial.append({'mode':mode,'back':back,'raw':[255,0,0,255,0,0,255,255],'mask':127,'opacity':128,'straight':sum(native,[]),'display':sum([present(v) for v in native],[]),'lod1_straight':lod,'lod1_display':present(lod)})
# Partial right edge has one sample; blend before reducing2+1 native samples.
edge=[]
for mode in ['multiply','screen']:
 back=[31,127,241,128];raw=[[255,0,0,255],[0,255,0,128],[0,0,255,64]]
 native=[over(back,p,mode,127,128) for p in raw];sums=[]
 for group in [native[:2],native[2:]]:
  A=sum(p[3] for p in group);v=[up(F(sum(p[c]*p[3] for p in group),A)) for c in range(3)]+[up(F(A,len(group)))];sums.append(v)
 edge.append({'mode':mode,'back':back,'raw':sum(raw,[]),'native':sum(native,[]),'lod1_straight':sum(sums,[]),'lod1_display':sum([present(v) for v in sums],[])})
value={'status':'independent public-stage expectations frozen before tests; transparent-start Normal backdrop projection is explicit','parent_sha256':hashlib.sha256(PARENT.read_bytes()).hexdigest(),'vectors':rows,'ordered':order,'spatial':spatial,'edge':edge}
text=json.dumps(value,indent=2)+'\n';path=HERE/'oracle.json'
if '--check' in sys.argv:assert path.read_text()==text
else:path.write_text(text)
print(len(rows),hashlib.sha256(text.encode()).hexdigest())
