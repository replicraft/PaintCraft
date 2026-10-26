"""Independent writer specimen; owned pixels, no third-party image data."""
import importlib.metadata
from pathlib import Path
from PIL import Image
assert importlib.metadata.version("Pillow") == "13.0.0"
# Explicit row-major 7x3 black/white pixels; not a format-byte parser.
pixels = [255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 255, 255, 255, 255, 255, 255]
image = Image.new("1", (7, 3))
image.putdata(pixels)
image.save(Path(__file__).with_name("pillow-7.msp"), format="MSP")
