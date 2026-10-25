"""Check actual benchmark PCX artifacts with independent PNG/PCX decoders."""
import csv
import hashlib
import json
from pathlib import Path
import subprocess
import sys

import PIL
from PIL import Image

folder = Path(sys.argv[1])
checked = []
for row in csv.DictReader((folder / "fixtures.csv").open()):
    if row["format"] != "PCX":
        continue
    path = folder / f"{row['case']}.pcx"
    assert hashlib.sha256(path.read_bytes()).hexdigest() == row["encoded_sha256"]
    with Image.open(folder / f"{row['case']}.png") as image:
        source = image.convert("RGBA")
        assert hashlib.sha256(source.tobytes()).hexdigest() == row["source_sha256"]
        white = Image.new("RGBA", source.size, (255, 255, 255, 255))
        expected = Image.alpha_composite(white, source).tobytes()
    with Image.open(path) as image:
        assert image.size == (int(row["width"]), int(row["height"]))
        assert image.convert("RGBA").tobytes() == expected
    ffmpeg = subprocess.run(
        ["ffmpeg", "-v", "error", "-threads", "1", "-i", str(path), "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"],
        check=True, capture_output=True, timeout=30,
    )
    assert ffmpeg.stdout == expected
    checked.append(row["case"])
assert len(checked) == 8
print(json.dumps({"pillow": PIL.__version__, "exact_pcx_benchmark_cases": checked}))
