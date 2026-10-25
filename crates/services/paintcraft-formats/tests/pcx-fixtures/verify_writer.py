"""Independent PCX writer oracle; Pillow and FFmpeg never use PaintCraft code."""
import json
from pathlib import Path
import subprocess
import sys

import PIL
from PIL import Image

folder = Path(sys.argv[1])
palette = (folder / "palette.rgb").read_bytes()
checked = []
for width in [1, 7, 63, 64, 65, 257]:
    rgb = (folder / f"{width}.rgb").read_bytes()
    indices = (folder / f"{width}.indices").read_bytes()
    expected = b"".join(rgb[p:p + 3] + b"\xff" for p in range(0, len(rgb), 3))
    for profile in ["rgb", "indexed"]:
        path = folder / f"{width}-{profile}.pcx"
        with Image.open(path) as image:
            assert image.size == (width, 3)
            assert image.info["dpi"] == (72, 144)
            assert image.convert("RGBA").tobytes() == expected
            if profile == "indexed":
                assert image.mode == "P"
                assert image.tobytes() == indices
                assert bytes(image.getpalette()) == palette
        decoded = subprocess.run(
            ["ffmpeg", "-v", "error", "-threads", "1", "-i", str(path), "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"],
            check=True, capture_output=True, timeout=30,
        )
        assert decoded.stdout == expected
        checked.append({"width": width, "profile": profile})
print(json.dumps({"pillow": PIL.__version__, "ffmpeg": subprocess.check_output(["ffmpeg", "-version"], text=True).splitlines()[0],
                  "exact_rgba": checked, "index_bytes_and_palette_order": "passed"}))
