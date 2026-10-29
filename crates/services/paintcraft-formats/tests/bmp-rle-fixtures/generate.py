"""Owned BI_RLE4/BI_RLE8 command streams, independent FFmpeg pixel oracle."""
from pathlib import Path
import hashlib
import json
import struct
import subprocess
import PIL
from PIL import Image

ROOT = Path(__file__).resolve().parent
PALETTE = [(13, 71, 201)] + [((i * 31) % 256, (i * 67) % 256, (i * 97) % 256) for i in range(1, 16)]


def bmp(name, depth, width, rows, stream, *, image_size=None, extra=b"", resolution=(0, 0), profile=None):
    assert all(len(row) == width for row in rows)
    count = 13 if depth == 4 else 16
    palette = b"".join(bytes((b, g, r, 137 if i == 0 else 0)) for i, (r, g, b) in enumerate(PALETTE[:count]))
    dib = bytearray(struct.pack("<IiiHHIIiiII", 40, width, len(rows), 1, depth, 2 if depth == 4 else 1, len(stream) if image_size is None else image_size, *resolution, count, 0))
    icc_before = icc_after = b""
    if profile:
        original = (ROOT.parent / "raster-metadata-fixtures/rgb24-v5-icc.bmp").read_bytes()
        begin, length = struct.unpack_from("<II", original, 126)
        icc = original[14 + begin:14 + begin + length]
        dib.extend(bytes(84))
        struct.pack_into("<I", dib, 0, 124)
        struct.pack_into("<I", dib, 56, 0x4D424544)
        struct.pack_into("<I", dib, 108, 4)
        at = 14 + len(dib) + len(palette) + (len(stream) + len(extra) if profile == "after" else 0)
        struct.pack_into("<II", dib, 112, at - 14, len(icc))
        if profile == "before":
            icc_before = icc
        else:
            icc_after = icc
    offset = 14 + len(dib) + len(palette) + len(icc_before)
    storage = len(stream) if image_size is None else image_size
    header = struct.pack("<2sIHHI", b"BM", offset + len(stream) + len(extra) + len(icc_after), 0, 0, offset)
    header += dib
    data = header + palette + icc_before + bytes(stream) + extra + icc_after
    (ROOT / name).write_bytes(data)
    expected = [component for row in reversed(rows) for index in row for component in (*PALETTE[index], 255)]
    actual = list(subprocess.check_output(["ffmpeg", "-v", "error", "-nostdin", "-i", str(ROOT / name), "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"]))
    assert actual == expected, name
    try:
        pillow = list(Image.open(ROOT / name).convert("RGBA").tobytes())
        pillow_error = None
    except (OSError, ValueError) as error:
        pillow, pillow_error = None, str(error)
    return {"file": name, "sha256": hashlib.sha256(data).hexdigest(), "width": width, "height": len(rows), "bpp": depth, "compression": 2 if depth == 4 else 1, "image_size": storage, "stream_bytes": len(stream), "rows_bottom_up": rows, "expected_rgba": expected, "ffmpeg_rgba": actual, "pillow_rgba": pillow, "pillow_error": pillow_error, "icc_position": profile}



def main():
    # Runs + odd absolute payload/word padding + horizontal/vertical deltas +
    # skipped full rows + early EOI holes. Palette0 is deliberately not black.
    stream8 = [3, 1, 0, 5, 2, 3, 4, 5, 6, 0, 3, 7, 0, 0,
               0, 2, 2, 0, 4, 2, 0, 2, 1, 2, 4, 3, 0, 0,
               0, 11, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0, 0, 0,
               5, 4, 0, 1]
    rows8 = [[1, 1, 1, 2, 3, 4, 5, 6, 7, 7, 7], [0, 0, 2, 2, 2, 2, 0, 0, 0, 0, 0],
             [0] * 11, [0] * 7 + [3] * 4, list(range(1, 12)), [4] * 5 + [0] * 6]
    stream4 = [5, 0x12, 0, 6, 0x34, 0x56, 0x78, 0, 0, 0,
               0, 2, 2, 0, 3, 0x9A, 0, 2, 2, 2, 4, 0xBC, 0, 0,
               0, 11, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBF, 0, 0,
               1, 0xCF, 4, 0xAB, 0, 1]
    rows4 = [[1, 2, 1, 2, 1, 3, 4, 5, 6, 7, 8], [0, 0, 9, 10, 9, 0, 0, 0, 0, 0, 0],
             [0] * 11, [0] * 7 + [11, 12, 11, 12], list(range(1, 12)), [12, 10, 11, 10, 11] + [0] * 6]
    cases = [bmp("rle8-combined.bmp", 8, 11, rows8, stream8), bmp("rle4-combined.bmp", 4, 11, rows4, stream4)]
    for depth, stream, rows in [(8, stream8, rows8), (4, stream4, rows4)]:
        cases.append(bmp(f"rle{depth}-size-zero.bmp", depth, 11, rows, stream, image_size=0))
        cases.append(bmp(f"rle{depth}-padding.bmp", depth, 11, rows, stream + [0, 0], resolution=(-3780, 0)))
        cases[-1]["stream_bytes"] -= 2
        # Last EOL followed by EOI is valid, as is an empty bitmap of palette0.
        cases.append(bmp(f"rle{depth}-final-eol.bmp", depth, 3, [[1, 1, 1]], [3, 0x11 if depth == 4 else 1, 0, 0, 0, 1]))
        cases.append(bmp(f"rle{depth}-empty.bmp", depth, 3, [[0] * 3, [0] * 3], [0, 1]))
        cases.append(bmp(f"rle{depth}-zero-delta.bmp", depth, 3, [[1, 1, 1]], [0, 2, 0, 0, 3, 0x11 if depth == 4 else 1, 0, 1]))
        indices = [i % 13 for i in range(255)]
        encoded = list(indices) if depth == 8 else [(indices[i] << 4) | (indices[i+1] if i+1 < len(indices) else 15) for i in range(0, len(indices), 2)]
        if len(encoded) % 2:
            encoded.append(0)
        cases.append(bmp(f"rle{depth}-absolute255.bmp", depth, 255, [indices], [0, 255] + encoded + [0, 1]))
        cases.append(bmp(f"rle{depth}-run255.bmp", depth, 255, [[7] * 255], [255, 0x77 if depth == 4 else 7, 0, 1]))
        indices6 = [1, 2, 3, 4, 5, 6]
        encoded6 = indices6 if depth == 8 else [0x12, 0x34, 0x56, 0xA5]
        cases.append(bmp(f"rle{depth}-absolute6.bmp", depth, 6, [indices6], [0, 6] + encoded6 + [0, 1]))
    for depth, stream, rows in [(8, stream8, rows8), (4, stream4, rows4)]:
        for position in ["before", "after"]:
            for zero in [False, True]:
                cases.append(bmp(f"rle{depth}-icc-{position}{'-size-zero' if zero else ''}.bmp", depth, 11, rows, stream, image_size=0 if zero else None, profile=position))
    ffmpeg_version = subprocess.check_output(["ffmpeg", "-version"], text=True).splitlines()[0]
    (ROOT / "oracle.json").write_text(json.dumps({"schema": "paintcraft.bmp-rle-fixtures/1", "pillow": PIL.__version__, "ffmpeg": ffmpeg_version, "palette": PALETTE, "fixtures": cases}, indent=2) + "\n")
    print(f"{ffmpeg_version}: {len(cases)} owned RLE4/RLE8 images match explicitly authored raw index rows. Pillow{PIL.__version__} disagreements retained.")


if __name__ == "__main__":
    main()
