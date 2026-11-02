"""Independent Fraction expected colors for bounded-request regressions."""
from fractions import Fraction as F
from pathlib import Path
import hashlib,json,sys

def up(v):return (2*v.numerator+v.denominator)//(2*v.denominator)
def compose(back,source,mode):
 a=F(up(F(source[3]*64,255)),255);out=[]
 for s,d in zip(source[:3],back[:3]):
  s,d=F(s,255),F(d,255)
  mixed=s*d if mode=='multiply' else 1-(1-s)*(1-d)
  out.append(up(255*((1-a)*d+a*mixed)))
 return out+[255]
rows=[]
for modes,back in [(['multiply'],[255]*4),(['screen'],[0,0,0,255]),(['multiply','screen'],[255]*4)]:
 initial=compose(back,[204,77,13,128],modes[0]);dirty=compose(back,[128,64,200,128],modes[0])
 if len(modes)==2:
  initial=compose(initial,[40,190,230,128],'screen');dirty=compose(dirty,[40,190,230,128],'screen')
 rows.append({'modes':modes,'backdrop':back,'initial':initial,'dirty':dirty,'setup':256*(len(modes)+1),'extra':32*len(modes),'limit':256*(len(modes)+1)+32*len(modes)})
text=json.dumps({'status':'independent literals frozen before correction test source','cases':rows},indent=2)+'\n'
p=Path(__file__).with_name('bounds-oracle.json')
if '--check' in sys.argv:assert p.read_text()==text
else:p.write_text(text)
print(hashlib.sha256(text.encode()).hexdigest())
