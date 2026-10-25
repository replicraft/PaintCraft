"""Independent ZIP/XML/PNG verification of the fixture exported by PaintCraft.

Uses only Python's standard library, never PaintCraft's reader. This checks the
serialized interchange structure and arithmetic fixture samples; it is not a
substitute for the separately required GIMP/Krita reopen and edit qualification.
"""
import binascii
import hashlib
import json
from pathlib import Path
import struct
import sys
import xml.etree.ElementTree as ET
import zipfile
import zlib


def png_rgba(data):
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    cursor = 8
    compressed = bytearray()
    width = height = None
    ended = False
    while cursor < len(data):
        size = struct.unpack_from(">I", data, cursor)[0]
        kind = data[cursor + 4:cursor + 8]
        payload = data[cursor + 8:cursor + 8 + size]
        crc = struct.unpack_from(">I", data, cursor + 8 + size)[0]
        assert crc == binascii.crc32(kind + payload)
        cursor += 12 + size
        if kind == b"IHDR":
            width, height, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload)
            assert (depth, color, compression, filtering, interlace) == (8, 6, 0, 0, 0)
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            assert not payload and cursor == len(data)
            ended = True
            break
    assert ended and width and height
    inflated = zlib.decompress(compressed)
    stride = width * 4
    assert len(inflated) == height * (stride + 1)
    output = bytearray()
    previous = bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        filtering = inflated[start]
        assert 0 <= filtering <= 4
        row = bytearray(inflated[start + 1:start + 1 + stride])
        for x in range(stride):
            left = row[x - 4] if x >= 4 else 0
            up = previous[x]
            corner = previous[x - 4] if x >= 4 else 0
            if filtering == 1:
                predictor = left
            elif filtering == 2:
                predictor = up
            elif filtering == 3:
                predictor = (left + up) // 2
            elif filtering == 4:
                estimate = left + up - corner
                distances = [abs(estimate - left), abs(estimate - up), abs(estimate - corner)]
                predictor = [left, up, corner][distances.index(min(distances))]
            else:
                predictor = 0
            row[x] = (row[x] + predictor) % 256
        output.extend(row)
        previous = row
    return width, height, bytes(output)


expected = json.loads((Path(__file__).parent / "oracle.json").read_text())
with zipfile.ZipFile(sys.argv[1]) as archive:
    entries = archive.infolist()
    assert entries[0].filename == "mimetype"
    assert entries[0].compress_type == zipfile.ZIP_STORED
    assert archive.read("mimetype") == b"image/openraster"
    assert archive.testzip() is None
    assert len(entries) == 7
    assert len({entry.filename for entry in entries}) == len(entries)
    root = ET.fromstring(archive.read("stack.xml"))
    assert root.tag == "image" and root.attrib == {"version": "0.0.6", "w": "3", "h": "2"}
    assert len(root) == 1 and root[0].tag == "stack" and root[0].attrib == {}
    layers = list(reversed(root[0]))
    assert len(layers) == 3
    for index, (layer, oracle) in enumerate(zip(layers, expected["layers_bottom_to_top"])):
        assert layer.tag == "layer"
        assert layer.attrib["name"] == oracle["name"]
        assert layer.attrib["composite-op"] == "svg:src-over"
        assert float(layer.attrib["opacity"]) == 1.0
        assert layer.attrib["visibility"] == ("hidden" if index == 2 else "visible")
        assert (int(layer.attrib["x"]), int(layer.attrib["y"])) == [(0, 0), (1, 1), (-1, 0)][index]
        width, height, pixels = png_rgba(archive.read(layer.attrib["src"]))
        assert (width, height) == [(3, 2), (2, 1), (1, 1)][index]
        assert hashlib.sha256(pixels).hexdigest() == oracle["rgba_sha256"]
    for name in ["mergedimage.png", "Thumbnails/thumbnail.png"]:
        width, height, pixels = png_rgba(archive.read(name))
        assert (width, height) == (3, 2)
        assert hashlib.sha256(pixels).hexdigest() == expected["merged_rgba_sha256"]
print(json.dumps({"zip": "verified", "xml": "verified", "layer_pixels": 3, "previews": 2}))
