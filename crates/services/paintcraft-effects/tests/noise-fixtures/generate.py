"""Portable compact manifest of the frozen independent4380 noise math specimens."""
import hashlib
import json
from pathlib import Path
import runpy
import sys
sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
BASE_SHA = 'bcd6418fabd878b707c398bdb10d25db394ef4f451f105f1bc84c466d44ef11a'
BOUNDARY_SHA = '10559e4d0aa248f8d057bd9242fd0e59ed830bee7cef3774b3b2dd79fbdd7a4c'


def encoded(value):
    return (json.dumps(value, indent=2)+'\n').encode()


def build():
    reference = runpy.run_path(str(HERE/'reference.py'))
    base = reference['build']()
    boundary = runpy.run_path(str(HERE/'boundary_reference.py'))['build']()
    assert hashlib.sha256(encoded(base)).hexdigest() == BASE_SHA
    assert hashlib.sha256(encoded(boundary)).hexdigest() == BOUNDARY_SHA
    rows, masks = [], bytearray()
    for row in base['rows']+boundary['rows']:
        rgb = row['rgba'][:3]
        expected = row['reference_rgba']
        mixed = [[*reference['coverage'](rgb, expected[:3], a), expected[3]]
                 for a in reference['COVERAGES']]
        masks.extend(v for rgba in mixed for v in rgba)
        rows.append({key: row[key] for key in ['amount_hundredths', 'seed', 'monochrome',
                     'distribution', 'source_local', 'rgba', 'reference_rgba']} |
                    {'six_mask_rgba': mixed})
    rgba = bytes(v for row in rows for v in row['reference_rgba'])
    assert len(rows) == 4380 and not base['precision_ambiguities'] and not boundary['precision_ambiguities']
    return {'schema':'paintcraft.noise-mathematical-fixtures/v1',
            'scope':'Independent finite Fraction/Decimal mathematical agreement, NOT software-libm bit authority or global real-rounding proof',
            'base_original_sha256':BASE_SHA,'boundary_original_sha256':BOUNDARY_SHA,
            'base_rgba_stream_sha256':base['rgba_stream_sha256'],
            'base_mask_stream_sha256':base['mask_rgba_stream_sha256'],
            'boundary_rgba_stream_sha256':boundary['rgba_sha256'],
            'boundary_mask_stream_sha256':boundary['mask_rgba_sha256'],
            'rgba_stream_sha256':hashlib.sha256(rgba).hexdigest(),
            'mask_stream_sha256':hashlib.sha256(masks).hexdigest(),
            'precision_agreement_digits':[96,160],'guard_digits':20,
            'not_certified_intervals':True,'mask_coverages':reference['COVERAGES'],
            'counts':{'rows':4380,'rgba_masks':26280,'base':4320,'draw_boundaries':60},
            'rows':rows}


if __name__ == '__main__':
    result = build()
    blob = (json.dumps(result,separators=(',',':'))+'\n').encode()
    target=HERE/'oracle.json'
    if '--check' in sys.argv:
        assert target.read_bytes()==blob
    else:
        assert not target.exists(), 'preserve first expected fixture'
        target.write_bytes(blob)
    print('4380 independent rows/26280 masks',len(blob),hashlib.sha256(blob).hexdigest())
