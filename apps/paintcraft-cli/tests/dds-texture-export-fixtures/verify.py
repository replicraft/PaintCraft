"""Independent whole-file/mip pixel oracle for owned actual CLI/MCP DDS copies.

Original DXGI/alpha fields are asserted before single-mip UNORM rewrapping.
The latter checks block samples only, not sRGB wrapper support or transfer math.
"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import struct
import sys


def main():
    from PIL import Image, __version__
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    cases = []
    for path in sorted(args.directory.glob("*/*.dds")):
        if not path.name.startswith(("magenta-", "linear-")):
            continue
        color, encoding, transfer, filtering = path.stem.split("-", 3)
        assert filtering in ("nearest", "box-stored")
        data = path.read_bytes()
        read = lambda offset: struct.unpack_from("<I", data, offset)[0]
        dxgi = {"rgba8": 28, "bc1": 71, "bc2": 74, "bc3": 77}[encoding]
        assert data[:4] == b"DDS " and read(4) == 124 and read(76) == 32
        assert data[84:88] == b"DX10"
        assert (read(16), read(12), read(28)) == (8, 4, 4)
        assert read(128) == dxgi + (transfer == "srgb")
        assert (read(132), read(136), read(140)) == (3, 0, 1)
        assert read(144) == (3 if encoding == "bc1" else 1)
        assert read(112) == 0 and read(108) & 0x400008 == 0x400008
        expected = bytes([255, 0, 255, 255] if color == "magenta" else [55, 128, 255, 255])
        if color == "linear":
            assert encoding == "rgba8" and transfer == "linear8"
        else:
            assert color == "magenta"
        original = "accepted"
        try:
            Image.open(io.BytesIO(data)).load()
        except (NotImplementedError, OSError) as error:
            original = {"not_qualified_by_this_oracle": str(error)}
        levels = []
        offset = 148
        for width, height in [(8, 4), (4, 2), (2, 1), (1, 1)]:
            size = width * height * 4 if encoding == "rgba8" else ((width + 3) // 4) * ((height + 3) // 4) * (8 if encoding == "bc1" else 16)
            payload = data[offset:offset + size]
            assert len(payload) == size
            header = bytearray(data[:148])
            for at, value in [(12, height), (16, width), (20, width * 4 if encoding == "rgba8" else size), (28, 1), (108, 0x1000), (128, dxgi)]:
                struct.pack_into("<I", header, at, value)
            struct.pack_into("<I", header, 8, read(8) & ~0x20000)
            picture = Image.open(io.BytesIO(header + payload)).convert("RGBA")
            actual = picture.tobytes()
            assert picture.size == (width, height)
            assert actual == expected * (width * height), (path, width, height)
            levels.append({"size": [width, height], "offset": offset, "bytes": size,
                           "rgba_sha256": hashlib.sha256(actual).hexdigest()})
            offset += size
        assert offset == len(data)
        cases.append({"file": str(path.relative_to(args.directory)),
                      "sha256": hashlib.sha256(data).hexdigest(),
                      "original_wrapper": original, "levels": levels})
    assert len(cases) == 17, len(cases)
    report = {"python": sys.version, "pillow": __version__,
              "oracle_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "imaging_sha256": hashlib.sha256(Path(Image.core.__file__).read_bytes()).hexdigest(),
              "scope": "17 actual consumer files, 68 mip surfaces; exact owned uniform samples, not a general compression-error bound",
              "cases": cases}
    with args.report.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(f"{len(cases)} whole files / {sum(len(c['levels']) for c in cases)} exact mip surfaces")


if __name__ == "__main__":
    main()
