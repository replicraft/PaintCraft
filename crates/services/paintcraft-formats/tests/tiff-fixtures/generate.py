"""Record independently produced TIFFs and raw libtiff scanline/tile samples."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--libtiff-library", type=Path, required=True)
    parser.add_argument("--libtiff-version", required=True)
    parser.add_argument("--lcms-library", type=Path, required=True)
    parser.add_argument("--lcms-version", required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    reference = args.reference.resolve(strict=True)
    folder = Path(__file__).resolve().parent
    args.evidence.mkdir(parents=True, exist_ok=True)
    subprocess.run([str(reference), "generate", str(folder)], check=True)
    fixtures = []
    for path in sorted(folder.glob("*.tif")):
        result = subprocess.run([str(reference), "inspect", str(path)],
                                capture_output=True, text=True, check=True)
        pages = json.loads(result.stdout)
        for page in pages:
            raw, channels, bits = page["samples"], page["channels"], page["bits"]
            photo, extra = page["photometric"], page["extra"]
            qualified = photo in (0, 1, 2) and extra in (0, 2) and bits in (1, 8, 16) and page["sample_format"] == 1
            page["qualified_integer_profile"] = qualified
            if not qualified:
                continue
            rgba = []
            for index in range(0, len(raw), channels):
                values = raw[index:index + channels]
                if bits == 16:
                    values = [value >> 8 for value in values]
                elif bits == 1:
                    values = [255 * value for value in values]
                if photo in (0, 1):
                    gray = 255 - values[0] if photo == 0 else values[0]
                    rgba.extend([gray, gray, gray, values[1] if extra == 2 else 255])
                else:
                    rgba.extend(values[:3] + [values[3] if extra == 2 else 255])
            page["expected_rgba8"] = rgba
            page["expected_rgba8_sha256"] = hashlib.sha256(bytes(rgba)).hexdigest()
        fixtures.append({"file": path.name, "bytes": path.stat().st_size,
                         "sha256": digest(path), "pages": pages,
                         "whole_file_qualified": not path.name.startswith("bigtiff")})
        (args.evidence / (path.stem + "-reference.json")).write_text(result.stdout)
    oracle = {
        "provenance": {
            "oracle_builder_source_sha256": digest(Path(__file__).resolve()),
            "generator_source_sha256": digest(folder / "reference.c"),
            "generator_binary_sha256": digest(reference),
            "libtiff_version": args.libtiff_version,
            "libtiff_dylib_sha256": digest(args.libtiff_library.resolve(strict=True)),
            "littlecms_version": args.lcms_version,
            "littlecms_dylib_sha256": digest(args.lcms_library.resolve(strict=True)),
            "reference_api": "TIFFReadScanline/TIFFReadTile raw samples; no RGBA convenience APIs",
            "sample_ownership": "Authored arrays and arithmetic patterns in reference.c; no external artwork",
            "conversion_policy": "tiff-uint16-high-byte-rgba8-v1",
        },
        "fixtures": fixtures,
    }
    (folder / "oracle.json").write_text(json.dumps(oracle, indent=2) + "\n")
    print(f"Generated {len(fixtures)} independent TIFF fixtures and {sum(len(f['pages']) for f in fixtures)} page records")


if __name__ == "__main__":
    main()
