"""Ignored independent ordered-stage proposal; Fraction references, no product kernels."""
from fractions import Fraction as F
from pathlib import Path
import ast,argparse,hashlib,json,runpy,sys
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent
parser=argparse.ArgumentParser()
parser.add_argument('--repo-root',type=Path,default=HERE.parents[4])
parser.add_argument('--rust-data',type=Path)
parser.add_argument('--check',action='store_true')
args=parser.parse_args()
ROOT=args.repo_root.resolve()
RUST_DATA=args.rust_data or ROOT/'crates/services/paintcraft-render/src/hue_saturation_tests/ordered_data.rs' 
REF=ROOT/'crates/services/paintcraft-effects/tests'
paths={k:REF/v/'generate.py' for k,v in [('hsl','hue-saturation-fixtures'),('v','vibrance-fixtures'),('curves','curves-fixtures')]}
r={k:runpy.run_path(str(p),run_name='independent_ordered_'+k) for k,p in paths.items()}
nearest=r['hsl']['nearest']
table=r['curves']['table']([23,9,240,73,201])[0]
source=[[20,115,200,255],[64,16,128,255],[200,30,30,255]]

def point(rows,kind,params,coverage=255):
 out=[]
 for rgba in rows:
  rgb=rgba[:3]
  candidate=(r['hsl']['candidate'](rgb,params) if kind=='hsl' else
             r['v']['candidate'](rgb,params) if kind=='v' else [table[c] for c in rgb])
  out.append(r['hsl']['masked'](rgba,candidate,coverage))
 return out

def box(rows):
 # Radius1 finite CLAMP, opaque 3x1: vertical repeated samples cancel exactly.
 out=[]
 for x in range(len(rows)):
  slots=[rows[min(len(rows)-1,max(0,x+dx))] for dx in [-1,0,1]]
  out.append([nearest(sum((F(p[c]) for p in slots),F(0))/3) for c in range(3)]+[255])
 return out

records=[]
for hsl in [(60,100,10),(-137,-50,-10),(180,0,0)]:
 for coverage in [0,127,255]:
  for other,params in [('v',100),('curves',None),('box',1)]:
   h=lambda rows:point(rows,'hsl',hsl,coverage)
   other_fn=(lambda rows:box(rows)) if other=='box' else (lambda rows:point(rows,other,params))
   a,b=other_fn(h(source)),h(other_fn(source))
   records.append({'hsl':list(hsl),'coverage':coverage,'other':other,'source':source,'hsl_then_other':a,'other_then_hsl':b,'noncommutative':a!=b})
result={'policy':'Each point candidate rounds once, A8 mixes once afterward; Box radius1 finite CLAMP opaque exact mean nearest ties-up. No product evaluation imported.','references':{k:{'path':str(p.relative_to(ROOT)),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for k,p in paths.items()},'records':records,'literal_hsl_60_100_10':point(source,'hsl',(60,100,10))}
encoded=json.dumps(result,indent=2)+'\n'
expected=[(v['hsl'],v['coverage'],{'v':0,'curves':1,'box':2}[v['other']],sum(v['hsl_then_other'],[]),sum(v['other_then_hsl'],[])) for v in records]
if args.check:
 assert (HERE/'oracle.json').read_text()==encoded
 rust=RUST_DATA.read_text()
 normalized=''.join(rust.split())
 assert 'pub(super)typeCase=([i16;3],u8,u8,[u8;12],[u8;12]);' in normalized
 assert 'pub(super)constCASES:[Case;27]=' in normalized
 actual=ast.literal_eval(rust.split('pub(super) const CASES:',1)[1].split('=',1)[1].strip().rstrip(';'))
 assert actual==expected
else:
 (HERE/'oracle.json').write_text(encoded)
 RUST_DATA.parent.mkdir(parents=True,exist_ok=True)
 RUST_DATA.write_text('//! Independent Fraction expected data; tests/hue-saturation-order-fixtures.\n'+'pub(super) type Case = ([i16;3],u8,u8,[u8;12],[u8;12]);\npub(super) const CASES: [Case;27] = [\n'+''.join(str(row).replace("'",'')+',\n' for row in expected)+'];\n')
print(len(records),'records',sum(p['noncommutative'] for p in records),'causal order differences',hashlib.sha256(encoded.encode()).hexdigest())
