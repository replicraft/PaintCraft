"""Optional independent parser/writer check. Requires swatch==0.4.0."""
import argparse
import hashlib
import importlib.metadata
import json
import struct
from pathlib import Path
import swatch

ROOT = Path(__file__).parent

def color(name, mode, values, kind):
    return {'name': name, 'type': kind, 'data': {'mode': mode, 'values': values}}

EXPECTED = [
    color('Red', 'RGB', [1, 0, 0], 'Global'),
    {'name': '印刷 / Print', 'type': 'Color Group', 'swatches': [
        color('赤 🚀', 'RGB', [0.25, 0.5, 1], 'Process'),
        color('Cyan', 'CMYK', [1, 0, 0, 0.125], 'Spot'),
        color('Lab spot', 'LAB', [0.6, -35, -5], 'Spot'),
        color('Gray', 'Gray', [0.75], 'Process'),
    ]},
    {'name': 'Empty', 'type': 'Color Group', 'swatches': []},
    color('Red', 'Gray', [-0.0], 'Global'),
]

def canonical(entries):
    result = []
    for entry in entries:
        item = {'name': entry['name'], 'type': entry['type']}
        if 'swatches' in entry:
            item['swatches'] = canonical(entry['swatches'])
        else:
            item['mode'] = entry['data']['mode']
            item['channel_f32_be'] = [struct.pack('>f', v).hex() for v in entry['data']['values']]
        result.append(item)
    return result

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--input', type=Path, default=ROOT / 'mixed.ase')
    parser.add_argument('--write-fixture', type=Path)
    args = parser.parse_args()
    assert importlib.metadata.version('swatch') == '0.4.0'
    actual = canonical(swatch.parse(args.input))
    assert actual == canonical(EXPECTED), actual
    result = {'oracle': 'swatch 0.4.0', 'source': str(args.input), 'sha256': hashlib.sha256(args.input.read_bytes()).hexdigest(), 'exact_names_models_types_groups_channel_bits': True, 'swatches': 6}
    if args.write_fixture:
        specimen = [color('Independent gray', 'Gray', [0.25], 'Process'),
                    {'name': 'Print', 'type': 'Color Group', 'swatches': [color('Ink', 'CMYK', [0.125, 0.5, 0.75, 1], 'Spot')]}]
        data = swatch.dumps(specimen)
        args.write_fixture.write_bytes(data)
        assert canonical(swatch.parse(args.write_fixture)) == canonical(specimen)
        result['written_fixture_sha256'] = hashlib.sha256(data).hexdigest()
    print(json.dumps(result, indent=4, ensure_ascii=False))
