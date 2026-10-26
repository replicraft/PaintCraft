"""Owned OpenRaster group fixtures: only Python standard-library ZIP/XML/PNG/Fraction."""
from pathlib import Path
from fractions import Fraction
import binascii
import hashlib
import json
import struct
import zipfile
import zlib

ROOT = Path(__file__).parent


def png(width, height, pixels):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", binascii.crc32(kind + body))
    rows = b"".join(b"\0" + pixels[y*width*4:(y+1)*width*4] for y in range(height))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def rounded(value):
    return (2 * value.numerator + value.denominator) // (2 * value.denominator)


def over(source, destination):
    alpha = Fraction(source[3], 255)
    backdrop = Fraction(destination[3], 255)
    output_alpha = alpha + backdrop * (1-alpha)
    if alpha == 0:
        return destination
    colors = [rounded((source[c]*alpha + destination[c]*backdrop*(1-alpha))/output_alpha)
              for c in range(3)]
    return colors + [rounded(output_alpha*255)]


# Source XML is top first. Expected flat order is bottom first.
LAYERS = [
    ("Back", 3, 2, (0, 0), True, bytes([17, 51, 93, 255] * 6)),
    ("Mid", 3, 2, (0, 0), True, bytes([201, 11, 47, 128, 19, 71, 233, 64,
        9, 200, 55, 255, 81, 8, 13, 0, 99, 13, 200, 127, 0, 12, 40, 1])),
    ("Child visible but ancestor hidden", 1, 1, (0, 0), False, bytes([0, 255, 0, 255])),
    ("Child hidden", 1, 1, (2, 1), False, bytes([255, 0, 255, 255])),
    ("Bright", 2, 2, (-1, 0), True, bytes([111, 222, 13, 128, 91, 7, 200, 64,
        41, 52, 63, 0, 1, 2, 3, 254])),
    ("Fore", 2, 2, (1, -1), True, bytes([191, 29, 7, 255, 53, 0, 255, 128,
        14, 81, 241, 127, 13, 72, 12, 255])),
]


def merged():
    output = [[0, 0, 0, 0] for _ in range(6)]
    for _, width, height, (ox, oy), visible, raw in LAYERS:
        if not visible:
            continue
        for y in range(height):
            for x in range(width):
                if 0 <= x+ox < 3 and 0 <= y+oy < 2:
                    i = (y+oy)*3+x+ox
                    output[i] = over(list(raw[(y*width+x)*4:(y*width+x+1)*4]), output[i])
    return bytes(v for pixel in output for v in pixel)


XML = '''<image version="0.0.6" w="3" h="2" name="Source title"><stack>
 <layer name="Fore" src="data/5.png" x="1" y="-1"/>
 <stack name="日本語 outer" isolation="auto" opacity="1.000" composite-op="svg:src-over">
  <layer name="Bright" src="data/4.png" x="-1"/>
  <stack name="Duplicate" isolation="auto" visibility="hidden">
   <layer name="Child hidden" src="data/3.png" x="2" y="1" visibility="hidden"/>
   <layer name="Child visible but ancestor hidden" src="data/2.png"/>
   <stack name="Duplicate" isolation="auto"/>
  </stack>
  <layer name="Mid" src="data/1.png"/>
 </stack>
 <layer name="Back" src="data/0.png"/>
</stack></image>'''
FLAT = '''<image version="0.0.6" w="3" h="2" name="Source title"><stack>
 <layer name="Fore" src="data/5.png" x="1" y="-1"/>
 <layer name="Bright" src="data/4.png" x="-1"/>
 <layer name="Child hidden" src="data/3.png" x="2" y="1" visibility="hidden"/>
 <layer name="Child visible but ancestor hidden" src="data/2.png" visibility="hidden"/>
 <layer name="Mid" src="data/1.png"/>
 <layer name="Back" src="data/0.png"/>
</stack></image>'''


def write(name, xml, preview=None, corrupt_png=False, thumbnail=None):
    entries = [("mimetype", b"image/openraster"), ("stack.xml", xml.encode())]
    for i, (_, width, height, _, _, raw) in enumerate(LAYERS):
        encoded = png(width, height, raw)
        if corrupt_png:
            # The metadata should reject before examining this invalid pixel/header payload.
            encoded = b"not png"
        entries.append((f"data/{i}.png", encoded))
    preview = merged() if preview is None else preview
    entries += [("mergedimage.png", png(3, 2, preview)),
                ("Thumbnails/thumbnail.png", png(*(thumbnail or (3, 2, preview))))]
    with zipfile.ZipFile(ROOT/name, "w") as archive:
        for path, data in entries:
            info = zipfile.ZipInfo(path, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_STORED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, data)


def main():
    cases = {"neutral.ora": XML, "flat.ora": FLAT,
        "default-isolate.ora": XML.replace(' isolation="auto"', '', 1),
        "explicit-isolate.ora": XML.replace('isolation="auto"', 'isolation="isolate"', 1),
        "near-one.ora": XML.replace('opacity="1.000"', 'opacity="0.9999999999999999999999999"'),
        "half-opacity.ora": XML.replace('opacity="1.000"', 'opacity="0.5"'),
        "exponent-one.ora": XML.replace('opacity="1.000"', 'opacity="1e0"'),
        "group-offset.ora": XML.replace('name="日本語 outer"', 'name="日本語 outer" x="123"'),
        "group-blend.ora": XML.replace('composite-op="svg:src-over"', 'composite-op="svg:multiply"'),
        "leaf-blend.ora": XML.replace('name="Mid"', 'name="Mid" composite-op="svg:multiply"'),
        "hidden-isolate.ora": XML.replace('name="Duplicate" isolation="auto" visibility="hidden"',
            'name="Duplicate" isolation="isolate" visibility="hidden"'),
        "hidden-unknown.ora": XML.replace('<stack name="Duplicate" isolation="auto"/>', '<filter/>'),
        "old-version.ora": XML.replace('version="0.0.6"', 'version="0.0.3"')}
    for name, xml in cases.items():
        write(name, xml)
    altered = bytearray(merged()); altered[0] ^= 1
    write("different-projection.ora", XML, bytes(altered))
    write("policy-before-pixels.ora", cases["default-isolate.ora"], corrupt_png=True)
    write("large-thumbnail.ora", XML, thumbnail=(8, 8, bytes([1, 2, 3, 255]*64)))
    (ROOT/"merged.rgba").write_bytes(merged())
    for i, (*_, raw) in enumerate(LAYERS):
        (ROOT/f"layer-{i}.rgba").write_bytes(raw)
    manifest = {"generator": "Python3 zipfile/zlib and independent Fraction source-over per leaf",
        "group_names_xml_order": ["日本語 outer", "Duplicate", "Duplicate"],
        "layers_bottom_to_top": [{"name": name, "width": w, "height": h, "origin": origin,
            "effective_visible": visible, "rgba_sha256": hashlib.sha256(raw).hexdigest()}
            for name,w,h,origin,visible,raw in LAYERS],
        "files": {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in sorted(ROOT.iterdir()) if p.suffix in [".ora", ".rgba"]}}
    (ROOT/"oracle.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=4)+"\n")


if __name__ == "__main__":
    main()
