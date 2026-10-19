"""Developer benchmark validation. This is not a PaintCraft application subsystem."""
from pathlib import Path
import hashlib
import json
import math
import shutil
import re
import shlex
import sys
from typing import Any

PACK = Path(__file__).resolve().parents[2]

class BenchmarkError(ValueError):
    pass

def read_json(path: Path) -> Any:
    if path.stat().st_size > 2_000_000:
        raise BenchmarkError("configuration exceeds the 2 MB tooling bound")
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise BenchmarkError(f"duplicate JSON key: {key}")
            result[key] = value
        return result
    def constant(value):
        raise BenchmarkError(f"nonfinite JSON value: {value}")
    def finite(value):
        result=float(value)
        if not math.isfinite(result): raise BenchmarkError('out-of-range JSON float')
        return result
    return json.loads(path.read_text(), object_pairs_hook=pairs, parse_constant=constant, parse_float=finite)

def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()

def expand(value: str) -> str:
    if not isinstance(value, str) or not value or any(ch in value for ch in ['\x00', '\n', '\r']):
        raise BenchmarkError('expected nonempty single-line argument')
    for name in re.findall(r'\{[^{}]*\}', value):
        if name not in ('{pack}', '{python}'):
            raise BenchmarkError(f'unsupported placeholder: {name}')
    return value.replace('{pack}', str(PACK)).replace('{python}', sys.executable)

def argv(values: list[str]) -> list[str]:
    if not isinstance(values, list) or not values:
        raise BenchmarkError('commands must be nonempty argv arrays, not shell strings')
    return [expand(value) for value in values]

