# BMP RLE4 and RLE8 fixtures

These 26 small BMP files and their index rows were authored for PaintCraft.
`generate.py` creates the command streams directly, then requires the installed
FFmpeg 8.0.1 BMP decoder to reproduce every explicitly authored RGBA byte.
`oracle.json` preserves those independent results, source SHA-256 values, raw
bottom-up index rows, and separate Pillow 12.0.0 outputs or errors. Product and
Rust tests do not depend on either external tool.

The matrix covers encoded runs and absolute literals through 255 pixels,
RLE4 odd unused nibbles and alignment, RLE8 zero alignment, horizontal/vertical
and zero deltas, skipped rows, early EOI, final EOL followed by EOI, and empty
images. Palette zero is deliberately nonblack. Its reserved RGBQUAD byte is
137, proving that reserved bytes and holes are not alpha. Eight V5 cases place
an existing independently generated sRGB ICC profile before or after the pixel
stream with a zero or nonzero `biSizeImage` declaration. The copied profile is
from the adjacent `raster-metadata-fixtures/rgb24-v5-icc.bmp` fixture; no external
photograph or image content is copied.

The finite import policy is:

- Bottom-up BI_RLE4/BI_RLE8 only; top-down compressed images are refused.
- Authored indices must exist in the declared palette. Unused RLE4 low nibbles
  and RLE4 word-alignment bytes are not samples. RLE8 absolute padding is zero.
- Unwritten pixels are opaque declared palette-zero RGB, not black or clear.
- Mandatory EOI terminates the stream. A nonzero declared storage span contains
  that EOI and only zero trailing padding. A zero `biSizeImage` is accepted as
  an explicit EOI-delimited compatibility import with `bmp-metadata-ignored`;
  its original zero declaration and actual encoded extent remain separate.
- An embedded ICC profile is preserved as source metadata with explicit color
  loss. The scan cannot consume profile bytes while searching for EOI. It does
  not apply the profile or claim color-managed/preserving BMP conversion.

The strict source scanner validates commands and bounds before pixel allocation.
The selected `image` BMP decoder supplies normal RGB samples; a second source
scan verifies every authored RGB sample against its actual palette index and
composes only those pixels over the palette-zero background. The selected
backend's indexed-output switch is unsuitable for its RLE code path and is not
used. Compressed metadata has no physical row stride: declared image size,
actual EOI stream length and admitted storage length are distinct facts.

Pillow 12.0.0 stops or errors on several delta/empty cases; its disagreements
are retained rather than used to replace the authored rows. A separate Apple
ImageIO 14.7.2 exploration ignored the vertical delta on the combined image;
its raw exploratory output remains under ignored `docs/validation/bmp-rle`.
FFmpeg agrees on all 26 vectors, including holes and ICC placement.

Regenerate with Python containing Pillow 12.0.0 and installed FFmpeg 8.0.1:

```sh
python crates/services/paintcraft-formats/tests/bmp-rle-fixtures/generate.py
```

Tests additionally mutate actual command bytes to cover malformed indices,
run/delta overflow, missing EOI, short/padded spans, top-down compression,
profile overlap and metadata/resource/cancellation refusal. This finite reader
qualification does not add BMP16, full BMP/CMS support, or evidence from Windows
Paint or physical devices. Existing source/preview/consent guards are unchanged.
