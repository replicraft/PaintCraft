# PNG/BMP source metadata fixtures

These 19 tiny files are independently serialized by Python's standard-library
`struct` and `zlib`, without PaintCraft, `image`, or `png` encoding APIs. Run
`python3 generate.py` in this directory to reproduce them. `oracle.json` records
encoded SHA-256, literal source scanlines/palettes, expected straight RGBA8 samples,
and their independent SHA-256. Duplicate palette colors deliberately have different
indices. Transparent PNG and masked BMP pixels retain nonzero hidden RGB. PNG 16-bit
uses the established high-byte policy (`value >>8`), with values that distinguish
it from rounding division by 257.

The ICC blob comes from the independently LittleCMS-authored sRGB profile already
recorded in `../tiff-fixtures/oracle.json` for `icc-srgb-rgba8.tif`, page 0. The
producer records its byte hash. Structural inspection verifies profile extent,
header signature, and tag-table bounds; this is not profile interpretation,
gamut conversion, installed-profile lookup, or a claim of ICC retention in PCND.

PNG sources cover indexed 1/2/4/8+tRNS and an asymmetric 5×5 Adam7 image with all
seven pass rows hand-packed independently, raw gAMA/cHRM/sRGB declarations and priority,
ICC and cICP priority, physical meter/aspect units, trailing text, eXIf orientation, sBIT, and RGBA16.
eXIf is represented by its bounded streaming payload length/hash and a dedicated
`exif-metadata-ignored` loss; its presence never duplicates generic PNG metadata
loss or implies the orientation is applied. Tests cover eXIf before/after IDAT,
duplicate rejection, and a streamed payload larger than 1 MiB.
BMP sources cover indexed 1/4/8 with nonzero RGBQUAD reserved bytes (never alpha),
odd padded rows, both row orientations, V4 calibration/masked straight alpha,
and V5 embedded ICC/resolution. Unknown-unit PNG pHYs is aspect information,
not a DPI assignment. No import rescales the pixel dimensions.

`invalid-index.png` deliberately references index 1 with only one PLTE entry.
The descriptor inspector does not decode IDAT and therefore cannot validate the
index plane. Pinned image 0.25.10/png 0.18.1 EXPAND turns this undeclared entry into
opaque black. A strict facade must validate indices during actual indexed decode;
a successful metadata inspection is not proof of valid pixels.

Tests separately mutate CRCs, duplicate declarations, ordering, gamma, ICC extents,
BMP profile overlaps/linked paths, channel masks, and explicit lower inspection
budgets. Cancellation callbacks surround parsing and every 64 KiB streaming scan.
There is no arbitrary new default chunk-count or payload-size restriction:
PNG uses the existing caller pixel allocation allowance; omitted BMP profiles
are streamed within the existing source extent rather than retained wholesale.

This slice describes source facts and required omissions. It does not preserve
palette indices, resolution, text, ICC, or calibrated working color in the editor,
and it does not qualify original applications or physical color reproduction.

Primary references:

- [PNG Third Edition](https://www.w3.org/TR/png-3/), sections 5.6, 11.2, 11.3.2,
  and 11.3.4: chunk structure, palettes, color declarations, and physical dimensions.
- [Microsoft Bitmap Storage](https://learn.microsoft.com/en-us/windows/win32/gdi/bitmap-storage):
  file/header/palette/pixel layout and row order.
- [Microsoft BITMAPV5HEADER](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapv5header):
  resolution, color-space/profile fields, and DIB-relative profile offsets.
