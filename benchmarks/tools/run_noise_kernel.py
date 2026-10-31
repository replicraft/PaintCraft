"""Plan fresh registered Noise process confirmations from REAL reviewed build hashes."""
import argparse,json,sys
from pathlib import Path
sys.dont_write_bytecode=True
ROOT=Path.cwd()
sys.path.insert(0,str(ROOT/'benchmarks/tools'))
from benchmark_support import validate_config,sha256


def main():
 p=argparse.ArgumentParser(description=__doc__)
 p.add_argument('--binary',type=Path,required=True)
 p.add_argument('--build-receipt',type=Path,required=True)
 p.add_argument('--output',type=Path,required=True)
 p.add_argument('--iterations',type=int,default=1)
 p.add_argument('--exact2mp',action='store_true')
 a=p.parse_args();assert 1<=a.iterations<=32
 binary=a.binary.resolve();receipt=a.build_receipt.resolve();build=json.loads(receipt.read_text())
 assert build['schema']=='paintcraft.noise-kernel-build/1' and build['status']=='passed'
 assert Path(build['binary']['path']).resolve()==binary and sha256(binary)==build['binary']['sha256']
 assert build['build']['profile']in ['release','bench']
 artifacts={}
 def add(path,role,expected=None):
  path=Path(path).resolve();actual=sha256(path)
  if expected:assert actual==expected,path
  artifacts[str(path)]={'path':str(path),'sha256':actual,'role':role}
 for row in build['sources']:add(row['path'],'other',row['sha256'])
 add(binary,'binary');add(ROOT/'Cargo.lock','lockfile');add(receipt,'other');add(Path(__file__),'driver')
 base=ROOT/'crates/services/paintcraft-effects'
 add(base/'tests/noise-latency-fixtures/oracle.json','fixture');add(base/'tests/noise-latency-fixtures/generate.py','generator')
 assert not a.output.exists(),'fresh config directory required';a.output.mkdir(parents=True)
 cases=[]
 sizes=[('default',1000,486),('1080p',1920,1080)]+([('exact-2MP',2000,1000)]if a.exact2mp else[])
 for size,width,height in sizes:
  for dist in ['uniform','gaussian']:
   for mono in ['mono','color']:
    name=f'{size}-{dist}-{mono}';cases.append(name)
    config={'schema':'paintcraft.hyperfine-run/1','mode':'application',
     'case_id':'PALETTE-NOISE-KERNEL','task_id':'PALETTE-NOISE',
     'relevance':'Standalone actual kernel/process cost only; default10 and amount0 are distinct output workloads, not equivalent algorithm contenders. No UI completion/latency qualification.',
     'cwd':str(ROOT),'workload':{'id':name,'mode':'process-lifecycle','work_units':a.iterations,'seed':1,
       'cache_state':'new process;256 periodic input rows;full output;no hidden pools',
       'completion':'actual full-frame row work+probe validation+fulloutput SHA256+teardown;not UI'},
     'build':build['build'],'runs':20,'warmup':3,'timeout_seconds':600,'env':{},
     'artifacts':list(artifacts.values()),'correctness_commands':[[str(binary),'verify',name]],
     'variants':[{'id':'default10','argv':[str(binary),'run',name,str(a.iterations)]},
                 {'id':'identity-calibration','argv':[str(binary),'identity',name,str(a.iterations)]}]}
    validate_config(config)
    (a.output/(name+'.json')).write_text(json.dumps(config,indent=2)+'\n')
 plans=[]
 for session in range(3):
  order=cases if session==0 else list(reversed(cases))if session==1 else cases[1:]+cases[:1]
  for name in order:
   plans.append({'session':session+1,'case':name,'argv':[sys.executable,str(ROOT/'benchmarks/tools/run_hyperfine.py'),'--config',str(a.output/(name+'.json')),'--output',str(a.output/f'session-{session+1}-{name}'),'--order','reverse'if session==1 else'forward','--execute']})
 (a.output/'execution-plan.json').write_text(json.dumps({'status':'PLAN_ONLY_NOT_EXECUTED','registered_case':'PALETTE-NOISE-KERNEL','kernel_not_UI':True,'commands':plans},indent=2)+'\n')
 print(json.dumps({'status':'plan-only','configs':len(cases),'three_session_commands':len(plans),'output':str(a.output)}))


if __name__=='__main__':main()
