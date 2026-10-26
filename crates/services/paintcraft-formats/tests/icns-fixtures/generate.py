"""Owned ICNS/PNG wire packing; no third-party icons or runtime libraries."""
import hashlib
import json
import plistlib
import struct
import zlib
from pathlib import Path
ROOT = Path(__file__).parent
TYPES = [("icp4",16,1),("icp5",32,1),("icp6",64,1),("ic07",128,1),("ic08",256,1),("ic09",512,1),
         ("ic11",16,2),("ic12",32,2),("ic13",128,2),("ic14",256,2),("ic10",512,2)]


def pixels(side, seed):
    return bytes(value for y in range(side) for x in range(side) for value in
                 ((seed * 31 + (x % 4) * 17) % 256, (seed * 13 + (y % 4) * 29) % 256,
                  (seed * 19 + (x + y) % 5) % 256, [0, 64, 128, 255][(x + y) % 4]))


def chunk(tag, data):
    tag = tag.encode()
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data))


def png(side, rgba, metadata=False):
    rows = b"".join(b"\0" + rgba[y*side*4:(y+1)*side*4] for y in range(side))
    data = b"\x89PNG\r\n\x1a\n" + chunk("IHDR", struct.pack(">IIBBBBB", side, side, 8, 6, 0, 0, 0))
    if metadata:
        data += chunk("sRGB", b"\0") + chunk("eXIf", b"II*\0\x08\0\0\0\0\0\0\0\0\0")
    return data + chunk("IDAT", zlib.compress(rows, 9)) + chunk("IEND", b"")


def record(tag, payload):
    return tag.encode() + struct.pack(">I", 8 + len(payload)) + payload


def family(records, toc=True):
    if toc:
        records = [record("TOC ", b"".join(item[:8] for item in records))] + records
    payload = b"".join(records)
    return b"icns" + struct.pack(">I", 8 + len(payload)) + payload


def image(tag, logical, scale, seed, metadata=False):
    rgba = pixels(logical * scale, seed)
    return record(tag, png(logical * scale, rgba, metadata)), {"tag": tag, "logical": logical, "scale": scale,
        "side": logical * scale, "seed": seed, "rgba_sha256": hashlib.sha256(rgba).hexdigest(), "qualified": True}


def main():
    files = {}
    records, images = zip(*(image(tag, logical, scale, index + 1) for index, (tag, logical, scale) in enumerate(TYPES)))
    files["modern.icns"] = (family(list(records)), list(images))
    for index,(tag,logical,scale) in enumerate(TYPES[:3]):
        item,receipt=image(tag,logical,scale,index+1)
        files[f"isolated-{tag}.icns"]=(family([item],False),[receipt])
    records, images = zip(*(image("icp5", 32, 1, seed) for seed in [2, 8]))
    files["duplicates.icns"] = (family(list(records), False), list(images))
    item, receipt = image("icp4", 16, 1, 3, True)
    files["metadata.icns"] = (family([item, record("info", plistlib.dumps({"Owned": "inert retained metadata"}, fmt=plistlib.FMT_BINARY))]), [receipt])
    item, receipt = image("icp4", 16, 1, 3)
    files["mixed-legacy.icns"] = (family([item, record("is32", bytes([20, 40, 60]) * 256)]), [receipt,
        {"tag": "is32", "logical": 16, "scale": 1, "side": 16, "qualified": False, "reason": "legacy RGB/mask behavior unqualified"}])
    files["mixed-jp2.icns"] = (family([item, record("ic08", b"\0\0\0\x0cjP  \r\n\x87\n")]), [receipt,
        {"tag": "ic08", "logical": 256, "scale": 1, "side": 256, "qualified": False, "reason": "inert unqualified compressed payload, never decoded"}])
    manifest = {"license": "MIT, repository-authored wire/pixel content", "generator": "Python standard library", "files": {}}
    for name, (data, images) in files.items():
        (ROOT / name).write_bytes(data)
        manifest["files"][name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "entries": images}
    (ROOT / "sources.json").write_text(json.dumps(manifest, indent=4) + "\n")


if __name__ == "__main__":
    main()
