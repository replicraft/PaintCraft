"""Independent original-header/chain, rational filter and Pillow BC pixel checks.

Run only on owned writer specimens. No product reader or header parser is used.
SRGB payload rewrapping is explicitly pixel-only evidence, not wrapper acceptance.
"""
import argparse
from fractions import Fraction
import hashlib
import io
import json
from pathlib import Path
import math
import re
import struct
import sys


def intended(base, width, height, w, h, filtering):
    pixels = []
    for y in range(h):
        for x in range(w):
            if filtering == "Nearest":
                sx = int(Fraction((2 * x + 1) * width, 2 * w))
                sy = int(Fraction((2 * y + 1) * height, 2 * h))
                pixels.extend(base[(sy * width + sx) * 4:][:4])
                continue
            x0, x1 = Fraction(x * width, w), Fraction((x + 1) * width, w)
            y0, y1 = Fraction(y * height, h), Fraction((y + 1) * height, h)
            sums = [Fraction(0) for _ in range(4)]
            total = (x1 - x0) * (y1 - y0)
            for sy in range(math.floor(y0), math.ceil(y1)):
                for sx in range(math.floor(x0), math.ceil(x1)):
                    weight = (min(x1, sx + 1) - max(x0, sx)) * (min(y1, sy + 1) - max(y0, sy))
                    p = base[(sy * width + sx) * 4:][:4]
                    sums[3] += p[3] * weight
                    for c in range(3):
                        sums[c] += p[c] * p[3] * weight
            rounded = lambda n: math.floor(n + Fraction(1, 2))
            pixels.extend([rounded(sums[c] / sums[3]) if sums[3] else 0 for c in range(3)])
            pixels.append(rounded(sums[3] / total))
    return bytes(pixels)


