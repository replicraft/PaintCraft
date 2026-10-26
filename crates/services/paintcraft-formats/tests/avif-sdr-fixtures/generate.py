#!/usr/bin/env python3
"""Owned lossless AV1 streams, minimal AVIF wrappers and exact rational RGB oracle."""
import argparse
from fractions import Fraction as F
import hashlib
import json
from pathlib import Path
import struct
import subprocess

U16 = lambda n: struct.pack(">H", n)
U32 = lambda n: struct.pack(">I", n)
ALPHA = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0"


def box(kind, data):
    return U32(len(data) + 8) + kind.encode() + data


def full(kind, data, version=0):
    return box(kind, bytes([version, 0, 0, 0]) + data)


def sequence_prefix(data):
    offset = 0
    while offset < len(data):
        kind = data[offset] >> 3 & 15
        offset += 1
        size = shift = 0
        while True:
            byte = data[offset]
            offset += 1
            size |= (byte & 127) << shift
            shift += 7
            if byte < 128:
                break
        if kind == 1:
            prefix = data[offset:offset + size]
            return prefix[0] >> 5, ((prefix[0] & 7) << 2) | (prefix[1] >> 6)
        offset += size
    raise ValueError("No sequence header")


def container(width, height, coded, alpha, layout, position, limited):
    profile, level = sequence_prefix(coded)
    assert profile == (1 if layout == "444" else 0)
    av1c = bytes([129, profile << 5 | level, (12 if layout == "420" else 0) | position, 0])
    props = [full("ispe", U32(width) + U32(height)), box("av1C", av1c),
             full("pixi", bytes([3, 8, 8, 8])),
             box("colr", b"nclx" + U16(1) + U16(13) + U16(1) + bytes([0 if limited else 128]))]
    if alpha:
        alpha_profile, alpha_level = sequence_prefix(alpha)
        assert alpha_profile == 0
        props.extend([box("av1C", bytes([129, alpha_level, 28, 0])), full("pixi", bytes([1, 8])), full("auxC", ALPHA)])
    items = [(1, coded, [1, 130, 3, 4])]
    if alpha:
        items.append((2, alpha, [1, 133, 6, 135]))

    def meta(start):
        offset = start
        locations = b""
        infos = b""
        associations = b""
        for ident, data, indices in items:
            locations += U16(ident) + U16(0) + U16(1) + U32(offset) + U32(len(data))
            infos += full("infe", U16(ident) + U16(0) + b"av01\0", 2)
            associations += U16(ident) + bytes([len(indices), *indices])
            offset += len(data)
        handler = full("hdlr", U32(0) + b"pict" + bytes(12) + b"PaintCraft owned SDR fixture\0")
        refs = full("iref", box("auxl", U16(2) + U16(1) + U16(1))) if alpha else b""
        return full("meta", handler + full("pitm", U16(1))
                    + full("iloc", bytes([68, 0]) + U16(len(items)) + locations)
                    + full("iinf", U16(len(items)) + infos)
                    + box("iprp", box("ipco", b"".join(props)) + full("ipma", U32(len(items)) + associations))
                    + refs)
    ftyp = box("ftyp", b"avif" + U32(0) + b"avifmif1")
    metadata = meta(0)
    metadata = meta(len(ftyp) + len(metadata) + 8)
    return ftyp + metadata + box("mdat", b"".join(data for _, data, _ in items))


def yuv(width, height, layout, alpha=False):
    if alpha:
        return bytes([0, 1, 127, 128, 254, 255][(x + y) % 6] for y in range(height) for x in range(width))
    divisor = 2 if layout == "420" else 1
    cw, ch = (width + divisor - 1) // divisor, (height + divisor - 1) // divisor
    luma = bytes((x * 17 + y * 23) % 256 for y in range(height) for x in range(width))
    cb = bytes([0, 16, 64, 128, 192, 240, 255][(x + 3 * y) % 7] for y in range(ch) for x in range(cw))
    cr = bytes([255, 240, 192, 128, 64, 16, 0][(3 * x + y) % 7] for y in range(ch) for x in range(cw))
    return luma + cb + cr


def sample(plane, width, height, x, y):
    x, y = max(F(0), min(x, F(width - 1))), max(F(0), min(y, F(height - 1)))
    ix, iy = int(x), int(y)
    fx, fy = x - ix, y - iy
    def at(dx, dy):
        return plane[min(iy + dy, height - 1) * width + min(ix + dx, width - 1)]
    return at(0, 0) * (1 - fx) * (1 - fy) + at(1, 0) * fx * (1 - fy) + at(0, 1) * (1 - fx) * fy + at(1, 1) * fx * fy


