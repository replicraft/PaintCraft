"""Independent psd-tools/Pillow checks for owned fresh RGB8 PSD/PSB output.

Source expectations are immutable Document snapshots exported by the opt-in
Rust fixture, not ag-psd decode results. Cached preview and fresh layer render
are measured separately; neither substitutes for exact raw layer checks.
"""
import hashlib
import json
from pathlib import Path
import sys

from PIL import __version__ as pillow_version
from psd_tools import PSDImage
from psd_tools.constants import Resource
import psd_tools

ROOT = Path(sys.argv[1])
PROFILE = Path(__file__).resolve().parents[5] / "assets/profiles/sRGB2014.icc"
expected = json.loads((ROOT / "expected.json").read_text())
assert psd_tools.__version__ == "1.26.0" and pillow_version == "13.0.0", "use the pinned oracle versions"


def sha(value):
    return hashlib.sha256(value).hexdigest()


def difference(actual, reference):
    if len(actual) != len(reference):
        raise AssertionError("independent image extent mismatch")
    errors = [abs(a - b) for a, b in zip(actual, reference)]
    maximum = max(errors, default=0)
    visible_rgb = [value for i,value in enumerate(errors) if i%4!=3 and reference[(i//4)*4+3]!=0]
    displayed = {}
    for background in (0,255):
        opaque_actual, opaque_reference = [], []
        for index in range(0,len(actual),4):
            for channel in range(3):
                opaque_actual.append((actual[index+channel]*actual[index+3]+background*(255-actual[index+3])+127)//255)
                opaque_reference.append((reference[index+channel]*reference[index+3]+background*(255-reference[index+3])+127)//255)
        displayed[str(background)] = max((abs(a-b) for a,b in zip(opaque_actual,opaque_reference)),default=0)
    return {
        "maximum_channel_error": maximum,
        "maximum_rgb_error_alpha_nonzero": max(visible_rgb,default=0),
        "opaque_backdrop_maximum_error": displayed,
        "different_channels": sum(bool(value) for value in errors),
        "maximum_rgb_error": max((value for i, value in enumerate(errors) if i % 4 != 3), default=0),
        "maximum_alpha_error": max(errors[3::4], default=0),
        "sha256": sha(actual),
        "expected_sha256": sha(reference),
    }


records = []
for case in expected:
    for suffix, version in (("psd", 1), ("psb", 2)):
        path = ROOT / (case["case"] + "." + suffix)
        original = path.read_bytes()
        image = PSDImage.open(path)
        assert image._record.header.version == version
        assert image.size == (case["width"], case["height"])
        assert image.depth == 8
        assert image.image_resources.get_data(Resource.ICC_PROFILE) == PROFILE.read_bytes()
        assert len(image) == len(case["layers"])
        layers = []
        for index, (actual, layer) in enumerate(zip(image, case["layers"])):
            assert actual.name == layer["name"]
            assert (actual.left, actual.top) == (layer["x"], layer["y"])
            assert actual.size == (layer["width"], layer["height"])
            assert actual.visible == layer["visible"]
            assert actual.opacity == layer["opacity"]
            assert actual.blend_mode.value == b"norm"
            pixels = actual.topil(apply_icc=False).convert("RGBA").tobytes()
            source = (ROOT / layer["pixels"]).read_bytes()
            assert sha(source) == layer["sha256"]
            assert pixels == source, (path.name, index, "raw pixels changed")
            layers.append({"name": actual.name, "sha256": sha(pixels)})
        reference = (ROOT / case["merged"]).read_bytes()
        cached = image.topil(apply_icc=False).convert("RGBA").tobytes()
        recomposed = image.composite(
            force=True, ignore_preview=True, color=0.0, alpha=0.0, apply_icc=False
        ).convert("RGBA").tobytes()
        assert path.read_bytes() == original
        records.append({
            "file": path.name,
            "sha256": sha(original),
            "layers": layers,
            "cached_preview": difference(cached, reference),
            "recomposed_layers": difference(recomposed, reference),
        })
report = {
    "python": sys.version,
    "psd_tools": psd_tools.__version__,
    "pillow": pillow_version,
    "profile_sha256": sha(PROFILE.read_bytes()),
    "files": records,
    "scope": "Independent raw layers/profile and separately measured cache/render; no Photoshop qualification",
}
if "--record" in sys.argv[2:]:
    with (ROOT / "independent-writer-oracle.json").open("x") as output:
        output.write(json.dumps(report, indent=4) + "\n")
for row in records:
    print(row["file"], "raw layers exact; cache", row["cached_preview"], "render", row["recomposed_layers"])
