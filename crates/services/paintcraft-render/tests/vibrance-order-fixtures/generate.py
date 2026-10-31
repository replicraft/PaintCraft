"""Independent ordered-stage literals; imports only frozen Python references."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = next(p for p in HERE.parents if (p / "Cargo.toml").is_file()
            and (p / "crates/services/paintcraft-effects/tests").is_dir())
REFERENCES = {
    'vibrance': 'docs/validation/vibrance-oracle/generate.py',
    'curves': 'crates/services/paintcraft-effects/tests/curves-fixtures/generate.py',
    'box': 'crates/services/paintcraft-effects/tests/box-fixtures/generate.py',
}


def portable_path(path):
    if path == 'docs/validation/vibrance-oracle/generate.py':
        return 'crates/services/paintcraft-effects/tests/vibrance-fixtures/generate.py'
    return path


def load(name):
    # Preserve original oracle provenance labels byte-for-byte, while resolving
    # the identical already-tracked Vibrance reference in a clean checkout.
    path = portable_path(REFERENCES[name])
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


V, C, B = (load(name) for name in REFERENCES)
FREE = [23, 9, 240, 73, 201]
LUT = C.table(FREE)[0]


def stage(pixels, size, kind, coverage):
    assert type(coverage) is int and coverage in [0, 127, 255]
    result = []
    for i in range(size[0] * size[1]):
        incoming = pixels[4*i:4*i+4]
        if kind == 'vibrance':
            candidate = V.candidate(incoming[:3], 100) + [incoming[3]]
            mixed = V.mix(incoming[:3], candidate[:3], coverage) + [incoming[3]]
        elif kind == 'curves':
            candidate = [LUT[x] for x in incoming[:3]] + [incoming[3]]
            mixed = V.mix(incoming[:3], candidate[:3], coverage) + [incoming[3]]
        else:
            assert kind == 'box'
            candidate = B.box_pixel(pixels, *size, 1, i % size[0], i // size[0])
            mixed = B.masked(incoming, candidate, coverage)
        result.extend(mixed)
    return result


def crop(pixels, size, origin, rect):
    x, y, w, h = rect
    ox, oy = origin
    assert ox <= x < x+w <= ox+size[0] and oy <= y < y+h <= oy+size[1]
    return [v for row in range(y-oy, y-oy+h)
            for col in range(x-ox, x-ox+w)
            for v in pixels[4*(row*size[0]+col):4*(row*size[0]+col)+4]]


def case(name, source, size, origin, order, coverage, partitions):
    output = list(source)
    prefixes = []
    for kind in order:
        output = stage(output, size, kind, coverage)
        prefixes.append(output)
    if coverage == 0:
        assert output == source
    return dict(name=name, size=size, origin=origin, source_rgba=source,
                stages=[dict(kind=k, coverage=coverage) for k in order],
                prefixes_rgba=prefixes, expected_rgba=output,
                expected_sha256=hashlib.sha256(bytes(output)).hexdigest(),
                partitions=[dict(region=r, expected_rgba=crop(output, size, origin, r))
                            for r in partitions])


def generate():
    rows = []
    point_source = [64, 16, 128, 255, 20, 115, 0, 0,
                    250, 1, 20, 127, 128, 128, 128, 1]
    for coverage in [0, 127, 255]:
        pair = []
        for order in [('curves', 'vibrance'), ('vibrance', 'curves')]:
            row = case('-'.join(order)+f'-mask{coverage}', point_source, [4, 1],
                       [-7, -3], order, coverage,
                       [[-7, -3, 1, 1], [-6, -3, 3, 1]])
            rows.append(row)
            pair.append(row['expected_rgba'])
        if coverage:
            assert pair[0] != pair[1], 'Curves order must be causal'
    # Cross 32/64/128 tile boundaries; negative global origin never changes
    # sampling. The full immutable finite source, not each output ROI, clamps.
    size, origin = [131, 2], [-67, -5]
    colors = [[255, 0, 0, 255], [0, 255, 0, 255], [128, 128, 128, 255],
              [64, 16, 128, 255], [20, 115, 200, 0], [250, 1, 20, 127]]
    source = [v for i in range(size[0]*size[1]) for v in colors[i % len(colors)]]
    parts = [[-67, -5, 32, 2], [-35, -5, 32, 2], [-3, -5, 64, 2], [61, -5, 3, 2]]
    for coverage in [0, 127, 255]:
        pair = []
        for order in [('vibrance', 'box'), ('box', 'vibrance')]:
            row = case('-'.join(order)+f'-mask{coverage}', source, size, origin,
                       order, coverage, parts)
            # Interior ROI includes both sides of a 128 partition boundary.
            roi = [59, -5, 5, 2]
            row['roi'] = dict(region=roi, expected_rgba=crop(row['expected_rgba'], size, origin, roi))
            rows.append(row)
            pair.append(row['expected_rgba'])
        if coverage:
            assert pair[0] != pair[1], 'Box order must be causal'
    return dict(schema='paintcraft.independent-ordered-vibrance/v1',
                parameters=dict(vibrance_amount_hundredths=100, curves_outputs=FREE, box_radius=1),
                references=[dict(path=p, sha256=hashlib.sha256((ROOT/portable_path(p)).read_bytes()).hexdigest())
                            for p in REFERENCES.values()], cases=rows)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(generate(), indent=2)+'\n'
    path = HERE/'oracle.json'
    if args.check:
        assert path.read_text() == text, 'oracle drift'
        print('12 ordered cases verified; four nonzero causal order pairs')
    else:
        path.write_text(text)
