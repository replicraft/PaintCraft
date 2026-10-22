#!/usr/bin/env python3
"""Measure shared-metadata overhead with frozen library and CPU-only CLI artifacts."""
import argparse
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import zipfile


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('baseline-cli','candidate-cli','baseline-library','candidate-library','output'):
        parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--execute',action='store_true')
    args=parser.parse_args()
    if not args.execute:
        print('Plan only: 480 library statistical samples and 1200 process observations; no files written.')
        return
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    baseline=json.loads(args.baseline_cli.read_text());oldlib=json.loads(args.baseline_library.read_text())
    cli={'baseline':Path(baseline['binary']),'charged':args.candidate_cli.resolve()}
    library={'baseline':Path(oldlib['binary']),'charged':args.candidate_library.resolve()}
    assert sha(cli['baseline'])==baseline['binary_sha256'];assert sha(library['baseline'])==oldlib['binary_sha256']
    (out/'baseline-cli-manifest.json').write_bytes(args.baseline_cli.read_bytes())
    (out/'baseline-library-manifest.json').write_bytes(args.baseline_library.read_bytes())
    feature_tree=subprocess.check_output(['cargo','tree','--locked','-p','paintcraft-cli','-e','features'],text=True)
    assert 'wgpu' not in feature_tree
    assert hashlib.sha256(feature_tree.encode()).hexdigest()==baseline['feature_tree_sha256']
    (out/'cli-features.log').write_text(feature_tree)
    sources=sorted({Path('Cargo.toml'),Path('Cargo.lock'),Path('rust-toolchain.toml'),Path(__file__).resolve().relative_to(Path.cwd()),*Path('apps').rglob('*.rs'),*Path('apps').rglob('Cargo.toml'),*Path('crates').rglob('*.rs'),*Path('crates').rglob('*.wgsl'),*Path('crates').rglob('Cargo.toml')})
    fixtures=out/'fixtures';fixtures.mkdir()
    previous=Path('benchmarks/results/paint-plan-cli-cpu-2026-10-21/fixtures')
    native=fixtures/'input.pcnd';native.write_bytes(gzip.decompress((previous/'input.pcnd.gz').read_bytes()))
    cases=['horizontal_pencil','diagonal_pencil','smooth_brush','smooth_eraser']
    for case in cases:(fixtures/(case+'.json')).write_bytes((previous/(case+'.json')).read_bytes())
    results=out/'correctness';results.mkdir()
    commands={case:{mode:[str(binary),'--json','apply',str(native),'--operations',str(fixtures/(case+'.json')),'--output',str(results/(mode+'-'+case+'.pcnd')),'--overwrite','--expect-input-sha256',sha(native)] for mode,binary in cli.items()} for case in cases}
    manifest={'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'platform':platform.platform(),'rustc':subprocess.check_output(['rustc','-Vv'],text=True),'hyperfine':subprocess.check_output(['hyperfine','--version'],text=True).strip(),'sources_sha256':{str(p):sha(p) for p in sources},'binaries':{kind:{mode:{'path':str(p),'sha256':sha(p)} for mode,p in binaries.items()} for kind,binaries in [('cli',cli),('library',library)]},'baseline_manifest_sha256':{'cli':sha(out/'baseline-cli-manifest.json'),'library':sha(out/'baseline-library-manifest.json')},'fixture_sha256':{str(p.relative_to(out)):sha(p) for p in fixtures.iterdir()},'feature_tree_sha256':hashlib.sha256(feature_tree.encode()).hexdigest(),'runs':20,'warmups':3,'sessions':3,'scope':'Shared descriptor/index resource ownership overhead. Same required canonical CPU CLI output; library real paint/flood primitives. Not editor whole-batch integration, physical RSS ceilings or native event/cancellation latency. Library process comparisons exercise combined programs only; all three modes retain statistical samples.','environment':'Uncontrolled local macOS host; no affinity/power/thermal controls; no intentional concurrent task builds/tests/timing. Preserve slow controls and calibration without subtraction.'}
    (out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    def identities():
        for binaries in manifest['binaries'].values():
            for entry in binaries.values():assert sha(entry['path'])==entry['sha256']
        assert all(sha(p)==value for p,value in manifest['sources_sha256'].items())
    checks={}
    def verify(label):
        identities();record={'cli':{},'library':{}}
        for mode,binary in library.items():
            raw=subprocess.check_output([str(binary),'verify'],stderr=subprocess.STDOUT);(out/f'library-{label}-{mode}.log').write_bytes(raw);record['library'][mode]=hashlib.sha256(raw).hexdigest()
        assert len(set(record['library'].values()))==1
        for case in cases:
            for mode in cli:
                with (out/f'cli-{label}-{case}-{mode}.log').open('wb') as log:subprocess.run(commands[case][mode],stdout=log,stderr=subprocess.STDOUT,check=True)
            a=results/('baseline-'+case+'.pcnd');b=results/('charged-'+case+'.pcnd');assert a.read_bytes()==b.read_bytes()
            with zipfile.ZipFile(a) as x,zipfile.ZipFile(b) as y:
                assert x.namelist()==y.namelist();assert all(x.read(n)==y.read(n) for n in x.namelist())
            record['cli'][case]=sha(a)
        checks[label]=record
        assert all(value==record for value in checks.values())
        (out/'correctness.json').write_text(json.dumps(checks,indent=2)+'\n')
    verify('before')
    for mode,binary in library.items():
        with (out/f'criterion-{mode}.log').open('wb') as log:subprocess.run([str(binary),'--bench','--noplot'],env=dict(os.environ,CRITERION_HOME=str(out/('criterion-'+mode))),stdout=log,stderr=subprocess.STDOUT,check=True)
        samples=list((out/('criterion-'+mode)).glob('paint_projection/*/*/new/sample.json'))
        assert len(samples)==12 and all(len(json.loads(p.read_text())['times'])==20 for p in samples)
    print('480 library statistical samples complete',flush=True)
    def hyperfine(session,kind,case,order):
        binaries=cli if kind=='cli' else library
        if case=='calibration':cmd={mode:[str(binaries[mode]),'--version' if kind=='cli' else 'calibration'] for mode in order}
        elif kind=='cli':cmd=commands[case]
        else:cmd={mode:[str(binaries[mode]),'process',case,'combined'] for mode in order}
        path=out/f'hyperfine-{session}-{kind}-{case}.json'
        with path.with_suffix('.log').open('wb') as log:subprocess.run(['hyperfine','--shell=none','--warmup','3','--runs','20','--export-json',str(path),*[shlex.join(cmd[mode]) for mode in order]],stdout=log,stderr=subprocess.STDOUT,check=True)
        raw=path.read_bytes();data=json.loads(raw)
        for result in data['results']:assert len(result['measurements'])==20 and all(x['exit_code']==0 for x in result['measurements'])
        compact=(json.dumps(data,separators=(',',':'))+'\n').encode();path.with_suffix('.json.gz').write_bytes(gzip.compress(raw,mtime=0));path.write_bytes(compact)
        path.with_suffix('.identity.json').write_text(json.dumps({'original_sha256':hashlib.sha256(raw).hexdigest(),'compact_sha256':sha(path),'equal_json_values':json.loads(raw)==json.loads(compact)},indent=2)+'\n')
    for session in range(1,4):
        order=['baseline','charged'] if session!=2 else ['charged','baseline']
        for case in cases:hyperfine(session,'cli',case,order);print('session',session,'cli',case,flush=True)
        for case in ['tiny','wall','overlap','default-control']:hyperfine(session,'library',case,order);print('session',session,'library',case,flush=True)
        for kind in ('cli','library'):hyperfine(session,kind,'calibration',order)
        verify('after-'+str(session))
    identities();print('All output postflights and artifact identities verified',flush=True)


if __name__=='__main__':main()
