"""Authored ICO/CUR bytes and independent Pillow pixel checks, no ico library."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import zlib


def chunk(tag, body):
    return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body))


def entry(size, depth):
    palette = [bytes((i * 17 % 256, i * 37 % 256, i * 61 % 256)) for i in range(1 << depth)] if depth <= 8 else []
    rgba = bytearray()
    for y in range(size):
        for x in range(size):
            rgb = palette[(x + y * 3) % len(palette)] if palette else bytes((x * 11 % 256, y * 13 % 256, (x + y) * 7 % 256))
            alpha = (x * 19 + y * 23) % 256 if depth == 32 else 255
            rgba.extend(rgb + bytes((alpha,)))
    if size == 256:
        raw = b"".join(b"\0" + rgba[y * size * 4:(y + 1) * size * 4] for y in range(size))
        payload = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")
        return payload, bytes(rgba), 0
    stride = ((size * depth + 31) // 32) * 4
    mask_stride = ((size + 31) // 32) * 4
    color_rows, mask_rows = bytearray(), bytearray()
    for y in reversed(range(size)):
        row, mask = bytearray(stride), bytearray(mask_stride)
        for x in range(size):
            pixel = (y * size + x) * 4
            if depth <= 8:
                index = (x + y * 3) % len(palette)
                shift = 8 - depth - ((x * depth) % 8)
                row[x * depth // 8] |= index << shift
            else:
                target = x * (depth // 8)
                row[target:target + 3] = rgba[pixel:pixel + 3][::-1]
                if depth == 32:
                    row[target + 3] = rgba[pixel + 3]
            if depth != 32 and (x + y) % 7 == 0:
                mask[x // 8] |= 0x80 >> (x % 8)
                rgba[pixel + 3] = 0
        color_rows.extend(row)
        mask_rows.extend(mask)
    header = struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, depth, 0, 0, 0, 0, len(palette), 0)
    payload = header + b"".join(rgb[::-1] + b"\0" for rgb in palette) + color_rows + mask_rows
    return payload, bytes(rgba), len(palette)


def container(entries, cursor):
    table, payloads = bytearray(), bytearray()
    for index, (size, depth, payload, colors) in enumerate(entries):
        first, second = (index, index + 1) if cursor else (1, depth)
        table.extend(struct.pack("<BBBBHHII", size % 256, size % 256, colors % 256, 0, first, second, len(payload), 6 + len(entries) * 16 + len(payloads)))
        payloads.extend(payload)
    return struct.pack("<HHH", 0, 2 if cursor else 1, len(entries)) + table + payloads


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    folder = Path(__file__).parent
    entries, hashes = [], []
    for size, depth in [(17, 1), (19, 4), (21, 8), (23, 24), (25, 32), (256, 32)]:
        payload, rgba, colors = entry(size, depth)
        entries.append((size, depth, payload, colors))
        hashes.append({"size": size, "depth": depth, "rgba_sha256": hashlib.sha256(rgba).hexdigest()})
    files = {"mixed.ico": container(entries, False), "mixed.cur": container(entries, True)}
    manifest = {"entries": hashes, "files": {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}}
    if args.generate:
        for name, data in files.items():
            (folder / name).write_bytes(data)
        (folder / "manifest.json").write_text(json.dumps(manifest, indent=4) + "\n")
    assert json.loads((folder / "manifest.json").read_text()) == manifest
    for name, data in files.items():
        assert (folder / name).read_bytes() == data
    import PIL
    from PIL import Image
    with Image.open(io.BytesIO(files["mixed.ico"])) as image:
        assert image.ico.sizes() == {(row["size"], row["size"]) for row in hashes}
        for row in hashes:
            decoded = image.ico.getimage((row["size"], row["size"])).convert("RGBA")
            assert hashlib.sha256(decoded.tobytes()).hexdigest() == row["rgba_sha256"], row
    # Pillow's CUR reader does not support PNG cursor entries. Check the CUR
    # directory independently, and compare the identical image payloads through
    # its ICO reader. This is not a Windows cursor-consumer qualification.
    cursor = files["mixed.cur"]
    for index, source in enumerate(entries):
        row = struct.unpack_from("<BBBBHHII", cursor, 6 + index * 16)
        assert row[4:6] == (index, index + 1)
        assert cursor[row[7]:row[7] + row[6]] == source[2]
    print(json.dumps({"pillow": PIL.__version__, "exact_ico_entries": hashes, "cur_payloads_and_hotspots": "passed", "windows_consumer": "not run"}))


if __name__ == "__main__":
    main()
