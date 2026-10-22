#!/usr/bin/env python3
"""Compare bounded stroke preparation with frozen real CLI and union baselines."""
import argparse
import copy
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import sys
import zipfile


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--baseline', type=Path, required=True)
    p.add_argument('--baseline-features', type=Path, required=True)
    p.add_argument('--candidate', type=Path, required=True)
    p.add_argument('--kernel', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--execute', action='store_true')
    a = p.parse_args()
    if not a.execute:
        print('Plan only: verify frozen artifacts, then run 160 Criterion samples and three CLI sessions; no files written.')
        return
    root = Path.cwd()
    out = a.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    base = json.loads(a.baseline.read_text())
    binaries = {'baseline': Path(base['binary']), 'checked': a.candidate.resolve()}
    assert sha(binaries['baseline']) == base['binary_sha256']
    feature_tree = subprocess.check_output(['cargo', 'tree', '--locked', '-p', 'paintcraft-cli', '-e', 'features'], text=True)
    assert 'wgpu' not in feature_tree and 'wgpu' not in a.baseline_features.read_text()
    assert feature_tree == a.baseline_features.read_text(), 'CLI feature graphs differ'
    (out/'baseline-features.log').write_bytes(a.baseline_features.read_bytes())
    (out/'checked-features.log').write_text(feature_tree)
    fixtures = out/'fixtures'
    fixtures.mkdir()
    previous = root/'benchmarks/results/paint-plan-cli-cpu-2026-10-21/fixtures'
    native = fixtures/'input.pcnd'
    native.write_bytes(gzip.decompress((previous/'input.pcnd.gz').read_bytes()))
    input_sha = sha(native)
    cases = {}
    brush = json.loads((previous/'smooth_brush.json').read_text())
    cases['pencil'] = json.loads((previous/'horizontal_pencil.json').read_text())
    for case, style, size in [('square', 'square', 16), ('round', 'round', 32), ('retrace', 'round', 32), ('smooth-control', 'smooth-round', 32)]:
        batch = copy.deepcopy(brush)
        for op in batch['operations']:
            op.update(brush=style, size=size)
            if case == 'retrace':
                op['points'] = [{'x': x, 'y': 400} for x in [0, 999, 0, 999, 0]]
        cases[case] = batch
    for case, batch in cases.items():
        (fixtures/f'{case}.json').write_text(json.dumps(batch, separators=(',', ':'))+'\n')
    sources = sorted({Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'),
                      Path(__file__).resolve().relative_to(root), *Path('crates').rglob('*.rs'),
                      *Path('crates').rglob('*.wgsl'), *Path('crates').rglob('Cargo.toml'),
                      *Path('apps').rglob('*.rs'), *Path('apps').rglob('Cargo.toml')})
    (out/'baseline-manifest.json').write_text(json.dumps(base, indent=2)+'\n')
    manifest = {'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                'platform': platform.platform(), 'baseline': {k:v for k,v in base.items() if k != 'sources_sha256'},
                'baseline_manifest_sha256': sha(out/'baseline-manifest.json'),
                'binaries': {k: {'path': str(v), 'sha256': sha(v)} for k,v in binaries.items()},
                'kernel': {'path': str(a.kernel.resolve()), 'sha256': sha(a.kernel)},
                'sources_sha256': {str(x): sha(x) for x in sources},
                'feature_tree_sha256': hashlib.sha256(feature_tree.encode()).hexdigest(),
                'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
                'hyperfine': subprocess.check_output(['hyperfine', '--version'], text=True).strip(),
                'input_sha256': input_sha, 'case_sha256': {c: sha(fixtures/f'{c}.json') for c in cases},
                'runs': 20, 'warmups': 3, 'sessions': 3, 'operations_per_batch': 100,
                'kernel_cases': ['tiny', 'square', 'round', 'retrace'],
                'scope': 'Constant-colour once-per-stroke union with bounded cancellable preparation. Kernel setup excluded, construction/result destruction included; real CPU-only CLI reads/hash-checks input, executes/publishes batch/history, encodes PCND and atomically replaces output. Not native input/display/cancellation tails or physical all-service budgets.',
                'environment': 'Local macOS developer host; no affinity/power/thermal controls. No intentionally concurrent task builds/tests/measurements. Keep slow samples, controls and calibration; no subtraction.'}
    (out/'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
    results = out/'correctness'
    results.mkdir()
    commands = {}
    for case in cases:
        commands[case] = {mode: [str(binary), '--json', 'apply', str(native), '--operations', str(fixtures/f'{case}.json'),
                                 '--output', str(results/f'{mode}-{case}.pcnd'), '--overwrite', '--expect-input-sha256', input_sha]
                          for mode,binary in binaries.items()}

    def identities():
        assert all(sha(p) == manifest['binaries'][k]['sha256'] for k,p in binaries.items())
        assert sha(a.kernel) == manifest['kernel']['sha256']
        assert {str(x): sha(x) for x in sources} == manifest['sources_sha256']
        assert sha(native) == input_sha
        assert all(sha(fixtures/f'{c}.json') == manifest['case_sha256'][c] for c in cases)

    def verify(label):
        data = {}
        for case in cases:
            for mode, command in commands[case].items():
                with (out/f'correctness-{label}-{case}-{mode}.log').open('wb') as log:
                    subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
            paths = [results/f'{mode}-{case}.pcnd' for mode in binaries]
            assert paths[0].read_bytes() == paths[1].read_bytes(), case
            with zipfile.ZipFile(paths[0]) as first, zipfile.ZipFile(paths[1]) as second:
                assert first.namelist() == second.namelist()
                assert all(first.read(name) == second.read(name) for name in first.namelist())
            data[case] = {'archive_sha256': sha(paths[0]), 'byte_identical': True}
        if label != 'before':
            assert data == json.loads((out/'correctness-before.json').read_text()), 'output changed across sessions'
        (out/f'correctness-{label}.json').write_text(json.dumps(data, indent=2)+'\n')
        identities()

    verify('before')
    print('CLI preflight complete', flush=True)
    env = dict(os.environ, CRITERION_HOME=str(out/'criterion'))
    with (out/'criterion.log').open('wb') as log:
        subprocess.run([str(a.kernel.resolve()), '--bench', '--noplot'], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    samples = list((out/'criterion').glob('stroke_union/*/*/new/sample.json'))
    assert len(samples) == 8, [str(x) for x in samples]
    assert all(len(json.loads(x.read_text())['times']) == 20 for x in samples)
    print('160 raw kernel samples complete', flush=True)

    def hyperfine(session, case, variants):
        if case == 'calibration':
            cli = {mode: [str(binaries[mode]), '--version'] for mode in variants}
        else:
            cli = commands[case]
        path = out/f'hyperfine-{session}-{case}.json'
        with path.with_suffix('.log').open('wb') as log:
            subprocess.run(['hyperfine', '--shell=none', '--warmup', '3', '--runs', '20', '--export-json', str(path),
                            *[shlex.join(cli[v]) for v in variants]], stdout=log, stderr=subprocess.STDOUT, check=True)
        raw = path.read_bytes()
        compact = (json.dumps(json.loads(raw), separators=(',', ':'))+'\n').encode()
        path.with_suffix('.json.gz').write_bytes(gzip.compress(raw, mtime=0))
        path.write_bytes(compact)
        path.with_suffix('.identity.json').write_text(json.dumps({'original_sha256': hashlib.sha256(raw).hexdigest(),
                                                                 'compact_sha256': hashlib.sha256(compact).hexdigest(),
                                                                 'equal_json_values': json.loads(raw) == json.loads(compact)}, indent=2)+'\n')
    for session in range(1,4):
        order = list(binaries) if session != 2 else list(reversed(binaries))
        for case in cases:
            hyperfine(session, case, order)
            print('session', session, case, 'complete', flush=True)
        hyperfine(session, 'calibration', order)
        verify(f'after-{session}')
    identities()
    print('All correctness postflights and artifact identities verified', flush=True)


if __name__ == '__main__':
    main()
