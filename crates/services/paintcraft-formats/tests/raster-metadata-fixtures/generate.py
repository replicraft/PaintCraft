"""Independent standard-library PNG/BMP fixture producer, no image codec dependency."""
from pathlib import Path
import hashlib
import json
import struct
import zlib

HERE = Path(__file__).resolve().parent
ORACLE = []


def chunk(kind, payload):
    return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))


def png(name, width, height, depth, model, raw, expected, chunks=(), palette=None, alpha=None, qualified=True, interlace=0):
    data = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, depth, model, 0, 0, interlace))
    data += b''.join(chunk(k, v) for k, v in chunks if k != b'tEXt')
    if palette is not None:
        data += chunk(b'PLTE', bytes(sum(palette, [])))
    if alpha is not None:
        data += chunk(b'tRNS', bytes(alpha))
    data += chunk(b'IDAT', zlib.compress(raw))
    data += b''.join(chunk(k, v) for k, v in chunks if k == b'tEXt')
    data += chunk(b'IEND', b'')
    save(name, data, width, height, expected, qualified, {'raw_scanlines_hex': raw.hex(), 'palette': palette, 'alpha': alpha})


def save(name, data, width, height, expected, qualified=True, facts=None):
    (HERE / name).write_bytes(data)
    ORACLE.append({'file': name, 'sha256': hashlib.sha256(data).hexdigest(), 'width': width, 'height': height, 'qualified': qualified, 'expected_rgba8': expected, 'rgba8_sha256': hashlib.sha256(bytes(expected)).hexdigest(), 'source_facts': facts or {}})


