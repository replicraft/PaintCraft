"""Plan or explicitly measure prebuilt qualified CPU-region artifacts; never build/install."""
import argparse
import csv
import json
import math
import os
import shutil
from pathlib import Path
import sys

sys.dont_write_bytecode = True
from benchmark_support import artifact_snapshot, sha256, validate_config
from run_hyperfine import execute, run_logged

ROOT = Path(__file__).resolve().parents[2]
CASES = ['default-multiply', 'default-screen', '1080p-multiply', '1080p-screen']
CASE_ID = 'BLEND-CPU-OPAQUE-V1'


def disk_floor():
    if shutil.disk_usage(ROOT).free < 300 << 20:
        raise RuntimeError('below300MiB phase floor; stop without cleanup/retry')


def write(path, value):
    path.write_text(json.dumps(value, indent='\t') + '\n')


def artifact(path, role):
    path = path.resolve()
    return {'path': str(path), 'sha256': sha256(path), 'role': role}


def cargo_artifact(path, binary, kind):
    matches = []
    for line in path.read_text().splitlines():
        row = json.loads(line)
        if row.get('reason') == 'compiler-artifact' and row.get('executable'):
            if Path(row['executable']).resolve() == binary.resolve():
                matches.append(row)
    if len(matches) != 1:
        raise ValueError('need one exact original Cargo artifact for ' + str(binary))
    row = matches[0]
    if row['target']['name'] != 'layer_blend_region' or kind not in row['target']['kind']:
        raise ValueError('wrong artifact target')
    if row['profile']['opt_level'] != '3' or row['profile']['debug_assertions']:
        raise ValueError('only optimized actual release/bench artifacts')
    if not binary.resolve().is_relative_to((ROOT / 'target').resolve()):
        raise ValueError('canonical target artifacts required')
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ['binary', 'bench', 'example-build-jsonl', 'bench-build-jsonl',
                   'source-manifest', 'build-metadata', 'output']:
        parser.add_argument('--' + option, type=Path, required=True)
    parser.add_argument('--iterations', type=int, default=1)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    if not 1 <= args.iterations <= 32:
        raise ValueError('iterations outside1..32')
    example = cargo_artifact(args.example_build_jsonl, args.binary, 'example')
    bench = cargo_artifact(args.bench_build_jsonl, args.bench, 'bench')
    build = json.loads(args.build_metadata.read_text())
    sources = json.loads(args.source_manifest.read_text())['files']
    if not isinstance(sources, list) or not sources:
        raise ValueError('Root actual compiled local graph manifest required')
    artifacts = {}
    for row in sources:
        path = Path(row['path'])
        if not path.is_absolute():
            path = ROOT / path
        item = artifact(path, 'other')
        if item['sha256'] != row['sha256']:
            raise ValueError('compiled graph source drift: ' + str(path))
        artifacts[item['path']] = item
    files = [(args.binary, 'binary'), (args.bench, 'other'),
             (ROOT / 'Cargo.lock', 'lockfile'), (ROOT / 'rust-toolchain.toml', 'other'),
             (args.source_manifest, 'other'), (args.build_metadata, 'other'),
             (args.example_build_jsonl, 'other'), (args.bench_build_jsonl, 'other'),
             (Path(__file__), 'driver'),
             (ROOT / 'benchmarks/tools/run_hyperfine.py', 'driver'),
             (ROOT / 'benchmarks/tools/benchmark_support.py', 'driver'),
             (ROOT / 'benchmarks/cases.json', 'other'),
             (ROOT / 'crates/services/paintcraft-render/tests/layer-blend-benchmark-fixtures/oracle.json', 'fixture'),
             (ROOT / 'crates/services/paintcraft-render/tests/layer-blend-benchmark-fixtures/generate.py', 'generator')]
    for path, role in files:
        item = artifact(path, role)
        artifacts[item['path']] = item
    configs = {}
    for case in CASES:
        for action in ['run', 'zero']:
            name = case if action == 'run' else 'calibration-' + case
            config = {
                'schema': 'paintcraft.hyperfine-run/1', 'mode': 'application',
                'case_id': CASE_ID, 'task_id': 'T12',
                'relevance': 'Actual public CPU regional renderer opaque-certificate versus forced-general; residual off-ROI residency/lookup differs. Not isolated kernel, GPU, UI latency or RSS.',
                'cwd': str(ROOT),
                'workload': {'id': name, 'mode': 'process-lifecycle',
                    'work_units': args.iterations, 'seed': 42,
                    'cache_state': 'new process;fixture once;fresh explicit64MiB/1b caller per frame;no output cache',
                    'completion': 'fixture construction/drop plus complete regional render/output drop' if action == 'run' else 'zero-render fixture lifecycle plus caller-factory cycles;separate calibration,never subtracted'},
                'build': build, 'runs': 20, 'warmup': 3, 'timeout_seconds': 900,
                'env': {}, 'artifacts': list(artifacts.values()),
                'correctness_commands': [[str(args.binary.resolve()), 'verify']],
                'variants': [{'id': variant, 'argv': [str(args.binary.resolve()), action, case, variant, str(args.iterations)]}
                             for variant in ['opaque', 'general']],
            }
            validate_config(config)
            configs[name] = config
    baseline = artifact_snapshot(next(iter(configs.values())))
    plan = {'status': 'PLAN_ONLY', 'case_id': CASE_ID, 'cargo_artifacts': {'example': example, 'bench': bench},
            'sessions': ['forward', 'reverse', 'forward'], 'criterion_samples': 100,
            'configs': list(configs), 'output': str(args.output.resolve())}
    if not args.execute:
        print(json.dumps(plan, indent='\t'))
        return
    destination = args.output.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    write(destination / 'plan.json', plan)
    write(destination / 'inputs-before.json', baseline)
    completed = []
    status = {'status': 'started', 'application_qualification': False,
              'boundary': 'full public CPU region+caller lifecycle,not isolated kernel/RSS/UI', 'completed': completed}
    try:
        for name, config in configs.items():
            write(destination / (name + '.json'), config)
        for session, order in enumerate(['forward', 'reverse', 'forward'], 1):
            names = list(configs)
            if order == 'reverse':
                names.reverse()
            for name in names:
                disk_floor()
                if artifact_snapshot(configs[name]) != baseline:
                    raise ValueError('input drift before phase')
                execute(configs[name], destination / (name + '.json'),
                        destination / f'session-{session}-{name}', 'hyperfine', order == 'reverse')
                completed.append(f'hyperfine:{session}:{name}')
                write(destination / 'execution.json', status)
            phase = destination / f'criterion-{session}'
            disk_floor()
            phase.mkdir()
            env = os.environ.copy()
            env['CRITERION_HOME'] = str(phase / 'samples')
            env['PAINTCRAFT_BLEND_ORDER'] = order
            criterion_before = artifact_snapshot(next(iter(configs.values())))
            if criterion_before != baseline:
                raise ValueError('input drift immediately before Criterion')
            write(phase / 'inputs-before.json', criterion_before)
            code = run_logged([str(args.bench.resolve()), '--bench'], ROOT, env, 3600, phase / 'criterion.log')
            if code:
                raise RuntimeError('Criterion original exit ' + str(code))
            samples = list((phase / 'samples').glob('**/new/sample.json'))
            if len(samples) != 9:
                raise ValueError('expected8actual contenders plus1caller-factory calibration')
            for sample in samples:
                values = json.loads(sample.read_text())
                if len(values['times']) != 100 or len(values['iters']) != 100:
                    raise ValueError('missing original100samples')
                if any(not math.isfinite(t) or t <= 0 for t in values['times']) or any(n <= 0 for n in values['iters']):
                    raise ValueError('invalid raw Criterion samples')
                with sample.with_name('raw.csv').open() as handle:
                    if len(list(csv.DictReader(handle))) != 100:
                        raise ValueError('missing original Criterion CSV')
            if artifact_snapshot(next(iter(configs.values()))) != baseline:
                raise ValueError('input drift after Criterion')
            completed.append(f'criterion:{session}:{order}')
            write(destination / 'execution.json', status)
        status['status'] = 'phases-complete-awaiting-final-validation'
    except BaseException as error:
        status['status'] = 'failed'
        status['error'] = repr(error)
        raise
    finally:
        final_error = None
        try:
            after = artifact_snapshot(next(iter(configs.values())))
            status['inputs_after_observation'] = after
            if after != baseline:
                raise ValueError('final artifact/source observation differs from baseline')
            status['final_inputs_exact'] = True
            if status['status'] == 'phases-complete-awaiting-final-validation':
                status['status'] = 'completed-unreviewed'
        except BaseException as observation_error:
            status['final_inputs_exact'] = False
            status['inputs_after_observation_error'] = repr(observation_error)
            if status['status'] == 'phases-complete-awaiting-final-validation':
                status['status'] = 'failed-final-observation'
                final_error = observation_error
        write(destination / 'execution.json', status)
        lines = [f'{sha256(path)}  {path.relative_to(destination).as_posix()}'
                 for path in sorted(destination.rglob('*')) if path.is_file() and path.name != 'MANIFEST.sha256']
        (destination / 'MANIFEST.sha256').write_text('\n'.join(lines) + '\n')
        if final_error is not None:
            raise RuntimeError('final input observation failed; raw results retained') from final_error


if __name__ == '__main__':
    main()
