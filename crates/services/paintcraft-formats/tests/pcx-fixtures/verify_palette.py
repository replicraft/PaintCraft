"""Independently decode a quantized PCX and compare it to the exact preview."""
import json
from pathlib import Path
import subprocess
import sys

import PIL
from PIL import Image

folder = Path(sys.argv[1])
preview = (folder / "preview.rgba").read_bytes()
source = (folder / "source.rgba").read_bytes()
with Image.open(folder / "quantized.pcx") as image:
    assert image.size == (40, 10) and image.mode == "P"
    actual = image.convert("RGBA").tobytes()
    assert actual == preview and actual != source
    assert len(image.getcolors(256)) <= 256
    colors = len(image.getcolors(256))
decoded = subprocess.run(
    ["ffmpeg", "-v", "error", "-threads", "1", "-i", str(folder / "quantized.pcx"), "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"],
    check=True, capture_output=True, timeout=30,
)
assert decoded.stdout == preview
changed = sum(source[p:p + 4] != actual[p:p + 4] for p in range(0, len(source), 4))
print(json.dumps({"pillow": PIL.__version__, "exact_preview_matches": ["Pillow", "FFmpeg"],
                  "colors": colors, "changed_pixels": changed}))
