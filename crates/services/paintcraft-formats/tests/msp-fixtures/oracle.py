"""Optional independent Pillow reader qualification, not a runtime dependency."""
import hashlib
import importlib.metadata
import inspect
import json
import sys
from pathlib import Path
from PIL import Image, MspImagePlugin
ROOT = Path(__file__).parent
assert importlib.metadata.version("Pillow") == "13.0.0"
plugin = Path(inspect.getfile(MspImagePlugin))
assert hashlib.sha256(plugin.read_bytes()).hexdigest() == "8e28773c59bae649f034c22b68d64ae4cdc9c26605947e34114e254dd458f8e6"
manifest = json.loads((ROOT / "sources.json").read_text())
for name, receipt in manifest["files"].items():
    source = ROOT / name
    data = source.read_bytes()
    assert len(data) == receipt["bytes"]
    assert hashlib.sha256(data).hexdigest() == receipt["sha256"]
    for directory in [ROOT] + ([Path(sys.argv[1])] if len(sys.argv) == 2 else []):
        with Image.open(directory / name) as image:
            assert image.format == "MSP"
            assert image.size == (receipt["width"], receipt["height"])
            assert hashlib.sha256(image.convert("RGBA").tobytes()).hexdigest() == receipt["rgba_sha256"]
        print(json.dumps({"source": str(directory / name), "width": receipt["width"], "height": receipt["height"], "rgba_sha256": receipt["rgba_sha256"], "oracle": "Pillow 13.0.0", "qualification": "independent reader pixels; original Windows Paint consumer unqualified"}))
