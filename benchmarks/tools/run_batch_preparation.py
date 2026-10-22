#!/usr/bin/env python3
"""Qualify complete batch preparation with frozen CPU CLIs and a real editor driver."""
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
import struct
import subprocess
import zipfile
import zlib


CASES = ['horizontal_pencil', 'diagonal_pencil', 'smooth_brush', 'smooth_eraser',
         'mixed_lineage', 'default_control', 'dense']


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def semantics(path):
    """Independent v1 raw-block reconstruction for required fixture/output pixels."""
    with zipfile.ZipFile(path) as archive:
        doc = json.loads(archive.read('document.json'))
        indexed = {layer['id']: layer for layer in doc['layers']}; layers = []
        for key in doc['roots']:
            layer = indexed[key]; width = layer['width']; height = layer['height']
            pixels = bytearray(bytes.fromhex(layer['default_pixel'][1:]) * width * height)
            for block in layer['blocks']:
                assert block['encoding'] == 'raw'
                data = archive.read(block['entry'])[block['offset']:block['offset']+block['stored_bytes']]
                assert hashlib.sha256(data).hexdigest() == block['sha256']
                x = block['block_x'] * 256; y = block['block_y'] * 256
                w = min(256, width-x); h = min(256, height-y)
                assert len(data) == w*h*4
                for row in range(h):
                    offset = ((y+row)*width+x)*4
                    pixels[offset:offset+w*4] = data[row*w*4:(row+1)*w*4]
            layers.append({'id': key, 'name': layer['name'], 'origin': [layer['origin']['x'], layer['origin']['y']],
                           'visible': layer['visible'], 'opacity': layer['opacity_u8'], 'width': width, 'height': height,
                           'default_pixel': layer['default_pixel'], 'pixels_sha256': hashlib.sha256(pixels).hexdigest()})
        return {'document_id': doc['document_id'], 'canvas': [doc['canvas']['width'], doc['canvas']['height']],
                'title': doc['metadata'].get('title'), 'layers': layers}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ['baseline-cli', 'candidate-cli', 'library', 'output']:
        parser.add_argument('--' + key, type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--preflight-only', action='store_true')
    args = parser.parse_args()
    if not args.execute:
        print('Plan only: fixture/exact-output preflight; no timing or files written.' if args.preflight_only else 'Plan only: 280 statistical samples, 1860 process observations; no files written.')
        return
    baseline = json.loads(args.baseline_cli.read_text())
    binaries = {'sequential': Path(baseline['binary']), 'planned': args.candidate_cli.resolve()}
    library = args.library.resolve()
    assert sha(binaries['sequential']) == baseline['binary_sha256']
    features = subprocess.check_output(['cargo', 'tree', '--locked', '-p', 'paintcraft-cli', '-e', 'features'], text=True)
    assert 'wgpu' not in features
    assert hashlib.sha256(features.encode()).hexdigest() == baseline['feature_tree_sha256']
    out = args.output.resolve(); out.mkdir(parents=True, exist_ok=False)
    (out / 'baseline-cli-manifest.json').write_bytes(args.baseline_cli.read_bytes())
    (out / 'cli-features.log').write_text(features)
    fixtures = out / 'fixtures'; fixtures.mkdir()
    def command(label, argv):
        with (out / (label + '.log')).open('wb') as log:
            subprocess.run(argv, stdout=log, stderr=subprocess.STDOUT, check=True)
    command('fixture-new', [str(binaries['sequential']), '--json', 'new', '--width', '1000', '--height', '1000', '--background', '#FFFFFFFF', '--output', str(fixtures / 'input.pcnd')])
    # A reproducible high-entropy RGBA8 PNG, imported through the real old CLI.
    raw = bytearray(); state = 1729
    for _ in range(1_000_000):
        state = (1664525 * state + 1013904223) & 0xffffffff
        raw.extend((state & 255, (state >> 8) & 255, (state >> 16) & 255, 255))
    (fixtures / 'dense.rgba').write_bytes(raw)
    def chunk(kind, data):
        return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
    rows = b''.join(b'\0' + bytes(raw[y*4000:(y+1)*4000]) for y in range(1000))
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1000, 1000, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')
    (fixtures / 'dense.png').write_bytes(png)
    (fixtures / 'dense.sha256').write_text(sha(fixtures / 'dense.png') + '\n')
    command('fixture-import', [str(binaries['sequential']), '--json', 'import', str(fixtures / 'dense.png'), '--output', str(fixtures / 'dense.pcnd')])
    assert semantics(fixtures / 'dense.pcnd')['layers'][0]['pixels_sha256'] == sha(fixtures / 'dense.rgba')
    def document(name):
        with zipfile.ZipFile(fixtures / name) as archive:
            return json.loads(archive.read('document.json'))
    docs = {'plain': document('input.pcnd'), 'dense': document('dense.pcnd')}
    def layer_ref(doc): return {'id': doc['layers'][0]['id']}
    previous = Path('benchmarks/results/paint-plan-cli-cpu-2026-10-21/fixtures')
    for case in CASES:
        doc = docs['dense' if case == 'dense' else 'plain']
        source = previous / (('horizontal_pencil' if case == 'dense' else case) + '.json')
        if source.exists():
            operations = copy.deepcopy(json.loads(source.read_text())['operations'])
            for op in operations: op['layer_ref'] = layer_ref(doc)
        elif case == 'default_control':
            operations = [{'op': 'raster.fill_rect', 'layer_ref': layer_ref(doc), 'rect': {'x': 0, 'y': 0, 'width': 1000, 'height': 1000}, 'color': '#FFFFFFFF', 'mode': 'replace'} for _ in range(100)]
        else:
            operations = [{'op': 'selection.set_rect', 'rect': {'x': 0, 'y': 0, 'width': 512, 'height': 64}}]
            for index in range(8):
                child = {'ref': 'copy' + str(index)}
                operations.extend([
                    {'op': 'raster.fill_rect', 'layer_ref': layer_ref(doc), 'rect': {'x': 16*(index+1), 'y': 0, 'width': 1, 'height': 64}, 'color': '#000000FF', 'mode': 'replace'},
                    {'op': 'layer.duplicate', 'layer_ref': layer_ref(doc), 'ref': child['ref'], 'name': 'Copy'},
                    {'op': 'raster.flood_fill', 'layer_ref': child, 'at': {'x': 0, 'y': 0}, 'color': '#11589C80', 'mode': 'source-over', 'tolerance': 0},
                    {'op': 'raster.fill_rect', 'layer_ref': child, 'rect': {'x': 300, 'y': 0, 'width': 20, 'height': 32}, 'color': '#11589C80', 'mode': 'source-over'},
                    {'op': 'layer.delete', 'layer_ref': child},
                ])
            operations.append({'op': 'selection.clear'})
        batch = {'schema': 'paintcraft.batch/v2', 'atomic': True, 'policy': 'paintcraft.core/default-v1', 'base': {'state_id': doc['state_id'], 'publication_epoch': '0'}, 'operations': operations}
        (fixtures / (case + '.json')).write_text(json.dumps(batch, separators=(',', ':')) + '\n')
    environment = dict(os.environ, PAINTCRAFT_BATCH_FIXTURES=str(fixtures))
    sources = sorted({Path('Cargo.toml'), Path('Cargo.lock'), Path('rust-toolchain.toml'), Path('crates/editing/paintcraft-editor/benches/support/sequential-manifest.json'), Path(__file__).resolve().relative_to(Path.cwd()),
                      *Path('apps').rglob('*.rs'), *Path('apps').rglob('Cargo.toml'), *Path('crates').rglob('*.rs'), *Path('crates').rglob('*.wgsl'), *Path('crates').rglob('Cargo.toml')})
    manifest = {'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(), 'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'platform': platform.platform(),
                'sources_sha256': {str(p): sha(p) for p in sources}, 'cli': {mode: {'path': str(p), 'sha256': sha(p)} for mode, p in binaries.items()},
                'library': {'path': str(library), 'sha256': sha(library)}, 'fixtures_sha256': {str(p.relative_to(out)): sha(p) for p in fixtures.iterdir()},
                'feature_tree_sha256': hashlib.sha256(features.encode()).hexdigest(), 'rustc': subprocess.check_output(['rustc', '-Vv'], text=True), 'hyperfine': subprocess.check_output(['hyperfine', '--version'], text=True).strip(),
                'cases': CASES, 'preflight_only': args.preflight_only, 'runs': 20, 'warmups': 3, 'sessions': 3, 'planned_statistical_samples': 280, 'planned_process_observations': 1860,
                'scope': 'Whole built-in editor batch compilation/replay/destruction versus frozen executor support using common current raster primitives. Parent/current CPU-only CLIs confirm real apply/history/canonical file IO. Input acquisition excluded from statistical loops but included in process runs. Not physical RSS ceilings, scheduler fairness or native input/display latency.',
                'environment': 'Uncontrolled local macOS host; no affinity/power/thermal controls; no intentional concurrent task builds/tests/timing. Preserve controls, raw samples and calibration without subtraction.'}
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    results = out / 'correctness'; results.mkdir()
    commands = {case: {mode: [str(binary), '--json', 'apply', str(fixtures / ('dense.pcnd' if case == 'dense' else 'input.pcnd')), '--operations', str(fixtures / (case + '.json')), '--output', str(results / (mode + '-' + case + '.pcnd')), '--overwrite', '--expect-input-sha256', sha(fixtures / ('dense.pcnd' if case == 'dense' else 'input.pcnd'))] for mode, binary in binaries.items()} for case in CASES}
    def identities():
        assert all(sha(p) == h for p, h in manifest['sources_sha256'].items())
        assert all(sha(e['path']) == e['sha256'] for e in [*manifest['cli'].values(), manifest['library']])
        assert all(sha(out / p) == h for p, h in manifest['fixtures_sha256'].items())
    checks = {}
    def verify(label):
        identities(); raw = subprocess.check_output([str(library), 'verify'], env=environment, stderr=subprocess.STDOUT)
        (out / ('library-' + label + '.log')).write_bytes(raw); record = {'library': hashlib.sha256(raw).hexdigest(), 'cli': {}, 'semantics': {}}
        expected = {}
        for line in raw.decode().splitlines():
            words = line.split()
            if words and words[0] in CASES:
                checksum = words[2].removeprefix('sha256=')
                assert expected.setdefault(words[0], checksum) == checksum
        assert set(expected) == set(CASES)
        for case in CASES:
            for mode in binaries: command('cli-' + label + '-' + case + '-' + mode, commands[case][mode])
            a = results / ('sequential-' + case + '.pcnd'); b = results / ('planned-' + case + '.pcnd'); assert a.read_bytes() == b.read_bytes()
            with zipfile.ZipFile(a) as x, zipfile.ZipFile(b) as y:
                assert x.namelist() == y.namelist() and all(x.read(name) == y.read(name) for name in x.namelist())
            record['cli'][case] = sha(a)
            summary = json.dumps(semantics(a), sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
            record['semantics'][case] = hashlib.sha256(summary).hexdigest()
            assert record['semantics'][case] == expected[case]
        checks[label] = record; assert all(value == record for value in checks.values())
        (out / 'correctness.json').write_text(json.dumps(checks, indent=2) + '\n')
    verify('before')
    if args.preflight_only:
        print('All fixture and exact-output preflights pass; no timings performed.', flush=True)
        return
    with (out / 'criterion.log').open('wb') as log:
        subprocess.run([str(library), '--bench', '--noplot'], env=dict(environment, CRITERION_HOME=str(out / 'criterion')), stdout=log, stderr=subprocess.STDOUT, check=True)
    samples = list((out / 'criterion').glob('batch_preparation/*/*/new/sample.json'))
    assert len(samples) == 14 and all(len(json.loads(p.read_text())['times']) == 20 for p in samples)
    print('280 statistical samples complete', flush=True)
    for session in range(1, 4):
        order = ['sequential', 'planned'] if session != 2 else ['planned', 'sequential']
        for kind in ['cli', 'library']:
            for case in [*CASES, 'calibration']:
                if kind == 'cli':
                    argv = {mode: ([str(binaries[mode]), '--version'] if case == 'calibration' else commands[case][mode]) for mode in order}
                else:
                    argv = {mode: [str(library), 'calibration'] if case == 'calibration' else [str(library), 'process', case, mode] for mode in order}
                path = out / f'hyperfine-{session}-{kind}-{case}.json'
                # One shared library executable needs one startup calibration;
                # the two distinct CPU CLI artifacts each retain their own.
                measured_order=order[:1] if kind=='library' and case=='calibration' else order
                with path.with_suffix('.log').open('wb') as log:
                    subprocess.run(['hyperfine', '--shell=none', '--warmup', '3', '--runs', '20', '--export-json', str(path), *[shlex.join(argv[mode]) for mode in measured_order]], env=environment, stdout=log, stderr=subprocess.STDOUT, check=True)
                raw = path.read_bytes(); data = json.loads(raw)
                assert all(len(r['measurements']) == 20 and all(x['exit_code'] == 0 for x in r['measurements']) for r in data['results'])
                compact = (json.dumps(data, separators=(',', ':')) + '\n').encode(); path.with_suffix('.json.gz').write_bytes(gzip.compress(raw, mtime=0)); path.write_bytes(compact)
                path.with_suffix('.identity.json').write_text(json.dumps({'original_sha256': hashlib.sha256(raw).hexdigest(), 'compact_sha256': sha(path), 'equal_json_values': json.loads(raw) == json.loads(compact)}, indent=2) + '\n')
                print('session', session, kind, case, 'complete', flush=True)
        verify('after-' + str(session))
    identities(); print('All artifact identities and exact output postflights verified', flush=True)


if __name__ == '__main__':
    main()
