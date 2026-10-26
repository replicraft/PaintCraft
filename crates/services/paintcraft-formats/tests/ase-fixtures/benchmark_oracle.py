"""Optional full-value oracle for palette_assets benchmark fixtures/CLI copies."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import swatch
from oracle import EXPECTED, canonical, color


def expected(case):
    if case == 'empty-0':
        return []
    if case == 'mixed-6':
        return EXPECTED
    count = {'grouped-256': 256, 'grouped-4096': 4096}[case]
    entries = []
    for group in range(count // 32):
        swatches = []
        for i in range(group * 32, group * 32 + 32):
            q = (i % 17) / 16
            model, values = [
                ('RGB', [q, (i % 9) / 8, 1]),
                ('CMYK', [q, 0.5, 0.25, 1]),
                ('Gray', [q]),
                ('LAB', [q, (i % 128) - 64, (i % 64) - 32]),
            ][i % 4]
            name = f'色 {i} 🚀' if i % 8 == 0 else f'Color {i}'
            swatches.append(color(name, model, values, ['Global', 'Spot', 'Process'][i % 3]))
        entries.append({'name': f'Group {group}', 'type': 'Color Group', 'swatches': swatches})
    return entries


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--fixtures', type=Path, required=True)
    parser.add_argument('--exports', type=Path)
    args = parser.parse_args()
    checked = []
    with (args.fixtures / 'fixtures.csv').open() as source:
        for row in csv.DictReader(source):
            case = row['case']
            path = args.fixtures / f'{case}.ase'
            data = path.read_bytes()
            assert len(data) == int(row['encoded_bytes'])
            assert hashlib.sha256(data).hexdigest() == row['sha256']
            values = canonical(expected(case))
            assert canonical(swatch.parse(path)) == values, case
            if args.exports:
                output = args.exports / f'{case}.ase'
                assert canonical(swatch.parse(output)) == values, case
                assert output.read_bytes() == data, case
            checked.append({'case': case, 'swatches': int(row['swatches']), 'exact_channel_bits_names_types_groups': True})
    assert len(checked) == 4
    print(json.dumps({'oracle': 'swatch 0.4.0', 'exports_checked': args.exports is not None, 'cases': checked}, indent=4))
