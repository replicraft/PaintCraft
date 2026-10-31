"""Plan/retain actual existing Criterion raw samples for registered Noise kernels."""
import argparse,csv,json,math,os,sys
from pathlib import Path
sys.dont_write_bytecode=True
ROOT=Path.cwd();sys.path.insert(0,str(ROOT/'benchmarks/tools'))
from benchmark_support import sha256
from run_hyperfine import run_logged,write_json


def main():
 p=argparse.ArgumentParser(description=__doc__)
 p.add_argument('--binary',type=Path,required=True);p.add_argument('--build-receipt',type=Path,required=True)
 p.add_argument('--output',type=Path,required=True);p.add_argument('--order',choices=['forward','reverse','rotate'],default='forward')
 p.add_argument('--exact2mp',action='store_true');p.add_argument('--execute',action='store_true')
 a=p.parse_args();binary=a.binary.resolve();receipt=json.loads(a.build_receipt.read_text())
 assert receipt['schema']=='paintcraft.noise-kernel-build/1' and receipt['status']=='passed'
 assert Path(receipt['criterion_binary']['path']).resolve()==binary and sha256(binary)==receipt['criterion_binary']['sha256']
 def verify():
  assert sha256(binary)==receipt['criterion_binary']['sha256']
  for row in receipt['sources']:assert sha256(Path(row['path']))==row['sha256'],row['path']
 verify();cases=[]
 for size in ['default','1080p']+(['exact-2MP']if a.exact2mp else[]):
  for dist in ['uniform','gaussian']:
   for mono in ['mono','color']:
    for amount in [10,0]:cases.append((f'{size}-{dist}-{mono}',amount))
 if a.order=='reverse':cases.reverse()
 if a.order=='rotate':cases=cases[1:]+cases[:1]
 commands=[]
 for case,amount in cases:
  label='default10'if amount else'identity-calibration'
  commands.append({'case':case,'amount':amount,'argv':[str(binary),'--bench',f'noise-kernel/{case}/{label}','--exact']})
 if not a.execute:
  print(json.dumps({'status':'plan-only','executed':False,'commands':commands},indent=2));return
 assert not a.output.exists(),'fresh Criterion evidence directory required';a.output.mkdir(parents=True)
 write_json(a.output/'plan.json',{'build_receipt_sha256':sha256(a.build_receipt),'order':a.order,'commands':commands,'scope':'Kernel only; source/construction/hash outside measured section; no UI delivery claim'})
 results=[];completed=False;failure=None
 try:
  for i,row in enumerate(commands):
   verify();env=dict(os.environ,CRITERION_HOME=str((a.output/'raw').resolve()),PAINTCRAFT_NOISE_CASE=row['case'],PAINTCRAFT_NOISE_AMOUNT=str(row['amount']))
   if a.exact2mp:env['PAINTCRAFT_NOISE_EXACT2MP']='1'
   code=run_logged(row['argv'],ROOT,env,600,a.output/f'{i:02d}.log')
   results.append(row|{'exit':code});write_json(a.output/'results.json',results)
   if code:raise RuntimeError('preserve failed first statistical invocation')
   label='default10'if row['amount']else'identity-calibration'
   sample=a.output/'raw/noise-kernel'/row['case']/label/'new/sample.json'
   data=json.loads(sample.read_text());assert len(data['times'])==len(data['iters'])==100
   assert all(math.isfinite(t)and t>0 for t in data['times'])and all(n>0 for n in data['iters'])
   with sample.with_name('raw.csv').open()as raw:assert len(list(csv.DictReader(raw)))==100
   results[-1]['raw_sample_sha256']=sha256(sample);write_json(a.output/'results.json',results)
   verify()
  completed=True
 except (Exception,KeyboardInterrupt)as error:
  failure=repr(error);raise
 finally:write_json(a.output/'final.json',{'status':'passed'if completed else'failed','error':failure,'results':results,'binary_sha256':sha256(binary),'scope':'Raw Criterion kernel observations only; not application-visible completion'})


if __name__=='__main__':main()