def bmp(name, width, height, bpp, rows, expected, dib=40, palette=(), color=0, icc=b'', resolution=(0, 0), top_down=False, masks=(), gamma=(0, 0, 0)):
    stride = ((width * bpp + 31) // 32) * 4
    body = b''.join(row + b'\0' * (stride - len(row)) for row in (rows if top_down else rows[::-1]))
    header = bytearray(dib)
    struct.pack_into('<IiiHHIIiiII', header, 0, dib, width, -height if top_down else height, 1, bpp, 3 if masks else 0, len(body), *resolution, len(palette), 0)
    if dib >= 108:
        struct.pack_into('<4II', header, 40, *(masks or (0, 0, 0, 0)), color)
        struct.pack_into('<III', header, 96, *gamma)
    packed_palette = b''.join(bytes((b, g, r, reserved)) for r, g, b, reserved in palette)
    offset = 14 + dib + len(packed_palette)
    if dib == 124:
        struct.pack_into('<IIII', header, 108, 4, dib + len(packed_palette) + len(body) if icc else 0, len(icc), 0)
    data = b'BM' + struct.pack('<IHHI', offset + len(body) + len(icc), 0, 0, offset) + bytes(header) + packed_palette + body + icc
    save(name, data, width, height, expected, facts={'palette_rgb_reserved': palette, 'row_stride': stride, 'top_down': top_down, 'color_space': color, 'resolution': resolution, 'icc_sha256': hashlib.sha256(icc).hexdigest() if icc else None})


palette = [[13, 71, 201], [13, 71, 201], [250, 50, 20], [0, 255, 127]]
for depth, indices in [(1, [0, 1, 1]), (2, [0, 1, 2]), (4, [0, 2, 1]), (8, [2, 0, 1])]:
    count = min(1 << depth, len(palette))
    packed = bytearray((3 * depth + 7) // 8)
    for x, value in enumerate(indices):
        packed[x * depth // 8] |= value << (8 - depth - (x * depth % 8))
    alphas = [0, 128, 255, 1][:count]
    expected = sum((palette[i] + [alphas[i]] for i in indices), [])
    png(f'indexed{depth}-trns.png', 3, 1, depth, 3, b'\0' + packed, expected, palette=palette[:count], alpha=alphas)

# Independent Adam7 pass packing: each pass row starts with filter0 and has
# its own packed2-bit row padding. The asymmetric5x5 source exercises all7 passes.
indices = [[(x + 2 * y + (x * y) % 3) % 4 for x in range(5)] for y in range(5)]
passes = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]
raw = bytearray()
for x0, y0, dx, dy in passes:
    columns = list(range(x0, 5, dx))
    if not columns:
        continue
    for y in range(y0, 5, dy):
        packed = bytearray((len(columns) * 2 + 7) // 8)
        for sample, x in enumerate(columns):
            packed[sample // 4] |= indices[y][x] << (6 - (sample % 4) * 2)
        raw.extend(b'\0' + packed)
expected = sum((palette[index] + [[0, 128, 255, 1][index]] for row in indices for index in row), [])
png('indexed2-adam7.png', 5, 5, 2, 3, raw, expected, palette=palette, alpha=[0, 128, 255, 1], interlace=1)

rgba = [13, 71, 201, 0, 250, 1, 20, 128, 3, 4, 5, 255]
exif = b'II\x2a\0\x08\0\0\0' + struct.pack('<HHHI4sI', 1, 274, 3, 1, b'\x06\0\0\0', 0)
png('rgba8-exif-orientation.png', 3, 1, 8, 6, b'\0' + bytes(rgba), rgba, [(b'eXIf', exif)])
canonical_chrm = [31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000]
base_chunks = [(b'gAMA', struct.pack('>I', 45455)), (b'cHRM', struct.pack('>8I', *canonical_chrm)), (b'sRGB', b'\0'), (b'pHYs', struct.pack('>IIB', 3780, 7560, 1)), (b'tEXt', b'Author\0independent fixture')]
png('rgba8-srgb-phys-text.png', 3, 1, 8, 6, b'\0' + bytes(rgba), rgba, base_chunks)
png('rgba8-aspect-gamma.png', 3, 1, 8, 6, b'\0' + bytes(rgba), rgba, [(b'gAMA', struct.pack('>I', 100000)), (b'pHYs', struct.pack('>IIB', 2, 1, 0))])
samples = [129, 255, 256, 0, 43981, 39030, 30292, 32896, 65535, 1, 32767, 65535]
png('rgba16-high-byte.png', 3, 1, 16, 6, b'\0' + struct.pack('>12H', *samples), [x >> 8 for x in samples], [(b'sBIT', bytes([12, 12, 12, 16]))])
# This blob was independently produced by LittleCMS in the existing TIFF corpus.
tiff_oracle = json.loads((HERE.parent / 'tiff-fixtures/oracle.json').read_text())
icc = bytes.fromhex(next(f for f in tiff_oracle['fixtures'] if f['file'] == 'icc-srgb-rgba8.tif')['pages'][0]['icc_hex'])
png('rgba8-icc-precedence.png', 3, 1, 8, 6, b'\0' + bytes(rgba), rgba, [(b'gAMA', struct.pack('>I', 100000)), (b'iCCP', b'Independent sRGB\0\0' + zlib.compress(icc))])
png('rgba8-cicp-precedence.png', 3, 1, 8, 6, b'\0' + bytes(rgba), rgba, [(b'gAMA', struct.pack('>I', 100000)), (b'sRGB', b'\1'), (b'cICP', bytes([1, 13, 0, 1]))])
png('invalid-index.png', 1, 1, 8, 3, b'\0\x01', [0, 0, 0, 255], palette=[[17, 19, 23]], qualified=False)
for bpp, raw in [(1, b'\x40'), (4, b'\x01\x00'), (8, b'\0\x01\0')]:
    colors = [(13, 71, 201, 137), (250, 1, 20, 0)]
    bmp(f'indexed{bpp}-reserved.bmp', 3, 1, bpp, [raw], [13, 71, 201, 255, 250, 1, 20, 255, 13, 71, 201, 255], palette=colors, resolution=(3780, 7560))
rows = [bytes([201, 71, 13, 20, 1, 250, 5, 4, 3]), bytes([30, 20, 10, 60, 50, 40, 90, 80, 70])]
expected = [13, 71, 201, 255, 250, 1, 20, 255, 3, 4, 5, 255, 10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255]
bmp('rgb24-top-down.bmp', 3, 2, 24, rows, expected, top_down=True)
bmp('rgb24-v4-calibrated.bmp', 3, 2, 24, rows, expected, dib=108, gamma=(65536, 65536, 65536))
bmp('rgb24-v5-icc.bmp', 3, 2, 24, rows, expected, dib=124, color=0x4D424544, icc=icc, resolution=(3780, 3780))
bmp('rgba32-v4-hidden.bmp', 3, 1, 32, [bytes([201, 71, 13, 0, 20, 1, 250, 128, 5, 4, 3, 255])], rgba, dib=108, color=0x73524742, masks=(0xFF0000, 0xFF00, 0xFF, 0xFF000000))
manifest = {'producer': 'Python standard-library struct/zlib; no PaintCraft/image/png writer', 'icc_origin': '../tiff-fixtures/oracle.json icc-srgb-rgba8.tif page0; independently authored LittleCMS profile', 'icc_sha256': hashlib.sha256(icc).hexdigest(), 'fixtures': ORACLE}
(HERE / 'oracle.json').write_text(json.dumps(manifest, indent=4) + '\n')
print(f'{len(ORACLE)} independent fixtures')
