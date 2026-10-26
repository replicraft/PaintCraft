#!/usr/bin/env python3
"""Measure prebuilt optimized ASE artifacts. Never build, install, or remove data."""
import argparse
import csv
import hashlib
import json
import os
import platform
import re
import shlex
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CASES = ('empty-0', 'mixed-6', 'grouped-256', 'grouped-4096')
BENCH_ENV = ('PAINTCRAFT_PALETTE_FIXTURES', 'PAINTCRAFT_PALETTE_SAMPLES', 'PAINTCRAFT_PALETTE_REVERSE')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_receipt(path, executable):
    for line in path.read_text().splitlines():
        row = json.loads(line)
        if row.get('reason') == 'compiler-artifact' and row.get('executable'):
            if Path(row['executable']).resolve() == executable.resolve():
                profile = row['profile']
                if profile['opt_level'] != '3' or profile['debug_assertions']:
                    raise ValueError('optimized release artifacts are required')
                return profile
    raise ValueError(f'no matching artifact in {path}')


def run(argv, log, env=None):
    with log.open('x') as output:
        subprocess.run(argv, cwd=ROOT, env=env, stdout=output,
                       stderr=subprocess.STDOUT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('bench', 'samples', 'cli', 'build-jsonl', 'cli-build-jsonl', 'oracle-python', 'output'):
        parser.add_argument('--' + option, type=Path, required=True)
    parser.add_argument('--execute', action='store_true', help='Default is a read-only plan.')
    args = parser.parse_args()
    profiles = {'bench': build_receipt(args.build_jsonl, args.bench),
                'samples': build_receipt(args.build_jsonl, args.samples),
                'cli': build_receipt(args.cli_build_jsonl, args.cli)}
    output = args.output.resolve()
    if not args.execute:
        print(json.dumps({'output': str(output), 'profiles': profiles,
                          'sessions': ['forward', 'reverse', 'forward'],
                          'runs': 20, 'warmups': 3, 'divan_samples': 30,
                          'iterations_per_sample': 16, 'criterion_samples': 100}, indent=4))
        return
    output.mkdir(parents=True, exist_ok=False)
    (output / 'bin').mkdir()
    for source, name in [(args.bench, 'palette_assets'), (args.samples, 'palette_samples'), (args.cli, 'paintcraft-cli')]:
        shutil.copy2(source, output / 'bin' / name)
        (output / 'bin' / name).chmod(0o555)
    shutil.copy2(args.build_jsonl, output / 'build.jsonl')
    shutil.copy2(args.cli_build_jsonl, output / 'cli-build.jsonl')
    bench = output / 'bin/palette_assets'
    cli = output / 'bin/paintcraft-cli'
    statistical = output / 'bin/palette_samples'
    env = {key: value for key, value in os.environ.items() if key not in BENCH_ENV}
    fixture_env = {**env, 'PAINTCRAFT_PALETTE_FIXTURES': str(output / 'fixtures')}
    run([str(bench), '--test'], output / 'correctness.log', fixture_env)
    oracle = ROOT / 'crates/services/paintcraft-formats/tests/ase-fixtures/benchmark_oracle.py'
    oracle_args = [str(args.oracle_python.absolute()), str(oracle), '--fixtures', str(output / 'fixtures')]
    run(oracle_args, output / 'independent-fixture-oracle.json')
    sample_env = {**env, 'PAINTCRAFT_PALETTE_FIXTURES': str(output / 'criterion-fixtures'),
                  'CRITERION_HOME': str(output / 'criterion-preflight')}
    run([str(statistical), '--test'], output / 'criterion-correctness.log', sample_env)
    for case in CASES:
        assert (output / 'criterion-fixtures' / (case + '.ase')).read_bytes() == (output / 'fixtures' / (case + '.ase')).read_bytes()

    (output / 'exports').mkdir()
    commands = []
    for case in CASES:
        source = output / 'fixtures' / (case + '.ase')
        destination = output / 'exports' / (case + '.ase')
        for operation in ('inspect', 'export'):
            argv = [str(cli), 'palette', operation, str(source), '--json']
            if operation == 'export':
                argv += ['-o', str(destination), '--overwrite']
            run(argv, output / f'preflight-{case}-{operation}.json')
            commands.append({'name': case + '-' + operation, 'argv': argv})
    run(oracle_args + ['--exports', str(output / 'exports')], output / 'independent-cli-oracle.json')
    (output / 'commands.json').write_text(json.dumps(commands, indent=4) + '\n')
    tracked = [bench, cli, statistical, ROOT / 'Cargo.lock', Path(__file__), oracle, oracle.with_name('oracle.py'),
               ROOT / 'crates/services/paintcraft-formats/benches/palette_samples.rs',
               ROOT / 'crates/services/paintcraft-formats/benches/palette_assets/fixtures.rs',
               ROOT / 'crates/services/paintcraft-formats/benches/palette_assets.rs']
    metadata = {'platform': platform.platform(), 'machine': platform.machine(),
                'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
                'hyperfine': subprocess.check_output(['hyperfine', '--version'], text=True),
                'git_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                'profiles': profiles, 'sha256': {str(p): digest(p) for p in tracked},
                'environment_scope': 'No task builds during measurements. Other user workload, power and thermal state uncontrolled.'}
    (output / 'environment.json').write_text(json.dumps(metadata, indent=4) + '\n')
    for session in range(1, 4):
        reverse = session == 2
        sample_env = {**env, 'PAINTCRAFT_PALETTE_SAMPLES': str(output / f'raw-{session}.csv')}
        if reverse:
            sample_env['PAINTCRAFT_PALETTE_REVERSE'] = '1'
        argv = [str(bench), '--bench', '--sample-count', '30', '--sample-size', '16',
                '--color', 'never', '--sortr' if reverse else '--sort', 'name']
        run(argv, output / f'divan-{session}.txt', sample_env)
        text = (output / f'divan-{session}.txt').read_text()
        rows = [line for line in text.splitlines() if re.search(r'(empty-0|mixed-6|grouped-256|grouped-4096)\s+\d', line)]
        assert len(rows) == 8, 'Divan must produce statistics, not just a test/list inventory'
        assert all([int(part.strip()) for part in row.split('│')[-2:]] == [30, 480] for row in rows)
        with (output / f'raw-{session}.csv').open() as source:
            samples = list(csv.DictReader(source))
        assert len(samples) == 240 and all(int(row['iterations']) == 16 for row in samples)
        statistical_env = {**env, 'CRITERION_HOME': str(output / f'criterion-{session}')}
        if reverse:
            statistical_env['PAINTCRAFT_PALETTE_REVERSE'] = '1'
        run([str(statistical), '--bench', '--noplot'], output / f'criterion-{session}.log', statistical_env)
        sample_files = list((output / f'criterion-{session}').rglob('new/sample.json'))
        assert len(sample_files) == 8
        for path in sample_files:
            raw = json.loads(path.read_text())
            assert raw['sampling_mode'] == 'Flat' and len(raw['times']) == len(raw['iters']) == 100
            assert all(t > 0 and n > 0 for t, n in zip(raw['times'], raw['iters']))
        argv = ['hyperfine', '--shell=none', '--runs', '20', '--warmup', '3',
                '--export-json', str(output / f'hyperfine-{session}.json'),
                '--export-markdown', str(output / f'hyperfine-{session}.md')]
        for command in reversed(commands) if reverse else commands:
            argv += ['--command-name', command['name'], shlex.join(command['argv'])]
        run(argv, output / f'hyperfine-{session}.log')
        results = json.loads((output / f'hyperfine-{session}.json').read_text())['results']
        assert len(results) == 8
        for result in results:
            if 'measurements' in result:
                assert len(result['measurements']) == 20
                assert all(row['exit_code'] == 0 for row in result['measurements'])
            else:
                assert len(result['times']) == 20 and all(code == 0 for code in result['exit_codes'])
        print(f'Session {session} complete', flush=True)
    for path in tracked:
        assert digest(path) == metadata['sha256'][str(path)], f'artifact changed: {path}'
    run(oracle_args + ['--exports', str(output / 'exports')], output / 'postflight-oracle.json')
    print(output)


if __name__ == '__main__':
    main()
