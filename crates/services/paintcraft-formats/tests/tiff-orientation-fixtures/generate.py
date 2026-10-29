"""Author classic TIFF scanlines and explicit orientation coordinate oracles.

No PaintCraft encoder/decoder or image library participates in fixture creation.
The label orders are written independently for a source laid out ABC / DEF.
"""
import hashlib
import json
import struct
from pathlib import Path


ORDERS = ("ABCDEF", "CBAFED", "FEDCBA", "DEFABC",
          "ADBECF", "DAEBFC", "FCEBDA", "CFBEAD")
RGBA = [11, 22, 33, 0, 44, 55, 66, 1, 77, 88, 99, 127,
        111, 122, 133, 128, 144, 155, 166, 254, 177, 188, 199, 255]
GRAY_ALPHA = [129, 0, 43981, 1, 65535, 32768,
              256, 32896, 32767, 65534, 39030, 65535]


def make_tiff(orientations, endian, gray):
    """Each IFD owns a contiguous, uncompressed 3x2 straight-alpha strip."""
    pack = lambda fmt, *values: struct.pack(endian + fmt, *values)
    out = bytearray((b"II" if endian == "<" else b"MM") + pack("HI", 42, 8))
    previous_next = None
    for orientation in orientations:
        at = len(out)
        if previous_next is not None:
            out[previous_next:previous_next + 4] = pack("I", at)
        channels, bits = (2, 16) if gray else (4, 8)
        samples = GRAY_ALPHA if gray else RGBA
        tags = [(256, 4, [3]), (257, 4, [2]), (258, 3, [bits] * channels),
                (259, 3, [1]), (262, 3, [1 if gray else 2]),
                (273, 4, [0]), (274, 3, [orientation]), (277, 3, [channels]),
                (278, 4, [2]), (279, 4, [len(samples) * (bits // 8)]),
                (284, 3, [1]), (338, 3, [2]), (339, 3, [1] * channels)]
        out.extend(pack("H", len(tags)))
        fields_at = len(out)
        out.extend(bytes(12 * len(tags) + 4))
        previous_next = fields_at + 12 * len(tags)
        strip_pointer = None
        for index, (tag, kind, values) in enumerate(tags):
            field = fields_at + index * 12
            raw = pack(("H" if kind == 3 else "I") * len(values), *values)
            out[field:field + 8] = pack("HHI", tag, kind, len(values))
            if len(raw) > 4:
                out[field + 8:field + 12] = pack("I", len(out))
                out.extend(raw)
            else:
                out[field + 8:field + 12] = raw.ljust(4, b"\0")
            if tag == 273:
                strip_pointer = field + 8
        out[strip_pointer:strip_pointer + 4] = pack("I", len(out))
        out.extend(pack(("H" if gray else "B") * len(samples), *samples))
    return bytes(out)


def page_record(orientation, gray):
    if gray:
        raw_rgba = []
        for index in range(0, len(GRAY_ALPHA), 2):
            value, alpha = (sample >> 8 for sample in GRAY_ALPHA[index:index + 2])
            raw_rgba.extend([value, value, value, alpha])
    else:
        raw_rgba = RGBA
    order = ORDERS[orientation - 1]
    rgba = [value for label in order
            for value in raw_rgba[(ord(label) - 65) * 4:(ord(label) - 64) * 4]]
    return {"orientation": orientation, "source_width": 3, "source_height": 2,
            "width": 2 if orientation >= 5 else 3,
            "height": 3 if orientation >= 5 else 2,
            "labels": order, "rgba": rgba,
            "rgba_sha256": hashlib.sha256(bytes(rgba)).hexdigest()}


def main():
    folder = Path(__file__).resolve().parent
    fixtures = []
    for prefix, endian, gray in (("rgba8-le", "<", False),
                                 ("rgba8-be", ">", False),
                                 ("gray-alpha16-be", ">", True)):
        for orientation in range(1, 9):
            name = f"{prefix}-orientation-{orientation}.tif"
            data = make_tiff([orientation], endian, gray)
            (folder / name).write_bytes(data)
            fixtures.append({"file": name, "sha256": hashlib.sha256(data).hexdigest(),
                             "gray": gray, "little_endian": endian == "<",
                             "pages": [page_record(orientation, gray)]})
    name = "rgba8-le-eight-orientations.tif"
    data = make_tiff(range(1, 9), "<", False)
    (folder / name).write_bytes(data)
    fixtures.append({"file": name, "sha256": hashlib.sha256(data).hexdigest(),
                     "gray": False, "little_endian": True,
                     "pages": [page_record(value, False) for value in range(1, 9)]})
    oracle = {"source_layout": "ABC/DEF", "source_rgba8": RGBA,
              "source_gray_alpha16": GRAY_ALPHA,
              "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "fixtures": fixtures}
    (folder / "oracle.json").write_text(json.dumps(oracle, indent=4) + "\n")


if __name__ == "__main__":
    main()
