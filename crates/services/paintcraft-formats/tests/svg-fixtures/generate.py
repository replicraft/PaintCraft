"""Authored SVG fixtures and arithmetic PNG. No renderer or third-party artwork."""
import base64
import binascii
import hashlib
import json
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).parent


def png():
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", binascii.crc32(kind + data))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 2, 1, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes([0, 255, 0, 0, 255, 0, 255, 0, 128]))) + chunk(b"IEND", b""))


fixtures = [
    ("aligned", 32, 16, '''<rect width="16" height="16" fill="#ff0000"/>
      <rect x="16" width="16" height="8" fill="#00ff00"/>
      <rect x="16" y="8" width="16" height="8" fill="#0000ff" opacity="0.5"/>'''),
    ("gradient-use", 48, 24, '''<defs>
      <linearGradient id="paint"><stop stop-color="#123456"/><stop offset="1" stop-color="#edcba9"/></linearGradient>
      <rect id="tile" width="22" height="24"/>
      </defs><style>.painted { fill: url(#paint); }</style>
      <g class="painted"><use href="#tile"/><use href="#tile" x="26"/></g>'''),
    ("mask-clip", 32, 32, '''<defs>
      <clipPath id="clip"><circle cx="16" cy="16" r="14"/></clipPath>
      <mask id="mask"><rect width="32" height="32" fill="white"/><rect x="16" width="16" height="32" fill="#808080"/></mask>
      </defs><g clip-path="url(#clip)" mask="url(#mask)"><rect width="32" height="32" fill="#6633cc"/></g>'''),
    ("blur", 48, 32, '''<defs><filter id="blur" x="0" y="0" width="48" height="32" filterUnits="userSpaceOnUse" color-interpolation-filters="sRGB"><feGaussianBlur stdDeviation="1.25"/></filter></defs>
      <rect x="12" y="8" width="24" height="16" rx="4" fill="#ee5599" filter="url(#blur)"/>'''),
    ("curves", 64, 48, '''<g transform="translate(2 3)" opacity="0.7">
      <path d="M3 36 C8 0 42 0 56 36 Q28 22 3 36Z" fill="#2299bb" stroke="#551177" stroke-width="2.5"/>
      <circle cx="28" cy="15" r="6" fill="#ffcc00"/>
      </g>'''),
    ("text", 128, 32, '''<text x="3" y="24" font-family="Selawik" font-size="22" fill="#123456">PaintCraft</text>'''),
    ("embedded", 16, 8, f'''<image width="16" height="8" image-rendering="pixelated" href="data:image/png;base64,{base64.b64encode(png()).decode()}"/>'''),
    ("embedded-css", 16, 8, f'''<style>image {{ image-rendering: auto; }}</style><image width="16" height="8" image-rendering="pixelated" href="data:image/png;base64,{base64.b64encode(png()).decode()}"/>'''),
    ("embedded-css-smooth", 16, 8, f'''<style>image {{ image-rendering: smooth; }}</style><image width="16" height="8" image-rendering="pixelated" href="data:image/png;base64,{base64.b64encode(png()).decode()}"/>'''),
]
manifest = {"generator": "Authored SVG, Python struct/zlib arithmetic PNG; no third-party artwork", "files": []}
for name, width, height, body in fixtures:
    raw = f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}">{body}</svg>\n'.encode()
    (ROOT / f"{name}.svg").write_bytes(raw)
    manifest["files"].append(dict(name=name, width=width, height=height, source_sha256=hashlib.sha256(raw).hexdigest()))
(ROOT / "sources.json").write_text(json.dumps(manifest, indent=4) + "\n")
