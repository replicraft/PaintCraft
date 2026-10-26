"""Owned fixtures written and reopened by official OpenEXR 3.5.2, NumPy 2.4.3.
Run in a private environment; no Rust decoder participates in these references.
"""
import hashlib
import json
from pathlib import Path
import struct
import OpenEXR
import numpy as np
assert OpenEXR.__version__ == '3.5.2'
assert np.__version__ == '2.4.3'
root = Path(__file__).parent
manifest = []
for name, dtype, compression, alpha, origin, rows in [
    ('half-rgb-none', np.float16, OpenEXR.NO_COMPRESSION, False, (0, 0), 2),
    ('half-rgba-rle', np.float16, OpenEXR.RLE_COMPRESSION, True, (-7, 11), 2),
    ('float-rgb-zips', np.float32, OpenEXR.ZIPS_COMPRESSION, False, (12, -9), 2),
    ('float-rgba-zip', np.float32, OpenEXR.ZIP_COMPRESSION, True, (-7, -11), 17),
    ('interop-rec709', np.float32, OpenEXR.NO_COMPRESSION, False, (0, 0), 2),
    ('unlabelled', np.float32, OpenEXR.NO_COMPRESSION, False, (0, 0), 2),
    ('unknown-chromaticities', np.float32, OpenEXR.NO_COMPRESSION, False, (0, 0), 2),
    ('conflicting-color', np.float32, OpenEXR.NO_COMPRESSION, False, (0, 0), 2),
]:
    width = 3
    channels = {}
    for index, channel in enumerate('RGBA' if alpha else 'RGB'):
        values = [((x + 3*y + index*2) % 13 - 3) / 4 for y in range(rows) for x in range(width)]
        if channel == 'A':
            values = [0.0, 0.25, 1.0] * rows
        channels[channel] = np.array(values, dtype=dtype).reshape(rows, width)
    header = {'compression': compression, 'type': OpenEXR.scanlineimage,
              'dataWindow': (np.array(origin, dtype=np.int32), np.array((origin[0]+width-1, origin[1]+rows-1), dtype=np.int32)),
              'displayWindow': (np.array(origin, dtype=np.int32), np.array((origin[0]+width-1, origin[1]+rows-1), dtype=np.int32)),
              'comments': 'PaintCraft owned EXR qualification pixels'}
    if name != 'unlabelled':
        header['chromaticities'] = (0.64, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290)
    if name == 'unknown-chromaticities':
        header['chromaticities'] = (0.64004, 0.33, 0.30, 0.60, 0.15, 0.06, 0.3127, 0.3290)
    if name == 'interop-rec709':
        header['colorInteropID'] = 'lin_rec709_scene'
    if name == 'conflicting-color':
        header['colorInteropID'] = 'lin_ap1_scene'
    path = root / (name + '.exr')
    expected = {channel: values.copy() for channel, values in channels.items()}
    OpenEXR.File(header, channels).write(str(path))
    reopened = OpenEXR.File(str(path), separate_channels=True)
    raw = bytearray()
    for y in range(rows):
        for x in range(width):
            for channel in ('RGBA' if alpha else 'RGB'):
                actual = float(reopened.channels()[channel].pixels[y, x])
                assert actual == float(expected[channel][y, x])
                raw.extend(struct.pack('<f', actual))
    golden = root / (name + '.f32le')
    golden.write_bytes(raw)
    manifest.append({'file': path.name, 'width': width, 'height': rows, 'origin': origin,
                     'alpha': alpha, 'compression': int(compression),
                     'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                     'reference_sha256': hashlib.sha256(raw).hexdigest()})
(root / 'manifest.json').write_text(json.dumps({'oracle': 'OpenEXR 3.5.2', 'numpy': np.__version__, 'fixtures': manifest}, indent=4) + '\n')
print(f'Wrote and reopened {len(manifest)} owned fixtures')
