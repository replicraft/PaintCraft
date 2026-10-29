"""Original PNG samples/profile framing; independent FFmpeg raw-sample checks."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import zlib

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
RGB = ROOT / "assets/profiles/sRGB2014.icc"
PASSES = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]
ROWS = []


def chunk(tag, payload):
    return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", zlib.crc32(tag + payload))


def gray_profile():
    # Structural gray source fixture, not a qualified color-transform oracle.
    data = bytearray(192)
    struct.pack_into(">II", data, 0, len(data), 0)
    struct.pack_into(">I", data, 8, 0x02400000)
    data[12:24] = b"mntrGRAYXYZ "
    struct.pack_into(">6H", data, 24, 2026, 10, 29, 0, 0, 0)
    data[36:40] = b"acsp"
    struct.pack_into(">3I", data, 68, 0xF6D6, 0x10000, 0xD32D)
    data[80:84] = b"PCFT"
    struct.pack_into(">I4sII4sII", data, 128, 2, b"wtpt", 156, 20, b"kTRC", 176, 16)
    data[156:160] = b"XYZ "
    struct.pack_into(">3I", data, 164, 0xF6D6, 0x10000, 0xD32D)
    data[176:180] = b"curv"
    struct.pack_into(">IH", data, 184, 1, 563)
    (HERE / "gray-structural.icc").write_bytes(data)
    return bytes(data)


GRAY = gray_profile()


def pack(values, depth):
    if depth == 16:
        return b"".join(struct.pack(">H", v) for v in values)
    if depth == 8:
        return bytes(values)
    result = bytearray((len(values) * depth + 7) // 8)
    for x, v in enumerate(values):
        result[x * depth // 8] |= v << (8 - depth - x * depth % 8)
    return result


def make(name, model, depth, interlaced=False, transparency=False, profile=True, phys=True, extra=False):
    width, height = 9, 7
    palette = [(13, 71, 201), (101, 23, 11), (90, 80, 70), (250, 127, 31)][:min(4, 1 << depth)]
    alpha = [0, 1, 127, 255][:len(palette)]
    maxima = 65535 if depth == 16 else 255
    series = [0, 1, 255, 256, 257, 32767, 32768, 65534, 65535]
    pixels = []
    for y in range(height):
        row = []
        for x in range(width):
            i = x + width * y
            gray = series[i % len(series)] if depth == 16 else (17 + i * 31) % 256
            rgb = tuple(series[(i + c * 3) % len(series)] for c in range(3)) if depth == 16 else ((17 + i * 31) % 256, (33 + i * 11) % 256, (55 + i * 23) % 256)
            a = series[(x + 2 * y) % len(series)] if depth == 16 else [0, 1, 127, 254, 255][i % 5]
            row.append({0: (gray,), 2: rgb, 3: (i % len(palette),), 4: (gray, a), 6: (*rgb, a)}[model])
        pixels.append(row)
    transparent = pixels[0][2] if transparency else None
    source_profile = GRAY if model in [0, 4] else RGB.read_bytes()
    before = []
    if model == 3:
        before += [(b"PLTE", bytes(v for p in palette for v in p)), (b"tRNS", bytes(alpha))]
    elif transparent is not None:
        before.append((b"tRNS", b"".join(struct.pack(">H", v) for v in transparent)))
    if profile:
        # PNG3 color declarations must precede PLTE, not merely IDAT.
        before.insert(0, (b"iCCP", b"Original profile\0\0" + zlib.compress(source_profile)))
    if phys:
        before.append((b"pHYs", struct.pack(">IIB", 3780, 3779, 1)))
    if extra:
        before += [(b"eXIf", b"II" + struct.pack("<HIHHHIHHI", 42, 8, 1, 274, 3, 1, 6, 0, 0)), (b"tEXt", b"Note\0Original fixture metadata")]
    raw = bytearray()
    for sx, sy, dx, dy in PASSES if interlaced else [(0, 0, 1, 1)]:
        xs, ys = list(range(sx, width, dx)), list(range(sy, height, dy))
        if not xs:
            continue
        for y in ys:
            raw += b"\0" + pack([v for x in xs for v in pixels[y][x]], depth)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, depth, model, 0, 0, int(interlaced)))
    data += b"".join(chunk(k, v) for k, v in before) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
    rgba = bytearray()
    for row in pixels:
        for p in row:
            if model == 3:
                rgba.extend((*palette[p[0]], alpha[p[0]]))
                continue
            rgb = (p[0],) * 3 if model in [0, 4] else p[:3]
            a = p[-1] if model in [4, 6] else (0 if transparent == p else maxima)
            rgba.extend(tuple(v >> 8 if depth == 16 else v for v in (*rgb, a)))
    (HERE / name).write_bytes(data)
    (HERE / (name + ".rgba")).write_bytes(rgba)
    native = subprocess.check_output(["ffmpeg", "-v", "error", "-nostdin", "-noautorotate", "-i", str(HERE / name), "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgba64be" if depth == 16 else "rgba", "-"])
    checked = native[::2] if depth == 16 else native
    if checked != rgba:
        (HERE / (name + ".ffmpeg.rgba")).write_bytes(checked)
        raise AssertionError(f"Independent FFmpeg samples differ: {name}")
    ROWS.append({"file": name, "sha256": hashlib.sha256(data).hexdigest(), "width": width, "height": height, "depth": depth, "model": model, "interlaced": interlaced, "transparency": transparency, "icc": profile, "physical": phys, "extra_omissions": extra, "profile_sha256": hashlib.sha256(source_profile).hexdigest() if profile else None, "rgba_sha256": hashlib.sha256(rgba).hexdigest()})


if __name__ == "__main__":
    for model, label in [(0, "gray"), (2, "rgb"), (4, "gray-alpha"), (6, "rgba")]:
        for depth in [8, 16]:
            for interlace in [False, True]:
                make(f"{label}{depth}{'-adam7' if interlace else ''}.png", model, depth, interlace)
    for model, label in [(0, "gray"), (2, "rgb")]:
        for depth in [8, 16]:
            make(f"{label}{depth}-trns.png", model, depth, transparency=True)
    for depth in [1, 2, 4, 8]:
        make(f"indexed{depth}-adam7.png", 3, depth, interlaced=True)
    make("rgba8-phys-only.png", 6, 8, profile=False)
    make("rgba8-omissions-only.png", 6, 8, profile=False, phys=False, extra=True)
    make("rgba8-profile-plus-omissions.png", 6, 8, extra=True)
    tool = Path(subprocess.check_output(["which", "ffmpeg"], text=True).strip()).resolve()
    (HERE / "oracle.json").write_text(json.dumps({"schema": "paintcraft.png-source-retention/1", "rights": "Original authored samples/framing; RGB profile retains assets/profiles provenance. Gray profile is structurally qualified only, not a CMS oracle.", "ffmpeg": subprocess.check_output([str(tool), "-version"], text=True).splitlines()[0], "tool_sha256": hashlib.sha256(tool.read_bytes()).hexdigest(), "fixtures": ROWS}, indent=2) + "\n")
    print(f"{len(ROWS)} original PNG cases match independent raw FFmpeg samples (explicit high-byte16-bit policy).")
