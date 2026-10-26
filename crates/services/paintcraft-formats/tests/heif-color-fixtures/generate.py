"""Owned FFmpeg/libx265 elementary + authored HEIF C/libheif readback oracle.
Run in an empty output directory with pinned pillow-heif1.1.1/Pillow12.0.0.
No product writer or fixture normalization; original input bytes stay unchanged.
"""
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import sys

import PIL
import pillow_heif
from rational_oracle import rgb as rational_rgb
from libheif_oracle import decode as libheif_decode

ODD = len(sys.argv) > 2 and sys.argv[2] == "odd"
ANONYMOUS = len(sys.argv) > 2 and sys.argv[2] == "anonymous"
ITEM_NAME = b"" if ANONYMOUS else b"owned"
WIDTH, HEIGHT = (33, 25) if ODD else (32, 24)
OUTPUT = Path(sys.argv[1])
OUTPUT.mkdir(parents=True, exist_ok=True)
if any(OUTPUT.iterdir()):
    raise SystemExit("output must be empty")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def box(kind, data):
    return struct.pack(">I4s", len(data) + 8, kind) + data


def full(kind, data, version=0, flags=0):
    return box(kind, bytes([version]) + flags.to_bytes(3, "big") + data)


def u16(n):
    return struct.pack(">H", n)


def u32(n):
    return struct.pack(">I", n)


def units(data):
    return [part.rstrip(b"\0") for part in re.split(b"\x00\x00(?:\x00)?\x01", data)[1:]]


def configuration(data, chroma):
    nals = units(data)
    sps = next(nal for nal in nals if (nal[0] >> 1) & 63 == 33)
    rbsp = sps[2:].replace(b"\0\0\3", b"\0\0")
    header = bytearray(23)
    header[0] = 1
    header[1:13] = rbsp[1:13]
    header[13] = 0xf0
    header[15] = 0xfc
    header[16] = 0xfc | chroma
    header[17] = header[18] = 0xf8
    header[21] = 15
    header[22] = 3
    for kind in (32, 33, 34):
        nal = next(nal for nal in nals if (nal[0] >> 1) & 63 == kind)
        header += bytes([128 | kind]) + u16(1) + u16(len(nal)) + nal
    return box(b"hvcC", header)


def picture(data):
    return b"".join(u32(len(nal)) + nal for nal in units(data) if (nal[0] >> 1) & 63 < 32)


def container(stream, chroma, matrix, full_range, alpha=None):
    streams = [stream] + ([alpha] if alpha is not None else [])
    props = []
    items, associations, extents, payloads = [], [], [], []
    offset = 0
    for index, data in enumerate(streams, 1):
        is_alpha = index == 2
        items.append(full(b"infe", u16(index) + u16(0) + b"hvc1" + ITEM_NAME + b"\0", 2, int(is_alpha)))
        base = len(props)
        props += [
            full(b"ispe", u32(WIDTH) + u32(HEIGHT)),
            configuration(data, 0 if is_alpha else chroma),
            full(b"pixi", bytes([1, 8]) if is_alpha else bytes([3, 8, 8, 8])),
        ]
        encoding = (2, 2, 2, True) if is_alpha else (1, 13, matrix, full_range)
        props.append(box(b"colr", b"nclx" + b"".join(u16(n) for n in encoding[:3]) + bytes([128 if encoding[3] else 0])))
        if is_alpha:
            props.append(full(b"auxC", b"urn:mpeg:hevc:2015:auxid:1\0"))
        indices = bytes(range(base + 1, len(props) + 1))
        associations.append(u16(index) + bytes([len(indices)]) + indices)
        payload = picture(data)
        payloads.append(payload)
        extents.append(u16(index) + u16(1) + u16(0) + u16(1) + u32(offset) + u32(len(payload)))
        offset += len(payload)
    meta = full(b"hdlr", u32(0) + b"pict" + bytes(12) + b"PaintCraft owned fixture\0")
    meta += full(b"pitm", u16(1))
    meta += full(b"iinf", u16(len(items)) + b"".join(items))
    meta += box(b"iprp", box(b"ipco", b"".join(props)) + full(b"ipma", u32(len(items)) + b"".join(associations)))
    if alpha is not None:
        meta += full(b"iref", box(b"auxl", u16(2) + u16(1) + u16(1)))
    meta += full(b"iloc", bytes([0x44, 0]) + u16(len(items)) + b"".join(extents), 1)
    meta += box(b"idat", b"".join(payloads))
    return box(b"ftyp", b"heic" + u32(0) + b"mif1heic") + full(b"meta", meta)


def command(arguments):
    result = subprocess.run(arguments, capture_output=True, check=True)
    return result.stdout


def encode(name, raw, pixel_format, matrix, full_range):
    raw_path = OUTPUT / (name + ".raw")
    elementary = OUTPUT / (name + ".h265")
    raw_path.write_bytes(raw)
    parameters = (
        f"lossless=1:colorprim={2 if matrix == 2 else 1}:"
        f"transfer={2 if matrix == 2 else 13}:colormatrix={matrix}:"
        f"range={'full' if full_range else 'limited'}:repeat-headers=1:"
        "keyint=1:bframes=0:pools=none:frame-threads=1:wpp=0:log-level=error"
    )
    args = [
        "ffmpeg", "-hide_banner", "-loglevel", "error",
        "-f", "rawvideo", "-pixel_format", pixel_format,
        "-video_size", f"{WIDTH}x{HEIGHT}", "-i", str(raw_path),
        "-frames:v", "1", "-c:v", "libx265", "-x265-params", parameters,
        "-f", "hevc", str(elementary),
    ]
    command(args)
    decoded = command([
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(elementary),
        "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", pixel_format, "-",
    ])
    (OUTPUT / (name + ".ffmpeg-native.raw")).write_bytes(decoded)
    if decoded != raw:
        raise AssertionError(f"authored native samples differ: {name}")
    return elementary.read_bytes(), args, sum(a != b for a, b in zip(decoded, raw))