def rational(width, height, source, alpha, layout, position, limited):
    count, divisor = width * height, 2 if layout == "420" else 1
    cw, ch = (width + divisor - 1) // divisor, (height + divisor - 1) // divisor
    plane_len = cw * ch
    cb, cr = source[count:count + plane_len], source[count + plane_len:]
    kr, kb = F(2126, 10000), F(722, 10000)
    result = bytearray()
    for y in range(height):
        for x in range(width):
            cx = F(x, divisor)
            cy = (F(y) - (F(1, 2) if position == 1 else 0)) / divisor
            u = sample(cb, cw, ch, cx, cy) - 128
            v = sample(cr, cw, ch, cx, cy) - 128
            lum = F(source[y * width + x])
            if limited:
                lum = (lum - 16) * F(255, 219)
                u, v = u * F(255, 224), v * F(255, 224)
            r, b = lum + 2 * (1 - kr) * v, lum + 2 * (1 - kb) * u
            g = (lum - kr * r - kb * b) / (1 - kr - kb)
            for value in [r, g, b]:
                result.append(min(255, max(0, int((value + F(1, 2)).numerator // (value + F(1, 2)).denominator))))
            result.append(alpha[y * width + x] if alpha else 255)
    return bytes(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder", type=Path, required=True)
    parser.add_argument("--oracle", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path(__file__).parent)
    parser.add_argument("--raw-log", type=Path, help="Optional ignored log of actual local invocations")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    commands, references, invocations = [], {}, []

    def invoke(command):
        result = subprocess.run(command, capture_output=True, text=True)
        invocations.append({"command": command, "exit_code": result.returncode,
                            "stdout": result.stdout, "stderr": result.stderr})
        if args.raw_log:
            args.raw_log.parent.mkdir(parents=True, exist_ok=True)
            args.raw_log.write_text(json.dumps(invocations, indent=2) + "\n")
        result.check_returncode()
        return result.stdout

    def encode(name, width, height, layout, position, limited, alpha=False):
        data = yuv(width, height, layout, alpha)
        path = args.output / name
        path.with_suffix(".yuv").write_bytes(data)
        command = [str(args.encoder), str(path.with_suffix(".obu")), str(width), str(height),
                   "mono" if alpha else layout, str(position), str(int(not limited)), str(path.with_suffix(".yuv"))]
        invoke(command)
        commands.append(["$ENCODER", *[value.replace(str(args.output) + "/", "") for value in command[1:]]])
        reference = invoke([str(args.oracle), str(path.with_suffix(".obu")), str(path)])
        references[name] = json.loads(reference)
        planes = path.with_suffix(".planes").read_bytes()
        count = width * height
        if alpha:
            expected = data + bytes(count * 2)
        elif layout == "420":
            chroma = ((width + 1) // 2) * ((height + 1) // 2)
            expected = data[:count] + data[count:count + chroma].ljust(count, b'\0') + data[count + chroma:].ljust(count, b'\0')
        else:
            expected = data
        assert planes == expected, name
        return path.with_suffix(".obu").read_bytes(), data

    for width, height, size in [(16, 16, "even"), (17, 13, "odd")]:
        alpha, a = encode("alpha-" + size, width, height, "420", 0, False, True)
        for layout, positions in [("444", [0]), ("420", [0, 1, 2])]:
            for position in positions:
                for limited in [False, True]:
                    name = f"{layout}-{'vertical' if position == 1 else 'colocated' if position == 2 else 'unknown' if layout == '420' else 'fullchroma'}-{'limited' if limited else 'full'}-{size}"
                    coded, native = encode(name, width, height, layout, position, limited)
                    for transparent in [False, True]:
                        suffix = "-alpha" if transparent else ""
                        (args.output / (name + suffix + ".avif")).write_bytes(container(width, height, coded, alpha if transparent else None, layout, position, limited))
                        if layout == "444" or position != 0:
                            (args.output / (name + suffix + ".rational.rgba")).write_bytes(rational(width, height, native, a if transparent else None, layout, position, limited))
    hashes = {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(args.output.iterdir()) if path.suffix in (".obu", ".planes", ".avif", ".rgba")}
    (args.output / "oracle.json").write_text(json.dumps({"libaom_version": "3.15.2", "commit": "af3dc9aadc793c1b00edc003e3fc19a44fdeb574", "archive_sha256": "01d2e7a4a04a4c57d33eba487ff00f5cea29932d95a848de760d800adb8054a2", "build_options": ["AOM_TARGET_CPU=generic", "ENABLE_DOCS=OFF", "ENABLE_EXAMPLES=OFF", "ENABLE_TESTS=OFF", "ENABLE_TOOLS=OFF", "CONFIG_AV1_ENCODER=1", "CONFIG_AV1_DECODER=1", "BUILD_SHARED_LIBS=OFF", "CMAKE_BUILD_TYPE=Release"], "tools": {"encoder_sha256": hashlib.sha256(args.encoder.read_bytes()).hexdigest(), "oracle_sha256": hashlib.sha256(args.oracle.read_bytes()).hexdigest()}, "harness_sha256": {name: hashlib.sha256((Path(__file__).parent / name).read_bytes()).hexdigest() for name in ["libaom_encode.c", "libaom_oracle.c", "generate.py"]}, "encoder_options": commands, "reference_decodes": references, "sha256": hashes}, indent=2) + "\n")


if __name__ == "__main__":
    main()