def validate_config(config: dict, cases: list[dict] | None = None) -> None:
    required = {'schema','mode','case_id','task_id','relevance','cwd','workload','build','runs','warmup','timeout_seconds','env','artifacts','correctness_commands','variants'}
    if not isinstance(config, dict) or set(config) != required:
        raise BenchmarkError('unexpected or missing config fields')
    if config['schema'] != 'paintcraft.hyperfine-run/1':
        raise BenchmarkError('unsupported config version')
    if config['mode'] not in ('application','tooling-smoke'):
        raise BenchmarkError('unsupported evidence mode')
    for name in ('case_id','task_id','relevance','cwd'):
        expand(config[name])
    for name, minimum, maximum in [('runs',2,10000),('warmup',0,1000),('timeout_seconds',1,86400)]:
        if type(config[name]) is not int or not minimum <= config[name] <= maximum:
            raise BenchmarkError(f'invalid {name}')
    w = config['workload']
    if not isinstance(w, dict) or set(w) != {'id','mode','work_units','seed','cache_state','completion'}:
        raise BenchmarkError('invalid workload descriptor')
    if w['mode'] not in ('batch','process-lifecycle','interactive-replay','gpu-complete','file-complete'):
        raise BenchmarkError('invalid measurement mode')
    if type(w['work_units']) is not int or w['work_units'] < 1 or type(w['seed']) is not int or w['seed'] < 0:
        raise BenchmarkError('invalid work count or seed')
    for name in ('id','cache_state','completion'):
        expand(w[name])
    b = config['build']
    if not isinstance(b, dict) or set(b) != {'source_revision','profile','target','compiler','flags','environment_notes'}:
        raise BenchmarkError('invalid build descriptor')
    for v in b.values():
        expand(v)
    if not isinstance(config['env'], dict):
        raise BenchmarkError('env must be an object')
    for key,value in config['env'].items():
        if not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*',key) or not isinstance(value,str) or '\x00' in value:
            raise BenchmarkError('invalid environment override')
        if any(word in key.upper() for word in ('PASSWORD','SECRET','TOKEN','PRIVATE_KEY')):
            raise BenchmarkError('do not store credentials in benchmark configs')
    if not isinstance(config['artifacts'],list) or not config['artifacts']:
        raise BenchmarkError('record defining binaries/fixtures/lockfiles')
    paths = set(); roles = set()
    for item in config['artifacts']:
        if not isinstance(item,dict) or set(item) != {'path','sha256','role'}:
            raise BenchmarkError('invalid artifact descriptor')
        path = expand(item['path'])
        if path in paths:
            raise BenchmarkError('duplicate artifact path')
        paths.add(path)
        if not re.fullmatch('[0-9a-f]{64}',item['sha256']):
            raise BenchmarkError('invalid artifact SHA-256')
        if item['role'] not in ('binary','driver','fixture','generator','lockfile','other'):
            raise BenchmarkError('invalid artifact role')
        roles.add(item['role'])
    if not isinstance(config['correctness_commands'],list) or not config['correctness_commands']:
        raise BenchmarkError('correctness commands required')
    for cmd in config['correctness_commands']:
        argv(cmd)
    ids=set(); commands=set()
    if not isinstance(config['variants'],list) or len(config['variants']) < 2:
        raise BenchmarkError('comparison requires at least two contenders')
    for v in config['variants']:
        if not isinstance(v,dict) or set(v) != {'id','argv'} or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_-]*',v['id']):
            raise BenchmarkError('invalid variant')
        command=tuple(argv(v['argv']))
        if v['id'] in ids or command in commands:
            raise BenchmarkError('duplicate variant identity or command')
        ids.add(v['id']);commands.add(command)
    if config['mode'] == 'application':
        if config['runs'] < 20:
            raise BenchmarkError('application decision runs require at least twenty repetitions')
        if b['profile'] in ('debug','tooling-only','unknown','pending'):
            raise BenchmarkError('application run needs an actual optimized build description')
        if not {'binary','lockfile'} <= roles or not roles.intersection({'fixture','generator'}):
            raise BenchmarkError('application run requires binary, lockfile and fixture/generator artifacts')
        known = {c['id']:c for c in (cases if cases is not None else read_json(PACK/'benchmarks/cases.json'))}
        if config['case_id'] not in known or config['task_id'] not in known[config['case_id']]['activation_tasks']:
            raise BenchmarkError('case/task does not match the relevant workload gate')
        for v in config['variants']:
            exe = Path(argv(v['argv'])[0]).name.lower()
            if exe in ('cargo','cargo.exe','rustc','rustc.exe'):
                raise BenchmarkError('prebuild artifacts; do not time cargo/rustc as application work')
    elif config['case_id'] != 'TOOLING-SMOKE':
        raise BenchmarkError('smoke cannot impersonate an application case')

def artifact_snapshot(config: dict) -> list[dict]:
    cwd=Path(expand(config['cwd'])).resolve()
    if not cwd.is_dir():
        raise BenchmarkError('working directory does not exist')
    result=[]
    for item in config['artifacts']:
        p=Path(expand(item['path']))
        if not p.is_absolute(): p=cwd/p
        p=p.resolve()
        if not p.is_file(): raise BenchmarkError(f'artifact missing: {p}')
        actual=sha256(p)
        if actual != item['sha256']:
            raise BenchmarkError(f'artifact changed: {p}')
        result.append({'path':str(p),'sha256':actual,'bytes':p.stat().st_size,'role':item['role']})
    if config['mode']=='application':
        binaries={item['path'] for item in result if item['role']=='binary'}
        for variant in config['variants']:
            exe=Path(argv(variant['argv'])[0])
            if not exe.is_absolute(): exe=cwd/exe
            if str(exe.resolve()) not in binaries:
                raise BenchmarkError('each timed application executable must be a hashed binary artifact')
    return result

def hyperfine_argv(config: dict, executable: str, output: Path, reverse: bool = False) -> list[str]:
    variants=list(reversed(config['variants'])) if reverse else config['variants']
    args=[executable,'--shell=none','--warmup',str(config['warmup']),'--runs',str(config['runs']), '--export-json',str(output/'hyperfine.json'),'--export-markdown',str(output/'hyperfine.md')]
    for v in variants:
        args += ['--command-name',v['id'],shlex.join(argv(v['argv']))]
    return args
