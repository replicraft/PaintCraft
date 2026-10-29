"""Independent BMP/DIB packing and literal RGBA oracles; no image codec imports."""
from pathlib import Path
import hashlib
import json
import struct

HERE = Path(__file__).resolve().parent
PROFILE_PATH = HERE.parents[4] / "assets/profiles/sRGB2014.icc"
PROFILE = PROFILE_PATH.read_bytes()
RGB = [(13, 71, 201), (250, 1, 20), (3, 4, 5), (17, 99, 200), (255, 0, 128), (0, 127, 254)]
ALPHA = [0, 128, 255, 1, 254, 64]
PALETTE = [(13, 71, 201, 137), (250, 1, 20, 0)]
INDICES = [0, 1, 0, 1, 1, 0]
ROWS = []


def emit(name, *, dib=124, depth=24, density=(0, 0), profile=False,
         before=False, top=False, calibrated=False):
    width, height = 3, 2
    stride = ((width * depth + 31) // 32) * 4
    rows, expected = [], []
    for y in range(height):
        packed = bytearray((width * depth + 7) // 8)
        for x in range(width):
            at = y * width + x
            if depth <= 8:
                index = INDICES[at]
                packed[x * depth // 8] |= index << (8 - depth - x * depth % 8)
                expected.extend([*PALETTE[index][:3], 255])
            else:
                r, g, b = RGB[at]
                if depth == 24:
                    packed[x * 3:x * 3 + 3] = bytes([b, g, r])
                    expected.extend([r, g, b, 255])
                else:
                    packed[x * 4:x * 4 + 4] = bytes([b, g, r, ALPHA[at]])
                    expected.extend([r, g, b, ALPHA[at]])
        rows.append(bytes(packed) + bytes(stride - len(packed)))
    pixels = b"".join(rows if top else reversed(rows))
    palette = b"".join(bytes([b, g, r, reserved]) for r, g, b, reserved in PALETTE) if depth <= 8 else b""
    original_profile = PROFILE if profile else b""
    header = bytearray(dib)
    struct.pack_into("<IiiHHIIiiII", header, 0, dib, width,
                     -height if top else height, 1, depth, 3 if depth == 32 else 0,
                     len(pixels), *density, len(PALETTE) if palette else 0, 0)
    if depth == 32:
        struct.pack_into("<4I", header, 40, 0xff0000, 0xff00, 0xff, 0xff000000)
    if dib >= 108:
        struct.pack_into("<I", header, 56, 0x4d424544 if profile else 0 if calibrated else 0x73524742)
        if calibrated:
            struct.pack_into("<9i3I", header, 60, *[i * 123456 for i in range(9)], 65536, 70000, 100000)
    if dib == 124:
        profile_offset = dib + len(palette) + (0 if before else len(pixels)) if profile else 0
        struct.pack_into("<4I", header, 108, 4, profile_offset, len(original_profile), 0)
    offset = 14 + dib + len(palette) + (len(original_profile) if before else 0)
    body = original_profile + pixels if before else pixels + original_profile
    data = b"BM" + struct.pack("<IHHI", 14 + dib + len(palette) + len(body), 0, 0, offset)
    data += header + palette + body
    (HERE / name).write_bytes(data)
    (HERE / f"{name}.rgba").write_bytes(bytes(expected))
    ROWS.append({"file": name, "sha256": hashlib.sha256(data).hexdigest(),
                 "width": width, "height": height, "bits_per_pixel": depth,
                 "expected_rgba8": expected, "rgba_sha256": hashlib.sha256(bytes(expected)).hexdigest(),
                 "dib_bytes": dib, "density": list(density), "profile": profile,
                 "profile_sha256": hashlib.sha256(PROFILE).hexdigest() if profile else None,
                 "profile_before_pixels": before if profile else None,
                 "top_down": top, "source_model": "indexed" if palette else "rgb",
                 "palette_reserved": [137, 0] if palette else [], "calibrated": calibrated})


emit("rgb24-v40-density-positive.bmp", dib=40, density=(3780, 3779))
emit("rgb24-v40-density-mixed-zero.bmp", dib=40, density=(0, -3780), top=True)
emit("rgb24-v108-density-negative.bmp", dib=108, density=(-3780, -3779), calibrated=True)
emit("rgb24-v124-density-extremes.bmp", density=(-(1 << 31), (1 << 31) - 1))
emit("rgb24-v124-profile-only.bmp", profile=True, before=True)
emit("rgb24-v124-profile-positive.bmp", profile=True, density=(3780, 3779))
emit("rgb24-v124-profile-negative.bmp", profile=True, before=True, density=(-3780, 0), top=True)
emit("rgba32-v124-profile-hidden.bmp", depth=32, profile=True, density=(0, 3779))
for depth in [1, 4, 8]:
    emit(f"indexed{depth}-v124-profile-reserved.bmp", depth=depth, profile=True, before=True)
emit("rgb24-v108-calibrated-no-retention.bmp", dib=108, calibrated=True)
emit("rgb24-v40-no-metadata.bmp", dib=40)
(HERE / "oracle.json").write_text(json.dumps({
    "schema": "paintcraft.bmp-retention-fixtures/1",
    "generator": "Python standard-library struct packing; independent literal RGB/alpha/indices",
    "profile_path": "assets/profiles/sRGB2014.icc",
    "profile_sha256": hashlib.sha256(PROFILE).hexdigest(),
    "fixtures": ROWS,
}, indent=4) + "\n")
