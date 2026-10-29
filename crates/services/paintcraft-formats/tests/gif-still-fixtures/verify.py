"""Recheck owned GIF streams with GIFLIB and Pillow, never PaintCraft."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

import PIL
from PIL import Image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--giflib-decoder", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    manifest = json.loads((root / "oracle.json").read_text())
    assert PIL.__version__ == manifest["pillow"]
    for case in manifest["cases"]:
        path = root / case["file"]
        assert hashlib.sha256(path.read_bytes()).hexdigest() == case["sha256"]
        facts = json.loads(subprocess.check_output([args.giflib_decoder, path]))
        assert facts == case["descriptor"]
        pillow = Image.open(path).convert("RGBA")
        assert pillow.size == (facts["width"], facts["height"])
        assert list(pillow.tobytes()) == case["pillow_rgba"]
        inside = []
        for index in facts["indices"]:
            inside.extend(facts["palette"][index])
            inside.append(0 if index == facts["transparent"] else 255)
        assert inside == case["source_frame_rgba"]
        for i in range(len(facts["indices"])):
            x = facts["left"] + i % facts["frame_width"]
            y = facts["top"] + i // facts["frame_width"]
            assert list(pillow.getpixel((x, y))) == inside[i * 4 : i * 4 + 4]
        if case["supported"]:
            fill = ([0, 0, 0, 0] if facts["transparent"] >= 0 else
                    facts["global_palette"][facts["background"]] + [255])
            expected = fill * (facts["width"] * facts["height"])
            for i in range(len(facts["indices"])):
                at = ((facts["top"] + i // facts["frame_width"]) * facts["width"]
                      + facts["left"] + i % facts["frame_width"]) * 4
                expected[at : at + 4] = inside[i * 4 : i * 4 + 4]
            assert expected == case["rgba"]
    for stem in ("literal-three", "invalid-palette-index"):
        actual = json.loads(subprocess.check_output([args.giflib_decoder, root / f"{stem}.gif"]))
        assert actual == json.loads((root / f"{stem}.json").read_text())
    bad = json.loads((root / "invalid-palette-index.json").read_text())
    assert len(bad["palette"]) == 2 and any(index >= 2 for index in bad["indices"])
    assert json.loads((root / "literal-three.json").read_text())["indices"] == [0, 1, 0]
    print("GIFLIB source facts and Pillow source-frame samples agree for all 10 fixtures; "
          "9 explicit compositions and 2 literal/index controls verified.")


if __name__ == "__main__":
    main()
