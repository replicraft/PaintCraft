"""Independent psd-tools/Pillow layer pixels, identity, and composite checks."""
import hashlib
import json
from pathlib import Path
from psd_tools import PSDImage
from PIL import __version__ as pillow_version
import psd_tools

ROOT = Path(__file__).parent
manifest = json.loads((ROOT / "oracle.json").read_text())
for name, digest in manifest["files"].items():
    path = ROOT / name
    assert hashlib.sha256(path.read_bytes()).hexdigest() == digest, name
    source = PSDImage.open(path)
    assert source.size == (5, 3), name
    expected_layers = manifest["layers_bottom_to_top"] if name.startswith("layers") else []
    # Compare file records in the independent reader's background-to-foreground order.
    assert len(source) == len(expected_layers), name
    for actual, expected in zip(source, expected_layers):
        assert actual.name == expected["name"], name
        assert (actual.left, actual.top) == tuple(expected["origin"]), name
        assert actual.size == (expected["width"], expected["height"]), name
        assert actual.opacity == expected["opacity"] and actual.visible == expected["visible"], name
        image = actual.topil(apply_icc=False).convert("RGBA")
        assert hashlib.sha256(image.tobytes()).hexdigest() == expected["rgba_sha256"], name
    image = source.topil(apply_icc=False).convert("RGB")
    planes = bytes((i * 13 + 17) % 256 for i in range(45))
    expected = bytes(planes[channel * 15 + pixel] for pixel in range(15) for channel in range(3))
    assert image.tobytes() == expected, name
print(f"Verified {len(manifest['files'])} PSD/PSB fixtures with psd-tools {psd_tools.__version__}, Pillow {pillow_version}")
