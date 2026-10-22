#!/usr/bin/env python3
"""Compare real projected and materialized paint/flood preparation; plan-only by default."""
import argparse
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import statistics
import subprocess


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    if not args.execute:
        print('Plan only: 240 Criterion samples and three 260-observation process sessions; no files written.')
        return
    binary = args.binary.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    paths = sorted({Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'),
                    Path(__file__).resolve().relative_to(Path.cwd()),
                    *Path('crates').rglob('*.rs'), *Path('crates').rglob('*.wgsl'),
                    *Path('crates').rglob('Cargo.toml'), *Path('apps').rglob('*.rs'),
                    *Path('apps').rglob('Cargo.toml')})
    manifest = {'source_commit': subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip(),
                'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                'platform': platform.platform(), 'binary': str(binary), 'binary_sha256': sha(binary),
                'sources_sha256': {str(p):sha(p) for p in paths},
                'rustc': subprocess.check_output(['rustc','-Vv'], text=True),
                'hyperfine': subprocess.check_output(['hyperfine','--version'], text=True).strip(),
                'cases': {'tiny': {'dimensions':[8,8], 'plans':1, 'process_repeats':10000},
                          'wall': {'dimensions':[512,64], 'plans':1, 'process_repeats':100},
                          'overlap': {'dimensions':[128,64], 'plans':64, 'process_repeats':100},
                          'default-control': {'dimensions':[512,64], 'plans':1, 'process_repeats':100}},
                'runs':20, 'warmups':3, 'sessions':3, 'variants':['materialized','projected','combined'],
                'scope':'Plan construction plus exact flood classification and destruction. Same-source real library paths, not editor whole-batch integration, CLI file IO, native input/display/cancellation tails or physical all-service budgets.',
                'environment':'Local uncontrolled macOS host; no affinity/power/thermal controls. No intentional concurrent task builds/tests/timing. Preserve all samples and calibration without subtraction.'}
    (output/'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
    def identities():
        assert sha(binary)==manifest['binary_sha256']
        assert all(sha(p)==v for p,v in manifest['sources_sha256'].items())
    checks = {}
    def verify(label):
        identities()
        raw = subprocess.check_output([str(binary),'verify'], stderr=subprocess.STDOUT)
        (output/f'correctness-{label}.log').write_bytes(raw)
        checks[label] = hashlib.sha256(raw).hexdigest()
        assert len(set(checks.values()))==1, 'correctness/resource counters changed'
        (output/'correctness.json').write_text(json.dumps(checks, indent=2)+'\n')
    verify('before')
    with (output/'criterion.log').open('wb') as log:
        subprocess.run([str(binary),'--bench','--noplot'], env=dict(os.environ,CRITERION_HOME=str(output/'criterion')), stdout=log, stderr=subprocess.STDOUT, check=True)
    samples = list((output/'criterion').glob('paint_projection/*/*/new/sample.json'))
    assert len(samples)==12 and all(len(json.loads(p.read_text())['times'])==20 for p in samples)
    print('240 statistical samples complete', flush=True)
    summaries = []
    for session in range(1,4):
        order = manifest['variants'] if session != 2 else list(reversed(manifest['variants']))
        for case in [*manifest['cases'],'calibration']:
            commands = [shlex.join([str(binary),'calibration'])] if case=='calibration' else [shlex.join([str(binary),'process',case,mode]) for mode in order]
            # Calibration uses the same executable once per session (20 observations).
            path = output/f'hyperfine-{session}-{case}.json'
            with path.with_suffix('.log').open('wb') as log:
                subprocess.run(['hyperfine','--shell=none','--warmup','3','--runs','20','--export-json',str(path),*commands],stdout=log,stderr=subprocess.STDOUT,check=True)
            raw=path.read_bytes(); data=json.loads(raw)
            compact=(json.dumps(data,separators=(',',':'))+'\n').encode()
            path.with_suffix('.json.gz').write_bytes(gzip.compress(raw,mtime=0));path.write_bytes(compact)
            path.with_suffix('.identity.json').write_text(json.dumps({'original_sha256':hashlib.sha256(raw).hexdigest(),'compact_sha256':sha(path),'equal_json_values':json.loads(raw)==json.loads(compact)},indent=2)+'\n')
            rows=[]
            for result in data['results']:
                observations=result['measurements']
                assert len(observations)==20 and all(x['exit_code']==0 for x in observations)
                rows.append({'command':result['command'],'mean_ms':statistics.mean(x['time_wall_clock']['value'] for x in observations)*1000,'median_rss_mib':statistics.median(x['memory_peak_resident']['value'] for x in observations)/(1024*1024)})
            summaries.append({'session':session,'case':case,'results':rows})
            print('session',session,case,'complete',flush=True)
        verify(f'after-{session}')
    identities()
    (output/'summary.json').write_text(json.dumps({'statistical_samples':240,'workload_observations':720,'calibration_observations':60,'process':summaries},indent=2)+'\n')
    print('All correctness postflights and identities verified',flush=True)


if __name__ == '__main__':
    main()
