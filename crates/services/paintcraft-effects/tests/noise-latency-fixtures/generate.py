"""Independent procedural probes, not product-generated full-plane goldens."""
import hashlib,json,os,runpy,sys
from pathlib import Path
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent
REFERENCE=Path(os.environ.get('PAINTCRAFT_NOISE_MATH_REFERENCE',str(HERE.parent/'noise-fixtures/reference.py')))
assert hashlib.sha256(REFERENCE.read_bytes()).hexdigest()=='031d258e1ed97921360f7e3344b6f71715316bd131aee91de9505164ec9cc68c'
r=runpy.run_path(str(REFERENCE))
rows=[]
for label,w,h in [('default',1000,486),('1080p',1920,1080),('exact-2MP',2000,1000)]:
 for distribution in ['uniform','gaussian']:
  for mono in [True,False]:
   case=f'{label}-{distribution}-'+('mono'if mono else'color')
   for amount in [0,10]:
    for x,y in [(0,0),(1,0),(63,1),(64,63),(65,64),(w//2,h//2),(w-1,0),(0,h-1),(w-1,h-1)]:
     rgb=[(17*x+31*y+23)&255,(29*x+11*y+71)&255,(7*x+43*y+193)&255]
     words=[r['draws'](x,y,1,0 if mono else c)for c in range(3)]
     if distribution=='uniform':expected=r['uniform'](rgb,amount,words)
     else:
      expected,_=r['gaussian'](rgb,amount,words,160,True)
      low,_=r['gaussian'](rgb,amount,words,96,True)
      assert expected==low,'Preserve/resolve precision ambiguity; never filter a case'
     for alpha in [0,255]:
      rows.append({'case':case,'amount_hundredths':amount,'seed':1,'monochrome':mono,'distribution':distribution,'xy':[x,y],'rgba':rgb+[alpha],'expected':expected+[alpha]})
assert len(rows)==432
v={'scope':'Independent finite Fraction/Decimal math probes for procedural benchmark source. No producer/libm expected outputs or global ideal-real-rounding claim.','reference_sha256':hashlib.sha256(REFERENCE.read_bytes()).hexdigest(),'rows':rows}
blob=(json.dumps(v,indent=2)+'\n').encode();p=HERE/'oracle.json'
if '--check'in sys.argv:assert p.read_bytes()==blob
else:assert not p.exists();p.write_bytes(blob)
print('432 independent procedural/hiddenRGB probes',hashlib.sha256(blob).hexdigest())
