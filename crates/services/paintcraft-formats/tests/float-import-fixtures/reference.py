"""Full conversion expectations from external float reference files only.

No selected Rust decoder or precision implementation generates these pixels.
Reuses the separately qualified Decimal100/NumPy color oracle's definitions,
without executing its fixture-writing entrypoint.
"""
import hashlib
import json
from pathlib import Path
import struct

root = Path(__file__).parent
fixtures = root.parent
policy_source = fixtures / 'precision-fixtures/reference.py'
policy_text = policy_source.read_text()
marker = '\ncases = []\n'
assert policy_text.count(marker) == 1, 'independent oracle entrypoint marker must exist exactly once'
definitions = policy_text.split(marker)[0]
oracle = {}
exec(compile(definitions, str(policy_source), 'exec'), oracle)
reference = oracle['reference']
# Same independent RGBE floats, with a different declared physical pixel aspect.
original = (fixtures / 'radiance-fixtures/literal-2x2.hdr').read_bytes()
(root / 'aspect-2.hdr').write_bytes(original.replace(b'\n\n-Y', b'\nPIXASPECT=2\n\n-Y', 1))
cases = []


def add(name, family, file, floats=None, **overrides):
    options = {'space': 'radiance' if family == 'hdr' else 'rec709',
               'alpha': 'opaque', 'curve': 'clip', 'negative': 'clip',
               'exposure_milli_ev': 0, **overrides}
    group = 'radiance-fixtures' if family == 'hdr' else 'exr-fixtures'
    source = root / file if file == 'aspect-2.hdr' else fixtures / group / file
    if floats is None:
        floats = file.replace('.hdr', '.lower-edge.f32le').replace('.exr', '.f32le')
    external = fixtures / group / floats
    data = external.read_bytes()
    width = 4 if options['alpha'] != 'opaque' else 3
    values = [item[0] for item in struct.iter_unpack('<f', data)]
    output = bytearray()
    error = None
    for start in range(0, len(values), width):
        sample = values[start:start+width]
        case = {**options, 'sample_bits': [struct.pack('>f', v).hex() for v in sample]}
        converted = reference(case)
        if 'error' in converted:
            error = converted['error']
            break
        output.extend(converted['rgba8'])
    item = {'id': name, 'family': family, 'file': file,
            'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
            'external_float_file': f'{group}/{floats}',
            'external_float_sha256': hashlib.sha256(data).hexdigest(),
            'settings': options, 'expected_error': error}
    if error is None:
        destination = name + '.rgba'
        (root / destination).write_bytes(output)
        item.update({'expected_rgba_file': destination, 'rgba_sha256': hashlib.sha256(output).hexdigest()})
    cases.append(item)


add('hdr-clip', 'hdr', 'literal-2x2.hdr')
add('hdr-reinhard', 'hdr', 'literal-2x2.hdr', curve='reinhard')
add('hdr-calibrated-as-stored', 'hdr', 'calibrated-2x2.hdr')
add('hdr-extremes', 'hdr', 'exponents-10x1.hdr')
add('hdr-aspect-normalization', 'hdr', 'aspect-2.hdr', floats='literal-2x2.lower-edge.f32le')
add('hdr-unresolved-refusal', 'hdr', 'near-rec709-unresolved.hdr', space='unspecified')
add('hdr-explicit-source', 'hdr', 'near-rec709-unresolved.hdr', space='unspecified', assignment='rec709', exposure_milli_ev=1000)
add('exr-clip', 'exr', 'half-rgb-none.exr')
add('exr-reinhard', 'exr', 'float-rgb-zips.exr', curve='reinhard', exposure_milli_ev=-1000)
add('exr-negative-refusal', 'exr', 'half-rgb-none.exr', negative='reject')
add('exr-alpha-additive-refusal', 'exr', 'half-rgba-rle.exr', alpha='premultiplied')
add('exr-alpha-explicit-flatten', 'exr', 'half-rgba-rle.exr', alpha='premultiplied', background=[255, 255, 255])
add('exr-alpha-explicit-straight', 'exr', 'mixed-rgba-zip.exr', alpha='premultiplied', alpha_assignment='straight')
add('exr-unresolved-refusal', 'exr', 'unlabelled.exr', space='unspecified')
add('exr-explicit-source', 'exr', 'unlabelled.exr', space='unspecified', assignment='rec709')
add('exr-explicit-unknown-color', 'exr', 'unknown-chromaticities.exr', space='unspecified', assignment='radiance')
add('exr-special-refusal', 'exr', 'float-rgba-special.exr', alpha='premultiplied', background=[255,255,255])
(root / 'manifest.json').write_text(json.dumps({'oracle': 'external float files -> Python Decimal100 + independent NumPy color matrix', 'policy_oracle_sha256': hashlib.sha256(policy_source.read_bytes()).hexdigest(), 'cases': cases}, indent=4) + '\n')
print(f'Wrote {len(cases)} end-to-end float conversion expectations')