records = []
alpha_raw = bytes([0, 1, 127, 128, 254, 255][(x + y) % 6] for y in range(HEIGHT) for x in range(WIDTH))
alpha, alpha_command, alpha_differences = encode("alpha8", alpha_raw, "gray", 2, True)
for name, fmt, chroma, matrix, full_range in [
    ("identity444", "gbrp", 3, 0, True),
    ("yuv444-full", "yuv444p", 3, 1, True),
    ("yuv444-limited", "yuv444p", 3, 1, False),
    ("yuv420-full", "yuv420p", 1, 1, True),
    ("yuv420-limited", "yuv420p", 1, 1, False),
]:
    if ODD and chroma == 1:
        continue
    raw = bytearray()
    for channel in range(3):
        w, h = (WIDTH // 2, HEIGHT // 2) if chroma == 1 and channel else (WIDTH, HEIGHT)
        for y in range(h):
            for x in range(w):
                if matrix == 0:
                    sample = (x * 7 + y * 11 + channel * 61) % 256
                elif channel == 0:
                    sample = 32 + ((x * 3 + y * 5) % 176)
                else:
                    sample = 96 + ((x * 5 + y * 3 + channel * 11) % 64)
                raw.append(sample)
    stream, args, differences = encode(name, bytes(raw), fmt, matrix, full_range)
    scale = (
        f"scale=in_range={'full' if full_range else 'limited'}:out_range=full:"
        "in_color_matrix=bt709:flags=bilinear+accurate_rnd+full_chroma_int:"
        f"in_h_chr_pos=0:in_v_chr_pos={128 if chroma == 1 else 0}"
    )
    arguments = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(OUTPUT / (name + ".h265"))]
    if matrix != 0:
        arguments += ["-vf", scale]
    arguments += ["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]
    reference_rgb = command(arguments)
    scalar_rgb = rational_rgb(bytes(raw), WIDTH, HEIGHT, chroma, matrix, full_range)
    if max(abs(a - b) for a, b in zip(scalar_rgb, reference_rgb)) > 1:
        raise AssertionError(f"independent FFmpeg/rational RGB mismatch: {name}")
    (OUTPUT / (name + ".rational-rgb")).write_bytes(scalar_rgb)
    (OUTPUT / (name + ".ffmpeg-rgb")).write_bytes(reference_rgb)
    for with_alpha in (False, True):
        stem = name + ("-alpha" if with_alpha else "")
        encoded = container(stream, chroma, matrix, full_range, alpha if with_alpha else None)
        path = OUTPUT / (stem + ".heic")
        path.write_bytes(encoded)
        image = pillow_heif.open_heif(path, convert_hdr_to_8bit=False)
        pixels = bytes(image.data)
        (OUTPUT / (stem + ".libheif-" + image.mode.lower())).write_bytes(pixels)
        if path.read_bytes() != encoded:
            raise AssertionError("oracle changed source")
        for algorithm, label in ((1, "nearest"), (2, "bilinear")):
            converted = libheif_decode(path, with_alpha, algorithm, width=WIDTH, height=HEIGHT)
            (OUTPUT / (stem + ".libheif-" + label + ("-rgba" if with_alpha else "-rgb"))).write_bytes(converted)
        if chroma == 1:
            native = libheif_decode(path, with_alpha, 2, native=True, width=WIDTH, height=HEIGHT)
            if native != raw:
                raise AssertionError(f"independent libheif native planes differ: {stem}")
        if path.read_bytes() != encoded:
            raise AssertionError("C oracle changed source")
        records.append({
            "name": stem,
            "size": image.size,
            "mode": image.mode,
            "stride": image.stride,
            "source_sha256": sha(encoded),
            "authored_samples_sha256": sha(raw),
            "ffmpeg_native_differences_from_authored": differences,
            "libheif_pixels_sha256": sha(pixels),
            "encode_command": args,
            "rgb_command": arguments,
            "ffmpeg_rgb_sha256": sha(reference_rgb),
            "rational_rgb_sha256": sha(scalar_rgb),
            "libheif_bilinear_sha256": sha(converted),
            "nclx": [1, 13, matrix, full_range],
            "alpha": with_alpha,
        })
report = {
    "pillow_heif": pillow_heif.__version__,
    "pillow": PIL.__version__,
    "libheif": pillow_heif.libheif_info(),
    "ffmpeg": command(["ffmpeg", "-version"]).decode().splitlines()[0],
    "fixtures": records,
    "alpha_encode_command": alpha_command,
    "alpha_native_differences_from_authored": alpha_differences,
}
report["native_library_sha256"] = {
    path.name: sha(path.read_bytes())
    for path in (Path(pillow_heif.__file__).parent / ".dylibs").glob("*.dylib")
}
(OUTPUT / "oracle.json").write_text(json.dumps(report, indent=4) + "\n")
