"""Owned BMP16 rows: exact rational scaling plus independent FFmpeg readback."""
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parent
WIDTH, HEIGHT = 65, 3
tool = Path(shutil.which("ffmpeg")).resolve()
records = []
for header in [40, 108, 124]:
    for mode in ["rgb555", "masks555", "masks565"]:
        for top in [False, True]:
            bits = 6 if mode.endswith("565") else 5
            masks = [0xF800, 0x07E0, 0x001F] if bits == 6 else [0x7C00, 0x03E0, 0x001F]
            rows, expected = [], bytearray()
            for y in range(HEIGHT):
                row = bytearray()
                for x in range(WIDTH):
                    r, g, b = (x + y * 7) % 32, (x + y * 11) % (1 << bits), (x * 3 + y) % 32
                    word = (r << (bits + 5)) | (g << 5) | b
                    # Bit 15 is unused in RGB555, not an alpha bit.
                    if bits == 5 and (x + y) % 2:
                        word |= 0x8000
                    row.extend(struct.pack("<H", word))
                    expected.extend([(r * 255 + 15) // 31, (g * 255 + ((1 << bits) - 1) // 2) // ((1 << bits) - 1), (b * 255 + 15) // 31, 255])
                row.extend(b"\xD3\xA7")  # Odd-width DWORD padding must be ignored.
                rows.append(row)
            pixels = b"".join(rows if top else reversed(rows))
            dib = bytearray(header)
            compression = 0 if mode == "rgb555" else 3
            struct.pack_into("<IiiHHIIiiII", dib, 0, header, WIDTH, -HEIGHT if top else HEIGHT, 1, 16, compression, len(pixels), 0, 0, 0, 0)
            extra = b""
            if compression:
                if header == 40:
                    extra = struct.pack("<III", *masks)
                else:
                    struct.pack_into("<IIII", dib, 40, *masks, 0)
            if header >= 108:
                struct.pack_into("<I", dib, 56, 0x73524742)
            offset = 14 + header + len(extra)
            source = struct.pack("<2sIHHI", b"BM", offset + len(pixels), 0, 0, offset) + dib + extra + pixels
            name = f"{mode}-v{header}-{'top' if top else 'bottom'}"
            (ROOT / (name + ".bmp")).write_bytes(source)
            (ROOT / (name + ".rgba")).write_bytes(expected)
            actual = subprocess.check_output([str(tool), "-v", "error", "-nostdin", "-i", str(ROOT / (name + ".bmp")), "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba", "-"], timeout=15)
            assert len(actual) == len(expected)
            delta = max(abs(a - b) for a, b in zip(actual, expected))
            assert delta <= 1, (name, delta)
            (ROOT / (name + ".ffmpeg.rgba")).write_bytes(actual)
            records.append({"file": name + ".bmp", "sha256": hashlib.sha256(source).hexdigest(), "expected_sha256": hashlib.sha256(expected).hexdigest(), "ffmpeg_sha256": hashlib.sha256(actual).hexdigest(), "ffmpeg_max_channel_difference": delta, "header": header, "mode": mode, "top_down": top, "masks": masks if compression else [0, 0, 0]})
report = {"schema": "paintcraft.bmp16-oracles/v1", "width": WIDTH, "height": HEIGHT, "scaling": "nearest integer sample*255/(2^bits-1); unused RGB555 bit15 ignored; opaque alpha", "ffmpeg": subprocess.check_output([str(tool), "-version"], text=True).splitlines()[0], "ffmpeg_binary_sha256": hashlib.sha256(tool.read_bytes()).hexdigest(), "fixtures": records}
(ROOT / "oracle.json").write_text(json.dumps(report, indent=2) + "\n")
print(f"{len(records)} BMP16 specimens, independent FFmpeg maximum difference {max(r['ffmpeg_max_channel_difference'] for r in records)}")
