# BMP16 conversion specimens

These original specimens cover opaque BI_RGB RGB555 and explicit BI_BITFIELDS
RGB555/RGB565 in 40, 108 and 124-byte headers, each stored top-down and bottom-up.
Odd row widths include nonzero alignment padding. Every 5/6-bit channel value
occurs, and unused RGB555 bit 15 is set on alternating pixels.

The exact conversion oracle scales each component to the nearest integer in
0..255 and sets alpha to 255. This is the selected `image` backend's expansion
policy. It expands a packed 16-bit pixel, not 16 bits per component. Implicit
RGB555 has no explicit mask declarations; the source descriptor retains zero
mask fields instead of inventing source metadata.

`generate.py` serializes the headers and samples directly. It also decodes each
specimen through FFmpeg and records that executable's version and SHA-256 in
`oracle.json`. FFmpeg's bit expansion differs by at most one RGB level on some
interior values. Both outputs are retained: `.rgba` is the exact rational oracle
and `.ffmpeg.rgba` is independent raw output. This is not an exact cross-decoder
agreement claim. Alpha, geometry and row order agree.

No BMP16 alpha, arbitrary masks, RLE16 or other header families are admitted.
Tests mutate overlapping, sparse, out-of-range and alpha masks, compression,
metadata overlap and truncated storage; caller pixel admission remains enforced.
