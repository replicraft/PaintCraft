"""Owned BI_RGB source rows, independently checked with FFmpeg raw RGBA output."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parent
PALETTE = [(231, 17, 43), (9, 197, 81), (25, 53, 239), (201, 149, 7)]


def make(depth, top_down, short_palette=False):
    width, height = 13, 3
    colors = 2 if depth == 1 else 4
    rows = [[(x + y) % colors for x in range(width)] for y in range(height)]
    stride = ((width * depth + 31) // 32) * 4
    encoded = bytearray()
    for row in rows if top_down else reversed(rows):
        packed = bytearray([0xFF] * stride)
        for x, index in enumerate(row):
            shift = 8 - depth - (x * depth % 8)
            at = x * depth // 8
            packed[at] = (packed[at] & ~( ((1 << depth) - 1) << shift)) | (index << shift)
        encoded.extend(packed)
    declared = colors - 1 if short_palette else colors
    palette = b"".join(bytes([b, g, r, 137 + i]) for i, (r, g, b) in enumerate(PALETTE[:declared]))
    offset = 54 + len(palette)
    dib = struct.pack("<IiiHHIIiiII", 40, width, -height if top_down else height, 1, depth, 0, len(encoded), 0, 0, declared, 0)
    data = struct.pack("<2sIHHI", b"BM", offset + len(encoded), 0, 0, offset) + dib + palette + encoded
    name = f"rgb{depth}-{'top' if top_down else 'bottom'}{'-undeclared' if short_palette else ''}.bmp"
    (ROOT / name).write_bytes(data)
    expected = bytes(v for row in rows for index in row for v in (*PALETTE[index], 255))
    if not short_palette:
        actual = subprocess.check_output(["ffmpeg", "-v", "error", "-nostdin", "-i", str(ROOT / name), "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        assert actual == expected, name
        (ROOT / name.replace(".bmp", ".rgba")).write_bytes(actual)
    return {"file": name, "sha256": hashlib.sha256(data).hexdigest(), "width": width, "height": height, "bpp": depth, "top_down": top_down, "palette_entries": declared, "valid": not short_palette, "rows_top_down": rows}


if __name__ == "__main__":
    cases = [make(depth, top) for depth in [1, 4, 8] for top in [False, True]]
    cases += [make(depth, False, True) for depth in [1, 4, 8]]
    tool = Path(subprocess.check_output(["which", "ffmpeg"], text=True).strip()).resolve()
    oracle = {"schema": "paintcraft.bmp-indexed-fixtures/1", "ffmpeg": subprocess.check_output([str(tool), "-version"], text=True).splitlines()[0], "tool_sha256": hashlib.sha256(tool.read_bytes()).hexdigest(), "palette": PALETTE, "fixtures": cases}
    (ROOT / "oracle.json").write_text(json.dumps(oracle, indent=2) + "\n")
    icons = []
    for depth in [1, 4, 8]:
        for invalid in [False, True]:
            stem = f"rgb{depth}-bottom{'-undeclared' if invalid else ''}"
            bmp = (ROOT / (stem + ".bmp")).read_bytes()
            dib = bytearray(bmp[14:])
            dib[8:12] = struct.pack("<i", 6)
            # Only top-left and bottom-right authored pixels are masked.
            mask = bytearray(12)
            mask[1] = 0x08
            mask[8] = 0x80
            dib.extend(mask)
            directory = struct.pack("<HHHBBBBHHII", 0, 1, 1, 13, 3, 0, 0, 1, depth, len(dib), 22)
            (ROOT / (stem + ".ico")).write_bytes(directory + dib)
            icons.append({"file": stem + ".ico", "sha256": hashlib.sha256(directory + dib).hexdigest(), "valid": not invalid})
            if not invalid:
                expected = bytearray((ROOT / (stem + ".rgba")).read_bytes())
                expected[3] = 0
                expected[(39 - 1) * 4 + 3] = 0
                actual = subprocess.check_output(["ffmpeg", "-v", "error", "-nostdin", "-i", str(ROOT / (stem + ".ico")), "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
                # This FFmpeg ICO path ignores the AND mask. Retain that
                # disagreement; RGB matches its independent BMP oracle, while
                # expected alpha follows the two explicitly authored mask bits.
                assert actual == (ROOT / (stem + ".rgba")).read_bytes(), stem + ".ico RGB"
                (ROOT / (stem + ".ico.ffmpeg.rgba")).write_bytes(actual)
                (ROOT / (stem + ".ico.rgba")).write_bytes(expected)
    oracle["icons"] = icons
    oracle["ico_alpha_oracle"] = "Explicit authored AND bits; FFmpeg ignores AND and its differing raw output is retained."
    (ROOT / "oracle.json").write_text(json.dumps(oracle, indent=2) + "\n")
    print("6 valid BI_RGB cases match independent FFmpeg pixels; 3 undeclared-index cases retained.")
