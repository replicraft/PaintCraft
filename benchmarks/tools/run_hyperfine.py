"""Explicit, non-installing Hyperfine runner for a locally reviewed benchmark config."""
from pathlib import Path
import argparse
from datetime import datetime, timezone
import json
import os
import platform
import shutil
import signal
import subprocess
import sys

sys.dont_write_bytecode = True

from benchmark_support import BenchmarkError, read_json, validate_config, artifact_snapshot, hyperfine_argv, expand, argv, sha256

def write_json(path: Path, data: object) -> None:
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n')

def run_logged(command: list[str], cwd: Path, env: dict, timeout: int, output: Path) -> int:
    """Bound parent/group lifetime on POSIX. Not a sandbox or a Windows job-object host."""
    with output.open('wb') as log:
        proc=subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                              stdout=log, stderr=subprocess.STDOUT, start_new_session=(os.name=='posix'))
        try:
            return proc.wait(timeout=timeout)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            if os.name=='posix':
                try: os.killpg(proc.pid,signal.SIGTERM)
                except ProcessLookupError: pass
            else:
                proc.terminate()
            try: proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                if os.name=='posix':
                    try: os.killpg(proc.pid,signal.SIGKILL)
                    except ProcessLookupError: pass
                else: proc.kill()
                proc.wait(timeout=2)
            raise

def execute(config: dict, config_path: Path, output: Path, executable: str, reverse: bool) -> dict:
    before=artifact_snapshot(config)
    if output.exists(): raise BenchmarkError('output directory already exists; preserve prior evidence')
    resolved=shutil.which(executable)
    if not resolved: raise BenchmarkError('Hyperfine is not installed/found; no automatic installation is performed')
    output.mkdir(parents=True,exist_ok=False)
    write_json(output/'config.json',config)
    env=os.environ.copy();env.update(config['env'])
    cwd=Path(expand(config['cwd'])).resolve()
    command=hyperfine_argv(config,resolved,output,reverse)
    meta={'schema':'paintcraft.benchmark-execution/1','mode':config['mode'],'case_id':config['case_id'],
          'execution_status':'started','qualification_status':'unreviewed','application_qualification':False,
          'started_utc':datetime.now(timezone.utc).isoformat(),'order':'reverse' if reverse else 'forward',
          'config_sha256':sha256(config_path),'host':{'system':platform.system(),'release':platform.release(),'machine':platform.machine(),'python':platform.python_version(),'logical_cpu_count':os.cpu_count()},
          'explicit_environment':config['env'],'artifacts_before':before,'argv':command,
          'limitations':['No automatic semantic/performance qualification','Platform details, traces, extra shared libraries and thermal/power observations require the reviewed build/evidence record']}
    write_json(output/'execution.json',meta)
    failure=None
    try:
        version=subprocess.run([resolved,'--version'],capture_output=True,text=True,check=True,timeout=10).stdout.strip()
        help_text=subprocess.run([resolved,'--help'],capture_output=True,text=True,check=True,timeout=10).stdout
        required=['--shell','--warmup','--runs','--export-json','--export-markdown','--command-name']
        if any(flag not in help_text for flag in required):
            raise BenchmarkError('installed Hyperfine lacks a required flag; review/adapt explicitly')
        meta['hyperfine_version']=version;meta['hyperfine_binary_sha256']=sha256(Path(resolved))
        for i,cmd in enumerate(config['correctness_commands']):
            result=run_logged(argv(cmd),cwd,env,config['timeout_seconds'],output/f'correctness-{i:02d}.log')
            if result:
                raise BenchmarkError(f'correctness command {i} failed with exit {result}; timing not accepted')
        meta['correctness_commands_completed']=len(config['correctness_commands'])
        # Detect mutation during correctness as well as during timing.
        if artifact_snapshot(config)!=before: raise BenchmarkError('inputs changed during preflight')
        result=run_logged(command,cwd,env,config['timeout_seconds'],output/'hyperfine.log')
        meta['hyperfine_exit_code']=result
        if result: raise BenchmarkError(f'Hyperfine failed with exit {result}')
        raw=output/'hyperfine.json'
        if not raw.is_file() or raw.stat().st_size==0:
            raise BenchmarkError('missing raw Hyperfine export')
        # Timing exports may be larger than configs; parse but never transform or rank them.
        parsed=json.loads(raw.read_text())
        if not isinstance(parsed,dict) or not isinstance(parsed.get('results'),list):
            raise BenchmarkError('expected Hyperfine results array; review unsupported export formats explicitly')
        if len(parsed['results'])!=len(config['variants']) or any(not isinstance(row,dict) or not row for row in parsed['results']):
            raise BenchmarkError('raw export does not account for each contender')
        after=artifact_snapshot(config)
        if after!=before: raise BenchmarkError('inputs changed during measurement')
        meta['artifacts_after']=after
        meta['raw_export_sha256']=sha256(raw)
        meta['execution_status']='completed'
    except (Exception,KeyboardInterrupt) as error:
        failure=error
        meta['execution_status']='interrupted' if isinstance(error,KeyboardInterrupt) else 'failed'
        meta['error']=f'{type(error).__name__}: {error}'
    finally:
        meta['finished_utc']=datetime.now(timezone.utc).isoformat()
        write_json(output/'execution.json',meta)
        lines=[]
        for path in sorted(output.rglob('*')):
            if path.is_file() and path.name!='MANIFEST.sha256':
                lines.append(f'{sha256(path)}  {path.relative_to(output).as_posix()}')
        (output/'MANIFEST.sha256').write_text('\n'.join(lines)+'\n')
    if failure:
        raise BenchmarkError(f'run not accepted; details retained at {output}: {meta.get("error")}') from failure
    return meta

def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--hyperfine',default='hyperfine')
    parser.add_argument('--order',choices=['forward','reverse'],default='forward')
    parser.add_argument('--execute',action='store_true',help='Execute this locally reviewed config; default is plan-only')
    args=parser.parse_args()
    try:
        config=read_json(args.config);validate_config(config)
        artifacts=artifact_snapshot(config)
        command=hyperfine_argv(config,args.hyperfine,args.output.resolve(),args.order=='reverse')
        if not args.execute:
            print(json.dumps({'mode':'plan-only','executed':False,'qualifies_application':False,
                              'argv':command,'verified_config_artifacts':artifacts,
                              'notice':'No benchmark programs were run and no repository/output files were changed. Review config and use --execute only when its workload is relevant.'},indent=2))
            return
        result=execute(config,args.config.resolve(),args.output.resolve(),args.hyperfine,args.order=='reverse')
        print(json.dumps({'execution_status':result['execution_status'],'qualification_status':'unreviewed','evidence':str(args.output.resolve())},indent=2))
    except (BenchmarkError,OSError,ValueError,subprocess.SubprocessError) as error:
        print(f'benchmark error: {error}',file=sys.stderr)
        raise SystemExit(1)

if __name__=='__main__':
    main()
