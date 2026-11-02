"""Independent Fraction literals, frozen before benchmark Rust source."""
from fractions import Fraction as F
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
BACK = [31, 127, 241, 255]
FRONT = [204, 77, 13, 128]
OPACITY = 128


def nearest(value):
    q, r = divmod(value.numerator, value.denominator)
    return q + int(2 * r >= value.denominator)


def pixel(mode):
    # Existing layer-opacity quantization occurs once before exact composition.
    a = F(nearest(F(FRONT[3] * OPACITY, 255)), 255)
    output = []
    for s, d in zip(FRONT[:3], BACK[:3]):
        s, d = F(s, 255), F(d, 255)
        mixed = s * d if mode == 'multiply' else 1 - (1 - s) * (1 - d)
        output.append(nearest(255 * ((1 - a) * d + a * mixed)))
    return output + [255]


rows = []
for size, width, height in [('default', 1000, 486), ('1080p', 1920, 1080)]:
    for mode in ['multiply', 'screen']:
        rgba = pixel(mode)
        digest = hashlib.sha256()
        row = bytes(rgba) * width
        for _ in range(height):
            digest.update(row)
        rows.append({'id': f'{size}-{mode}', 'width': width, 'height': height,
                     'document_height': ((height + 255) // 256) * 256 + 256,
                     'rgba': rgba, 'output_sha256': digest.hexdigest()})
value = {'status': 'independent-before-benchmark-source-not-product-results',
         'back': BACK, 'front': FRONT, 'opacity': OPACITY, 'cases': rows}
(HERE / 'oracle.json').write_text(json.dumps(value, indent='\t') + '\n')
