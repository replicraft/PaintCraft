"""Independent stdlib framing literals; no Raster/product imports or evaluator."""
import hashlib
import json
import struct
from pathlib import Path

HERE = Path(__file__).resolve().parent


def record(tx, ty, kind, value, width, height, payload=b''):
    return struct.pack('>qqBB2xHHI4x', tx, ty, kind, value, width, height, len(payload)) + payload


def vector(name, extent, clip, offset, default, inversion, records, expected):
    body = b''.join(records)
    header = struct.pack(
        '>8sQI4x4q4q2qBBB21x', b'PCSCAP01', 128 + len(body), len(records),
        *extent, *(clip or (0, 0, 0, 0)), *offset, default, inversion, int(clip is not None),
    )
    data = header + body
    return {
        'name': name, 'extent': extent, 'clip': clip, 'offset': offset,
        'raw_default': default, 'inverted': inversion,
        'expected_logical_rows': expected,
        'bytes_hex': data.hex(), 'sha256': hashlib.sha256(data).hexdigest(),
    }


vectors = [
    vector('uniform-zero', (0, 0, 3, 1), (0, 0, 3, 1), (0, 0), 0, 0, [], [[0, 0, 0]]),
    vector('empty-intersection', (0, 0, 3, 1), None, (0, 0), 255, 0, [], [[0, 0, 0]]),
    vector('solid', (0, 0, 3, 1), (0, 0, 3, 1), (0, 0), 0, 0,
           [record(0, 0, 0, 255, 3, 1)], [[255, 255, 255]]),
    vector('dense-lsb-partial', (0, 0, 3, 2), (0, 0, 3, 2), (0, 0), 0, 0,
           [record(0, 0, 1, 0, 3, 2, bytes.fromhex('15'))], [[255, 0, 255], [0, 255, 0]]),
    vector('negative-grid-offset-clip', (-2, -1, 2, 1), (-1, -1, 2, 1), (1, 0), 0, 0,
           [record(-1, -1, 1, 0, 2, 1, bytes.fromhex('01')),
            record(0, -1, 0, 255, 1, 1),
            record(-1, 0, 1, 0, 2, 1, bytes.fromhex('02'))],
           [[0, 255, 0, 255], [0, 0, 255, 0]]),
    vector('default255-inverted', (0, 0, 3, 1), (0, 0, 3, 1), (0, 0), 255, 1,
           [record(0, 0, 1, 0, 3, 1, bytes.fromhex('05'))], [[0, 255, 0]]),
]
output = {
    'status': 'independent-source-only-literals-not-product-test-results',
    'grammar': 'selection-capture-snapshot1', 'header_bytes': 128,
    'record_prefix_bytes': 32, 'vectors': vectors,
}
(HERE / 'vectors.json').write_text(json.dumps(output, indent='\t') + '\n')
