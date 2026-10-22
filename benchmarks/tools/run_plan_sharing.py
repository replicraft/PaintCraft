#!/usr/bin/env python3
"""Compare pre-sharing and shared planned execution, including genuine cache misses."""
import argparse
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import subprocess

from run_batch_preparation import CASES, semantics


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ['baseline-manifest', 'candidate-artifacts', 'output']:
        parser.add_argument('--' + key, type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--preflight-only', action='store_true')
    args = parser.parse_args()
    if not args.execute:
        print('Plan only: 480 statistical samples, 3120 process observations; no files written.')
        return
    old = json.loads(args.baseline_manifest.read_text())
    new = json.loads(args.candidate_artifacts.read_text())
    binaries = {'library': {'before': old['library'], 'shared': new['library']},
                'cli': {'before': old['cli']['planned'], 'shared': new['cli']}}
    for modes in binaries.values():
        for entry in modes.values():
            assert sha(entry['path']) == entry['sha256']
    features = subprocess.check_output(['cargo', 'tree', '--locked', '-p', 'paintcraft-cli', '-e', 'features'], text=True)
    assert 'wgpu' not in features
    assert hashlib.sha256(features.encode()).hexdigest() == old['feature_tree_sha256']
    out = args.output.resolve(); out.mkdir(exist_ok=False, parents=True)
    save(out / 'baseline-manifest.json', old)
    save(out / 'candidate-artifacts.json', new)
    (out / 'cli-features.log').write_text(features)
    root = args.baseline_manifest.parent
    archived = {r['original']: r for r in json.loads((root / 'compressed-artifacts.json').read_text())}
    fixtures = {}
    for relative, expected in old['fixtures_sha256'].items():
        if relative in archived:
            entry = archived[relative]; packed = (root / entry['archive']).read_bytes()
            assert hashlib.sha256(packed).hexdigest() == entry['archive_sha256']
            raw = gzip.decompress(packed)
        else:
            raw = (root / relative).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == expected
        fixtures[Path(relative).name] = raw
    datasets = {'repeated': CASES, 'varied': [c for c in CASES if c not in ['mixed_lineage', 'default_control']]}
    for dataset in datasets:
        folder = out / dataset; folder.mkdir()
        for name, raw in fixtures.items():
            if dataset == 'varied' and name.removesuffix('.json') in datasets[dataset]:
                batch = json.loads(raw)
                for index, op in enumerate(batch['operations']):
                    # Change actual descriptor values on every consecutive command,
                    # retaining geometry and alpha/coverage/blend work. No cache hit.
                    color = op['color']; op['color'] = f'#{(17+index)%256:02X}{(88+index*3)%256:02X}{(156+index*7)%256:02X}' + color[-2:]
                raw = (json.dumps(batch, indent=2) + '\n').encode()
            (folder / name).write_bytes(raw)
    sources = sorted({Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'),
                      Path(__file__).resolve().relative_to(Path.cwd()), Path('benchmarks/tools/run_batch_preparation.py'),
                      Path('crates/editing/paintcraft-editor/benches/support/sequential-manifest.json'),
                      *Path('crates').rglob('*.rs'), *Path('crates').rglob('Cargo.toml'),
                      *Path('apps').rglob('*.rs'), *Path('apps').rglob('Cargo.toml')})
    manifest = {'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'platform': platform.platform(),
                'sources_sha256': {str(p): sha(p) for p in sources}, 'binaries': binaries,
                'fixtures_sha256': {str(p.relative_to(out)): sha(p) for dataset in datasets for p in (out / dataset).iterdir()},
                'feature_tree_sha256': hashlib.sha256(features.encode()).hexdigest(), 'datasets': datasets,
                'runs': 20, 'warmups': 3, 'sessions': 3, 'statistical_samples': 480, 'process_observations': 3120,
                'preflight_only': args.preflight_only, 'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
                'hyperfine': subprocess.check_output(['hyperfine', '--version'], text=True).strip(),
                'scope': 'Before/shared planned editor execution and CPU CLI apply/history/file IO; repeated geometry and varied descriptor miss controls. Not native interaction latency or physical memory ceilings.',
                'environment': 'Uncontrolled local host, no affinity/power/thermal controls; no intentional concurrent task builds/tests/timing. Preserve raw samples, controls and calibration without subtraction.'}
    save(out / 'manifest.json', manifest)
    def command(label, argv, env):
        with (out / (label + '.log')).open('wb') as log:
            subprocess.run(argv, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    def env_for(dataset):
        return dict(os.environ, PAINTCRAFT_BATCH_FIXTURES=str(out / dataset))
    def cli(dataset, case, mode):
        folder = out / dataset
        return [binaries['cli'][mode]['path'], 'apply', str(folder / ('dense.pcnd' if case == 'dense' else 'input.pcnd')),
                '--operations', str(folder / (case + '.json')), '--overwrite',
                '--expect-input-sha256', sha(folder / ('dense.pcnd' if case == 'dense' else 'input.pcnd')), '--output', str(out / ('output-' + dataset + '-' + case + '-' + mode + '.pcnd'))]
    # Validate the actual CLI schema before creating measured commands.
    command('cli-help', [binaries['cli']['before']['path'], 'apply', '--help'], dict(os.environ))
    expected = {}; checks = {}
    def verify(label):
        record = {}
        for dataset, cases in datasets.items():
            env = env_for(dataset); rows = {}
            for mode in binaries['library']:
                name = label + '-' + dataset + '-library-' + mode
                command(name, [binaries['library'][mode]['path'], 'verify'], env)
                parsed = {}
                for line in (out / (name + '.log')).read_text().splitlines():
                    match = re.fullmatch(r'(\w+) (\w+) sha256=([a-f0-9]{64}) decoded_peak=(\d+) metadata_peak=(\d+) operations=(\d+) repeats=(\d+)', line)
                    assert match, line
                    case, execution, checksum, decoded, metadata, count, repeats = match.groups()
                    assert case in CASES and execution in ['sequential', 'planned']
                    key = (dataset, case); assert expected.setdefault(key, checksum) == checksum
                    parsed[(case, execution)] = {'sha256': checksum, 'decoded': int(decoded), 'metadata': int(metadata), 'operations': int(count), 'repeats': int(repeats)}
                assert len(parsed) == 14
                rows[mode] = parsed
            for case in cases:
                a = rows['before'][(case, 'planned')]; b = rows['shared'][(case, 'planned')]
                assert a['sha256'] == b['sha256'] and a['decoded'] == b['decoded']
                for mode in binaries['cli']: command(label + '-' + dataset + '-' + case + '-cli-' + mode, cli(dataset, case, mode), env)
                paths = [Path(cli(dataset, case, mode)[-1]) for mode in binaries['cli']]
                assert paths[0].read_bytes() == paths[1].read_bytes()
                summary = json.dumps(semantics(paths[0]), sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
                assert hashlib.sha256(summary).hexdigest() == a['sha256']
                record[dataset + '/' + case] = {'library': {'before': a, 'shared': b}, 'cli_sha256': sha(paths[0])}
        checks[label] = record; assert all(v == record for v in checks.values())
        save(out / 'correctness.json', checks)
        for modes in binaries.values():
            for entry in modes.values(): assert sha(entry['path']) == entry['sha256']
        assert all(sha(p) == h for p, h in manifest['sources_sha256'].items())
        assert all(sha(out / p) == h for p, h in manifest['fixtures_sha256'].items())
        print(label, 'exact outputs and identities verified', flush=True)
    verify('before')
    if args.preflight_only:
        return
    for dataset, cases in datasets.items():
        for mode in binaries['library']:
            home = out / ('criterion-' + dataset + '-' + mode)
            pattern = '(' + '|'.join(cases) + ')/planned$'
            command('criterion-' + dataset + '-' + mode,
                    [binaries['library'][mode]['path'], '--bench', '--noplot', pattern],
                    dict(env_for(dataset), CRITERION_HOME=str(home)))
            samples = list(home.glob('batch_preparation/*/planned/new/sample.json'))
            assert len(samples) == len(cases) and all(len(json.loads(p.read_text())['times']) == 20 for p in samples)
    print('480 statistical samples complete', flush=True)
    for session in range(1, 4):
        order = ['before', 'shared'] if session != 2 else ['shared', 'before']
        for dataset, cases in datasets.items():
            for kind in binaries:
                for case in cases:
                    commands = [cli(dataset, case, mode) if kind == 'cli' else [binaries[kind][mode]['path'], 'process', case, 'planned'] for mode in order]
                    measure(out / f'hyperfine-{session}-{dataset}-{kind}-{case}.json', commands, env_for(dataset))
        for kind in binaries:
            commands = [[binaries[kind][mode]['path'], '--version' if kind == 'cli' else 'calibration'] for mode in order]
            measure(out / f'hyperfine-{session}-{kind}-calibration.json', commands, dict(os.environ))
        verify('after-' + str(session))
        print('session', session, 'complete', flush=True)
    print('All measurements and postflights complete', flush=True)


def measure(path, commands, env):
    with path.with_suffix('.log').open('wb') as log:
        subprocess.run(['hyperfine', '--shell=none', '--warmup', '3', '--runs', '20', '--export-json', str(path),
                        *[shlex.join(cmd) for cmd in commands]], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
    raw = path.read_bytes(); data = json.loads(raw)
    assert len(data['results']) == 2 and all(len(r['measurements']) == 20 and all(m['exit_code'] == 0 for m in r['measurements']) for r in data['results'])
    compact = (json.dumps(data, separators=(',', ':')) + '\n').encode()
    path.with_suffix('.json.gz').write_bytes(gzip.compress(raw, mtime=0)); path.write_bytes(compact)
    save(path.with_suffix('.identity.json'), {'original_sha256': hashlib.sha256(raw).hexdigest(), 'compact_sha256': sha(path), 'equal_json_values': json.loads(raw) == json.loads(compact)})


if __name__ == '__main__':
    main()