def main():
    from PIL import Image, __version__
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--policy", choices=["initial-target", "uniform-lossy-v1"], default="initial-target")
    parser.add_argument("--expected-report", type=Path)
    args = parser.parse_args()
    expected = None
    if args.policy == "uniform-lossy-v1":
        assert args.expected_report, "New policy needs frozen independent decoded goldens"
        expected = {case["file"]: case for case in json.loads(args.expected_report.read_text())["cases"]}
    hashes = {}
    for name in ("DdsImagePlugin.py", "Image.py"):
        p = Path(Image.__file__).parent / name
        hashes[name] = hashlib.sha256(p.read_bytes()).hexdigest()
    report = {"verifier_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "pillow_version": __version__, "python_version": sys.version,
              "imaging_binary_sha256": hashlib.sha256(Path(Image.core.__file__).read_bytes()).hexdigest(),
              "python_plugin_sha256": hashes,
              "scope": "original DX10 fields and all mip extents independently parsed; each mip Pillow-decodes a new single-level wrapper; SRGB is separately rewrapped UNORM for pixel evidence only", "cases": [], "quality_failures": []}
    dxgi = {"Rgba8": (28, 29), "Bc1": (71, 72), "Bc2": (74, 75), "Bc3": (77, 78)}
    files = sorted(args.directory.glob("*.dds"))
    assert len(files) == 48, len(files)
    for file in files:
        match = re.fullmatch(r"(\d+)x(\d+)-(Rgba8|Bc1|Bc2|Bc3)-(Linear|Srgb)-(Nearest|BoxStored)(?:-(High))?", file.stem)
        assert match, file
        width, height = map(int, match.group(1, 2))
        encoding, transfer, filtering = match.group(3, 4, 5)
        raw = file.read_bytes()
        source = file.with_suffix(".rgba")
        if not source.exists():
            source = args.directory.parent / "sources" / f"{width}x{height}-{'opaque' if encoding == 'Bc1' else 'straight'}.rgba"
        base = source.read_bytes()
        assert len(base) == width * height * 4
        u32 = lambda offset: struct.unpack_from("<I", raw, offset)[0]
        assert raw[:4] == b"DDS " and u32(4) == 124 and u32(76) == 32
        assert raw[84:88] == b"DX10" and (u32(16), u32(12)) == (width, height)
        assert u32(128) == dxgi[encoding][transfer == "Srgb"]
        assert u32(132) == 3 and u32(136) == 0 and u32(140) == 1
        assert u32(144) == (3 if encoding == "Bc1" else 1)
        count = max(width, height).bit_length()
        assert u32(28) == count
        assert u32(8) & 0x20000 and u32(108) & 0x400008 == 0x400008
        assert u32(112) == 0
        case = {"file": file.name, "quality": match.group(6) or "Normal", "sha256": hashlib.sha256(raw).hexdigest(),
                "source_rgba_sha256": hashlib.sha256(base).hexdigest(), "levels": []}
        if transfer == "Srgb":
            try:
                Image.open(io.BytesIO(raw)).load()
                case["original_srgb_wrapper"] = "accepted"
            except (NotImplementedError, OSError) as error:
                case["original_srgb_wrapper"] = {"unqualified_by_this_oracle": str(error)}
        offset, w, h = 148, width, height
        for level in range(count):
            size = w * h * 4 if encoding == "Rgba8" else ((w + 3) // 4) * ((h + 3) // 4) * (8 if encoding == "Bc1" else 16)
            payload = raw[offset:offset + size]
            assert len(payload) == size
            header = bytearray(raw[:148])
            for where, value in [(12, h), (16, w), (20, w * 4 if encoding == "Rgba8" else size),
                                 (28, 1), (108, 0x1000), (128, dxgi[encoding][0])]:
                struct.pack_into("<I", header, where, value)
            struct.pack_into("<I", header, 8, u32(8) & ~0x20000)
            picture = Image.open(io.BytesIO(header + payload)).convert("RGBA")
            assert picture.size == (w, h)
            got = picture.tobytes()
            want = base if level == 0 else intended(base, width, height, w, h, filtering)
            rgb_errors = [abs(a - b) for i, (a, b) in enumerate(zip(got, want)) if i % 4 != 3]
            alpha_errors = [abs(a - b) for a, b in zip(got[3::4], want[3::4])]
            maximum = lambda errors: max(errors, default=0)
            rms = lambda errors: math.sqrt(sum(e * e for e in errors) / len(errors))
            if encoding == "Rgba8":
                assert got == want, (file.name, level, got, want)
            else:
                # Initial owned-vector quality budgets; not global encoder bounds.
                alpha_max = {"Bc1": 0, "Bc2": 8, "Bc3": 36}[encoding]
                if maximum(rgb_errors) > 96 or rms(rgb_errors) > 32 or maximum(alpha_errors) > alpha_max:
                    report["quality_failures"].append({"file": file.name, "level": level,
                        "rgb_max_error": maximum(rgb_errors), "rgb_rms_error": rms(rgb_errors),
                        "alpha_max_error": maximum(alpha_errors), "alpha_limit": alpha_max})
            case["levels"].append({"level": level, "width": w, "height": h, "offset": offset,
                                   "bytes": size, "decoded_sha256": hashlib.sha256(got).hexdigest(),
                                   "intended_sha256": hashlib.sha256(want).hexdigest(),
                                   "rgb_max_error": maximum(rgb_errors), "rgb_rms_error": rms(rgb_errors),
                                   "alpha_max_error": maximum(alpha_errors), "alpha_rms_error": rms(alpha_errors)})
            offset += size
            w, h = max(w // 2, 1), max(h // 2, 1)
        assert offset == len(raw), (file, offset, len(raw))
        if expected is not None:
            frozen = expected[file.name]
            assert case["sha256"] == frozen["sha256"]
            assert case["source_rgba_sha256"] == frozen["source_rgba_sha256"]
            assert len(case["levels"]) == len(frozen["levels"])
            for actual, known in zip(case["levels"], frozen["levels"]):
                for key in ["level", "width", "height", "offset", "bytes", "decoded_sha256", "intended_sha256", "rgb_max_error", "alpha_max_error"]:
                    assert actual[key] == known[key], (file.name, key, actual[key], known[key])
        report["cases"].append(case)
        args.report.write_text(json.dumps(report, indent=4) + "\n")
    report["policy"] = args.policy
    report["initial_targets_met"] = not report["quality_failures"]
    report["standard_decoding_layout_filter_and_goldens_passed"] = expected is not None
    if expected is not None:
        assert set(expected) == {case["file"] for case in report["cases"]}
        report["expected_report_sha256"] = hashlib.sha256(args.expected_report.read_bytes()).hexdigest()
    args.report.write_text(json.dumps(report, indent=4) + "\n")
    print(json.dumps({"cases": len(report["cases"]), "levels": sum(len(c["levels"]) for c in report["cases"]),
                      "quality_failures": len(report["quality_failures"])}))
    if args.policy == "initial-target":
        assert not report["quality_failures"], "Predeclared quality budgets failed; see retained report, do not relabel as passing"


if __name__ == "__main__":
    main()
