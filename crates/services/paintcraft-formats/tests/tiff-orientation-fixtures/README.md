# TIFF orientation coordinate fixtures

These authored 3 × 2 images have source coordinate labels `ABC/DEF`.
`generate.py` writes classic TIFF IFDs and uncompressed scanline samples directly
with Python `struct`. No PaintCraft codec participates. `oracle.json` contains
explicit independently written label orders for all eight TIFF Orientation
values, normalized dimensions and exact straight RGBA bytes, including hidden
RGB at alpha zero. Gray/alpha16 expectations use the established high-byte
conversion (`sample >> 8`), not rounding or alpha premultiplication.

The 25 files contain 32 directories: eight standalone orientations each for
RGBA8 little endian, RGBA8 big endian and Gray/alpha16 big endian, plus one
eight-page RGBA8 little-endian source. The original geometry is always 3 × 2;
orientations 5 through 8 normalize to 2 × 3. The source Orientation and original
geometry are provenance, rather than a second transform for the imported pixels.

Regenerate with `python3 generate.py`. The separate authored `reference.c`
uses libtiff `TIFFReadScanline` and original tags to check the source samples.
It deliberately avoids RGBA convenience APIs that may apply orientation or
premultiply alpha. A local qualification build can use:

```sh
clang -std=c11 -Wall -Wextra -Werror -O2 -I/opt/homebrew/include reference.c \
    -L/opt/homebrew/lib -ltiff -o ../../../../../target/qualification/tiff-orientation/reference
```

The independent scanline receipt records all 25 source hashes, 32 raw sample
arrays and the tool/library hashes under ignored `docs/validation/tiff-orientation`.
It is local evidence, not an original-application, foreign-OS or hardware claim.
